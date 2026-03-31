use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use repropack_git::{apply_patch, checkout_commit, clone_bundle};
use repropack_model::{DriftItem, ReplayPolicy, ReplayReceipt, ReplayStatus, Severity};
use repropack_pack::materialize;
use repropack_render::render_receipt_markdown;

#[derive(Clone, Debug, Default)]
pub struct ReplayOptions {
    pub into: Option<PathBuf>,
    pub set_env: BTreeMap<String, String>,
    pub no_run: bool,
    pub force: bool,
}

pub struct ReplayResult {
    pub workdir: PathBuf,
    pub receipt_path: PathBuf,
    pub receipt: ReplayReceipt,
    pub command_exit_code: i32,
}

pub fn replay(packet: &Path, options: &ReplayOptions) -> Result<ReplayResult> {
    let materialized = materialize(packet).with_context(|| format!("materializing {}", packet.display()))?;
    let manifest = repropack_model::PacketManifest::read_from_path(&materialized.manifest_path())
        .context("reading manifest from packet")?;

    let workdir = options
        .into
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("repropack-replay-{}", manifest.packet_id)));

    if workdir.exists() {
        return Err(anyhow!("replay target already exists: {}", workdir.display()));
    }

    let mut receipt = ReplayReceipt::new(
        manifest.packet_id.clone(),
        workdir.display().to_string(),
        manifest.command.display.clone(),
    );
    receipt.recorded_exit_code = manifest.execution.exit_code;

    let support_dir = workdir.join(".repropack-replay");

    match manifest.replay_policy {
        ReplayPolicy::Disabled => {
            fs::create_dir_all(&support_dir).with_context(|| format!("creating {}", support_dir.display()))?;
            receipt.status = ReplayStatus::Blocked;
            receipt.notes.push("packet replay policy is disabled".to_string());
            return finalize_receipt(&support_dir, receipt, 0);
        }
        ReplayPolicy::Confirm if !options.force => {
            fs::create_dir_all(&support_dir).with_context(|| format!("creating {}", support_dir.display()))?;
            receipt.status = ReplayStatus::Blocked;
            receipt.notes.push("packet replay policy requires --force".to_string());
            return finalize_receipt(&support_dir, receipt, 0);
        }
        _ => {}
    }

    let mut can_run = true;

    if let Some(git) = &manifest.git {
        if let Some(bundle_path) = &git.bundle_path {
            let bundle = materialized.root.join(bundle_path);
            if bundle.exists() {
                match clone_bundle(&bundle, &workdir) {
                    Ok(_) => {
                        if let Some(commit) = &git.commit_sha {
                            if let Err(err) = checkout_commit(&workdir, commit) {
                                can_run = false;
                                receipt.status = ReplayStatus::Error;
                                receipt.drift.push(DriftItem {
                                    subject: "checkout".to_string(),
                                    expected: Some(commit.clone()),
                                    observed: Some("failed".to_string()),
                                    severity: Severity::Error,
                                });
                                receipt.notes.push(format!("git checkout failed: {err}"));
                            }
                        }
                    }
                    Err(err) => {
                        can_run = false;
                        fs::create_dir_all(&workdir).with_context(|| format!("creating {}", workdir.display()))?;
                        receipt.status = ReplayStatus::Error;
                        receipt.drift.push(DriftItem {
                            subject: "bundle_clone".to_string(),
                            expected: Some(bundle_path.clone()),
                            observed: Some("failed".to_string()),
                            severity: Severity::Error,
                        });
                        receipt.notes.push(format!("bundle clone failed: {err}"));
                    }
                }
            } else {
                can_run = false;
                fs::create_dir_all(&workdir).with_context(|| format!("creating {}", workdir.display()))?;
                receipt.status = ReplayStatus::Error;
                receipt.drift.push(DriftItem {
                    subject: "bundle".to_string(),
                    expected: Some(bundle_path.clone()),
                    observed: Some("missing".to_string()),
                    severity: Severity::Error,
                });
                receipt.notes.push("bundle path was recorded but the file is missing from the packet".to_string());
            }
        } else {
            can_run = false;
            fs::create_dir_all(&workdir).with_context(|| format!("creating {}", workdir.display()))?;
            receipt.status = ReplayStatus::Error;
            receipt.drift.push(DriftItem {
                subject: "bundle".to_string(),
                expected: Some("git/repo.bundle".to_string()),
                observed: Some("missing".to_string()),
                severity: Severity::Error,
            });
            receipt.notes.push("packet does not contain a git bundle".to_string());
        }

        if workdir.exists() && can_run {
            if let Some(patch_path) = &git.worktree_patch_path {
                let patch = materialized.root.join(patch_path);
                if patch.exists() {
                    if let Err(err) = apply_patch(&workdir, &patch) {
                        can_run = false;
                        receipt.status = ReplayStatus::Error;
                        receipt.drift.push(DriftItem {
                            subject: "worktree_patch".to_string(),
                            expected: Some(patch_path.clone()),
                            observed: Some("apply_failed".to_string()),
                            severity: Severity::Error,
                        });
                        receipt.notes.push(format!("worktree patch apply failed: {err}"));
                    }
                } else {
                    receipt.drift.push(DriftItem {
                        subject: "worktree_patch".to_string(),
                        expected: Some(patch_path.clone()),
                        observed: Some("missing".to_string()),
                        severity: Severity::Warning,
                    });
                }
            }
        }
    } else {
        can_run = false;
        fs::create_dir_all(&workdir).with_context(|| format!("creating {}", workdir.display()))?;
        receipt.status = ReplayStatus::Error;
        receipt.notes.push("packet does not contain repo state".to_string());
    }

    if workdir.exists() {
        fs::create_dir_all(&support_dir).with_context(|| format!("creating {}", support_dir.display()))?;
    }

    restore_inputs(&materialized.root, &manifest.inputs, &workdir, &mut receipt)
        .context("restoring captured inputs")?;

    compare_tool_versions(&manifest.environment.tool_versions, &mut receipt);

    if options.no_run {
        receipt.status = ReplayStatus::Skipped;
        receipt.notes.push("replay requested with --no-run".to_string());
        return finalize_receipt(&support_dir, receipt, 0);
    }

    if !can_run {
        if !matches!(receipt.status, ReplayStatus::Error) {
            receipt.status = ReplayStatus::Error;
        }
        return finalize_receipt(&support_dir, receipt, 1);
    }

    let command_cwd = manifest
        .command
        .cwd_relative_to_repo
        .as_deref()
        .and_then(|relative| {
            if relative == "." {
                Some(workdir.clone())
            } else {
                Some(workdir.join(relative))
            }
        })
        .unwrap_or_else(|| workdir.clone());

    let mut command = Command::new(&manifest.command.program);
    command
        .args(&manifest.command.args)
        .current_dir(&command_cwd);

    for (key, value) in std::env::vars() {
        command.env(&key, value);
    }
    for (key, value) in &manifest.environment.allowed_vars {
        command.env(key, value);
    }
    for (key, value) in &options.set_env {
        command.env(key, value);
    }

    let output = command.output().with_context(|| {
        format!(
            "replaying command `{}` in {}",
            manifest.command.display,
            command_cwd.display()
        )
    })?;

    let (observed_exit_code, _observed_signal) = split_status(&output.status);
    let stdout_path = support_dir.join("stdout.log");
    let stderr_path = support_dir.join("stderr.log");
    fs::write(&stdout_path, &output.stdout).context("writing replay stdout.log")?;
    fs::write(&stderr_path, &output.stderr).context("writing replay stderr.log")?;

    receipt.stdout_path = Some(
        stdout_path
            .strip_prefix(&workdir)
            .unwrap_or(&stdout_path)
            .display()
            .to_string(),
    );
    receipt.stderr_path = Some(
        stderr_path
            .strip_prefix(&workdir)
            .unwrap_or(&stderr_path)
            .display()
            .to_string(),
    );
    receipt.observed_exit_code = observed_exit_code;
    receipt.matched = receipt.observed_exit_code == receipt.recorded_exit_code;

    if receipt.matched {
        receipt.status = ReplayStatus::Matched;
    } else {
        receipt.status = ReplayStatus::Mismatched;
        receipt.drift.push(DriftItem {
            subject: "exit_code".to_string(),
            expected: receipt.recorded_exit_code.map(|code| code.to_string()),
            observed: receipt.observed_exit_code.map(|code| code.to_string()),
            severity: Severity::Error,
        });
    }

    let exit_code = receipt.observed_exit_code.unwrap_or(1);
    finalize_receipt(&support_dir, receipt, exit_code)
}

fn restore_inputs(
    packet_root: &Path,
    inputs: &[repropack_model::IndexedFile],
    workdir: &Path,
    receipt: &mut ReplayReceipt,
) -> Result<()> {
    for input in inputs {
        let Some(restore_path) = &input.restore_path else {
            continue;
        };

        let source = packet_root.join(&input.packet_path);
        let destination = workdir.join(restore_path);

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        fs::copy(&source, &destination).with_context(|| {
            format!("copying {} to {}", source.display(), destination.display())
        })?;
    }

    if inputs.iter().any(|input| input.restore_path.is_none()) {
        receipt.drift.push(DriftItem {
            subject: "input_restore".to_string(),
            expected: Some("all inputs restorable".to_string()),
            observed: Some("some inputs were inspection-only".to_string()),
            severity: Severity::Warning,
        });
    }

    Ok(())
}

fn compare_tool_versions(recorded: &BTreeMap<String, String>, receipt: &mut ReplayReceipt) {
    for (tool, expected) in recorded {
        if let Some(observed) = probe_version(tool) {
            if &observed != expected {
                receipt.drift.push(DriftItem {
                    subject: format!("tool_version:{tool}"),
                    expected: Some(expected.clone()),
                    observed: Some(observed),
                    severity: Severity::Warning,
                });
            }
        } else {
            receipt.drift.push(DriftItem {
                subject: format!("tool_version:{tool}"),
                expected: Some(expected.clone()),
                observed: Some("unavailable".to_string()),
                severity: Severity::Warning,
            });
        }
    }
}

fn finalize_receipt(support_dir: &Path, receipt: ReplayReceipt, command_exit_code: i32) -> Result<ReplayResult> {
    fs::create_dir_all(support_dir).with_context(|| format!("creating {}", support_dir.display()))?;
    let receipt_json = support_dir.join("receipt.json");
    let receipt_md = support_dir.join("receipt.md");

    receipt
        .write_to_path(&receipt_json)
        .context("writing replay receipt.json")?;
    fs::write(&receipt_md, render_receipt_markdown(&receipt)).context("writing replay receipt.md")?;

    Ok(ReplayResult {
        workdir: support_dir
            .parent()
            .unwrap_or(support_dir)
            .to_path_buf(),
        receipt_path: receipt_json,
        receipt,
        command_exit_code,
    })
}

fn probe_version(tool: &str) -> Option<String> {
    Command::new(tool)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if stdout.is_empty() {
                String::from_utf8_lossy(&output.stderr).trim().to_string()
            } else {
                stdout
            }
        })
        .filter(|value| !value.is_empty())
}

#[cfg(unix)]
fn split_status(status: &std::process::ExitStatus) -> (Option<i32>, Option<i32>) {
    use std::os::unix::process::ExitStatusExt;

    (status.code(), status.signal())
}

#[cfg(not(unix))]
fn split_status(status: &std::process::ExitStatus) -> (Option<i32>, Option<i32>) {
    (status.code(), None)
}
