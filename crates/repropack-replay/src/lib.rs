use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use repropack_git::{apply_patch, checkout_commit, clone_bundle};
use repropack_model::{
    extract_semver, CaptureDelta, DriftItem, EnvClassification, IndexedFile, ReplayPolicy,
    ReplayReceipt, ReplayStatus, Severity,
};
use repropack_pack::{materialize, sha256_bytes, sha256_file};
use repropack_render::render_receipt_markdown;

#[derive(Clone, Debug, Default)]
pub struct ReplayOptions {
    pub into: Option<PathBuf>,
    pub set_env: BTreeMap<String, String>,
    pub no_run: bool,
    pub force: bool,
    pub inherit_env: bool,
    pub verbose: bool,
}

pub struct ReplayResult {
    pub workdir: PathBuf,
    pub receipt_path: PathBuf,
    pub receipt: ReplayReceipt,
    pub command_exit_code: i32,
}

// ── Pure, testable helpers ──────────────────────────────────────────

/// Build the minimal environment baseline for replay.
///
/// When `inherit_env` is false: starts from empty, injects `allowed_vars`,
/// then applies `set_env` overrides (set_env wins on conflict).
///
/// When `inherit_env` is true: starts from `host_env`, injects `allowed_vars`,
/// then applies `set_env` overrides.
///
/// Returns the final env map.
pub fn build_env_baseline(
    allowed_vars: &BTreeMap<String, String>,
    set_env: &BTreeMap<String, String>,
    host_env: &BTreeMap<String, String>,
    inherit_env: bool,
) -> BTreeMap<String, String> {
    let mut env = if inherit_env {
        host_env.clone()
    } else {
        BTreeMap::new()
    };

    // Inject manifest allowed_vars
    for (key, value) in allowed_vars {
        env.insert(key.clone(), value.clone());
    }

    // Apply set_env overrides (takes precedence)
    for (key, value) in set_env {
        env.insert(key.clone(), value.clone());
    }

    env
}

/// Classify environment variables into restored, overridden, and inherited.
///
/// - `restored`: keys in `allowed_vars` but NOT in `set_env`
/// - `overridden`: keys in both `allowed_vars` AND `set_env`
/// - `inherited`: keys from host env when `inherit_env` is true,
///   excluding keys already in `allowed_vars` or `set_env`
pub fn classify_env(
    allowed_vars: &BTreeMap<String, String>,
    set_env: &BTreeMap<String, String>,
    host_env: &BTreeMap<String, String>,
    inherit_env: bool,
) -> EnvClassification {
    let mut restored = Vec::new();
    let mut overridden = Vec::new();
    let mut inherited = Vec::new();

    for key in allowed_vars.keys() {
        if set_env.contains_key(key) {
            overridden.push(key.clone());
        } else {
            restored.push(key.clone());
        }
    }

    // Keys only in set_env (not in allowed_vars) are also overridden
    for key in set_env.keys() {
        if !allowed_vars.contains_key(key) {
            overridden.push(key.clone());
        }
    }

    if inherit_env {
        for key in host_env.keys() {
            if !allowed_vars.contains_key(key) && !set_env.contains_key(key) {
                inherited.push(key.clone());
            }
        }
    }

    restored.sort();
    overridden.sort();
    inherited.sort();

    EnvClassification {
        restored,
        overridden,
        inherited,
    }
}

/// Record drift items for excluded host environment variables.
///
/// When not inheriting env, each host var that is NOT in the final env
/// gets an info-level drift item.
pub fn compute_excluded_env_drift(
    host_env: &BTreeMap<String, String>,
    final_env: &BTreeMap<String, String>,
) -> Vec<DriftItem> {
    let mut drift = Vec::new();
    for key in host_env.keys() {
        if !final_env.contains_key(key) {
            drift.push(DriftItem {
                subject: format!("env_excluded:{key}"),
                expected: None,
                observed: None,
                severity: Severity::Info,
            });
        }
    }
    drift
}

/// Collapse per-variable env-excluded drift into a single summary item.
///
/// Returns a summary `DriftItem` with subject `env_excluded_summary` and
/// the full list of excluded key names for programmatic access.
pub fn collapse_env_excluded_drift(
    host_env: &BTreeMap<String, String>,
    final_env: &BTreeMap<String, String>,
) -> (DriftItem, Vec<String>) {
    let excluded_keys: Vec<String> = host_env
        .keys()
        .filter(|k| !final_env.contains_key(*k))
        .cloned()
        .collect();
    let count = excluded_keys.len();
    let summary = DriftItem {
        subject: "env_excluded_summary".to_string(),
        expected: None,
        observed: Some(count.to_string()),
        severity: Severity::Info,
    };
    (summary, excluded_keys)
}

/// Compare two version strings by extracting semver components.
///
/// When both strings contain a semver pattern and those patterns are equal,
/// returns true (no drift). Falls back to exact string comparison when no
/// semver pattern is found in either string.
pub fn versions_match(recorded: &str, observed: &str) -> bool {
    match (extract_semver(recorded), extract_semver(observed)) {
        (Some(r), Some(o)) => r == o,
        _ => recorded == observed,
    }
}

/// Compute evidence drift for stdout and stderr digests.
///
/// Compares observed digests against manifest-recorded digests.
/// Returns drift items for any mismatches.
pub fn compute_evidence_drift(
    manifest_stdout_sha256: Option<&str>,
    manifest_stderr_sha256: Option<&str>,
    observed_stdout: &[u8],
    observed_stderr: &[u8],
) -> Vec<DriftItem> {
    let mut drift = Vec::new();
    let observed_stdout_digest = sha256_bytes(observed_stdout);
    let observed_stderr_digest = sha256_bytes(observed_stderr);

    if let Some(expected) = manifest_stdout_sha256 {
        if expected != observed_stdout_digest {
            drift.push(DriftItem {
                subject: "stdout_digest".to_string(),
                expected: Some(expected.to_string()),
                observed: Some(observed_stdout_digest.clone()),
                severity: Severity::Warning,
            });
        }
    }

    if let Some(expected) = manifest_stderr_sha256 {
        if expected != observed_stderr_digest {
            drift.push(DriftItem {
                subject: "stderr_digest".to_string(),
                expected: Some(expected.to_string()),
                observed: Some(observed_stderr_digest.clone()),
                severity: Severity::Warning,
            });
        }
    }

    drift
}

/// Compute output artifact drift by comparing digests.
///
/// For each output in the manifest, recompute the digest at `restore_path`
/// relative to `workdir`. Returns (drift_items, all_matched).
pub fn compute_output_drift(outputs: &[IndexedFile], workdir: &Path) -> (Vec<DriftItem>, bool) {
    let mut drift = Vec::new();
    let mut all_matched = true;

    for output in outputs {
        let Some(restore_path) = &output.restore_path else {
            continue;
        };

        let full_path = workdir.join(restore_path);
        if !full_path.exists() {
            drift.push(DriftItem {
                subject: format!("output_missing:{}", output.packet_path),
                expected: Some("present".to_string()),
                observed: Some("missing".to_string()),
                severity: Severity::Error,
            });
            all_matched = false;
            continue;
        }

        match sha256_file(&full_path) {
            Ok(observed_digest) => {
                if observed_digest != output.sha256 {
                    drift.push(DriftItem {
                        subject: format!("output_digest:{}", output.packet_path),
                        expected: Some(output.sha256.clone()),
                        observed: Some(observed_digest),
                        severity: Severity::Warning,
                    });
                    all_matched = false;
                }
            }
            Err(_) => {
                drift.push(DriftItem {
                    subject: format!("output_missing:{}", output.packet_path),
                    expected: Some("readable".to_string()),
                    observed: Some("read_error".to_string()),
                    severity: Severity::Error,
                });
                all_matched = false;
            }
        }
    }

    (drift, all_matched)
}

/// Determine final match status considering exit code AND evidence.
///
/// Returns (matched, status, optional_note).
/// - `matched` is true only when exit code matches AND all evidence matches.
/// - When exit codes match but evidence diverges, status is Mismatched with a note.
pub fn determine_match_status(
    exit_code_matched: bool,
    evidence_all_matched: bool,
) -> (bool, ReplayStatus, Option<String>) {
    match (exit_code_matched, evidence_all_matched) {
        (true, true) => (true, ReplayStatus::Matched, None),
        (true, false) => (
            false,
            ReplayStatus::Mismatched,
            Some("exit code matched but evidence diverged".to_string()),
        ),
        (false, _) => (false, ReplayStatus::Mismatched, None),
    }
}

/// Compare two capture deltas. Returns a drift item if they differ.
pub fn compare_capture_deltas(
    manifest_delta: &CaptureDelta,
    replay_delta: &CaptureDelta,
) -> Option<DriftItem> {
    if manifest_delta != replay_delta {
        Some(DriftItem {
            subject: "capture_delta".to_string(),
            expected: Some(format!("{manifest_delta:?}")),
            observed: Some(format!("{replay_delta:?}")),
            severity: Severity::Warning,
        })
    } else {
        None
    }
}

// ── Replay orchestration ────────────────────────────────────────────

pub fn replay(packet: &Path, options: &ReplayOptions) -> Result<ReplayResult> {
    let materialized =
        materialize(packet).with_context(|| format!("materializing {}", packet.display()))?;
    let manifest = repropack_model::PacketManifest::read_from_path(&materialized.manifest_path())
        .context("reading manifest from packet")?;

    let workdir = options
        .into
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("repropack-replay-{}", manifest.packet_id)));

    if workdir.exists() {
        return Err(anyhow!(
            "replay target already exists: {}",
            workdir.display()
        ));
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
            fs::create_dir_all(&support_dir)
                .with_context(|| format!("creating {}", support_dir.display()))?;
            receipt.status = ReplayStatus::Blocked;
            receipt
                .notes
                .push("packet replay policy is disabled".to_string());
            return finalize_receipt(&support_dir, receipt, 0);
        }
        ReplayPolicy::Confirm if !options.force => {
            fs::create_dir_all(&support_dir)
                .with_context(|| format!("creating {}", support_dir.display()))?;
            receipt.status = ReplayStatus::Blocked;
            receipt
                .notes
                .push("packet replay policy requires --force".to_string());
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
                        fs::create_dir_all(&workdir)
                            .with_context(|| format!("creating {}", workdir.display()))?;
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
                fs::create_dir_all(&workdir)
                    .with_context(|| format!("creating {}", workdir.display()))?;
                receipt.status = ReplayStatus::Error;
                receipt.drift.push(DriftItem {
                    subject: "bundle".to_string(),
                    expected: Some(bundle_path.clone()),
                    observed: Some("missing".to_string()),
                    severity: Severity::Error,
                });
                receipt.notes.push(
                    "bundle path was recorded but the file is missing from the packet".to_string(),
                );
            }
        } else {
            can_run = false;
            fs::create_dir_all(&workdir)
                .with_context(|| format!("creating {}", workdir.display()))?;
            receipt.status = ReplayStatus::Error;
            receipt.drift.push(DriftItem {
                subject: "bundle".to_string(),
                expected: Some("git/repo.bundle".to_string()),
                observed: Some("missing".to_string()),
                severity: Severity::Error,
            });
            receipt
                .notes
                .push("packet does not contain a git bundle".to_string());
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
                        receipt
                            .notes
                            .push(format!("worktree patch apply failed: {err}"));
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
        receipt
            .notes
            .push("packet does not contain repo state".to_string());
    }

    if workdir.exists() {
        fs::create_dir_all(&support_dir)
            .with_context(|| format!("creating {}", support_dir.display()))?;
    }

    restore_inputs(&materialized.root, &manifest.inputs, &workdir, &mut receipt)
        .context("restoring captured inputs")?;

    compare_tool_versions(&manifest.environment.tool_versions, &mut receipt);

    if options.no_run {
        receipt.status = ReplayStatus::Skipped;
        receipt
            .notes
            .push("replay requested with --no-run".to_string());
        return finalize_receipt(&support_dir, receipt, 0);
    }

    if !can_run {
        if !matches!(receipt.status, ReplayStatus::Error) {
            receipt.status = ReplayStatus::Error;
        }
        return finalize_receipt(&support_dir, receipt, 1);
    }

    // ── Build environment (Task 10.1, 10.2, 10.3) ──────────────────

    let host_env: BTreeMap<String, String> = std::env::vars().collect();

    let final_env = build_env_baseline(
        &manifest.environment.allowed_vars,
        &options.set_env,
        &host_env,
        options.inherit_env,
    );

    let env_classification = classify_env(
        &manifest.environment.allowed_vars,
        &options.set_env,
        &host_env,
        options.inherit_env,
    );
    receipt.env_classification = Some(env_classification);

    // Record inherit-env warning drift
    if options.inherit_env {
        receipt.drift.push(DriftItem {
            subject: "env_inherited".to_string(),
            expected: None,
            observed: None,
            severity: Severity::Warning,
        });
    } else {
        // Record excluded host vars as collapsed summary drift
        let (summary_drift, excluded_keys) = collapse_env_excluded_drift(&host_env, &final_env);
        receipt.drift.push(summary_drift);
        receipt.env_excluded_keys = Some(excluded_keys);
    }

    // ── Capture pre-run snapshot for delta comparison (Task 10.7) ───

    let pre_snapshot = manifest
        .git
        .as_ref()
        .and_then(|g| g.capture_delta.as_ref())
        .and_then(|_| {
            repropack_git::capture_git_snapshot(
                &workdir,
                &support_dir,
                "replay-pre",
                &[".repropack-replay"],
            )
            .ok()
            .map(|outcome| outcome.snapshot)
        });

    // ── Execute command ─────────────────────────────────────────────

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

    // Task 10.1: Use env_clear + minimal baseline instead of host inheritance
    command.env_clear();
    for (key, value) in &final_env {
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

    // ── Evidence digest comparison (Task 10.4) ──────────────────────

    let evidence_drift = compute_evidence_drift(
        manifest.execution.stdout_sha256.as_deref(),
        manifest.execution.stderr_sha256.as_deref(),
        &output.stdout,
        &output.stderr,
    );
    let evidence_matched = evidence_drift.is_empty();
    receipt.drift.extend(evidence_drift);

    // ── Output artifact digest comparison (Task 10.5) ───────────────

    let (output_drift, outputs_matched) = compute_output_drift(&manifest.outputs, &workdir);
    receipt.drift.extend(output_drift);
    if !manifest.outputs.is_empty() {
        receipt.matched_outputs = Some(outputs_matched);
    }

    // ── Capture delta drift comparison (Task 10.7) ──────────────────

    let mut delta_matched = true;
    if let Some(manifest_delta) = manifest.git.as_ref().and_then(|g| g.capture_delta.as_ref()) {
        let post_snapshot = repropack_git::capture_git_snapshot(
            &workdir,
            &support_dir,
            "replay-post",
            &[".repropack-replay"],
        )
        .ok()
        .map(|outcome| outcome.snapshot);

        if let (Some(pre), Some(post)) = (&pre_snapshot, &post_snapshot) {
            let replay_delta = repropack_git::compute_capture_delta(pre, post);
            if let Some(drift_item) = compare_capture_deltas(manifest_delta, &replay_delta) {
                delta_matched = false;
                receipt.drift.push(drift_item);
            }
        }
    }

    // ── Determine final match status (Task 10.6) ────────────────────

    let exit_code_matched = receipt.observed_exit_code == receipt.recorded_exit_code;
    let all_evidence_matched = evidence_matched && outputs_matched && delta_matched;

    if !exit_code_matched {
        receipt.drift.push(DriftItem {
            subject: "exit_code".to_string(),
            expected: receipt.recorded_exit_code.map(|code| code.to_string()),
            observed: receipt.observed_exit_code.map(|code| code.to_string()),
            severity: Severity::Error,
        });
    }

    let (matched, status, note) = determine_match_status(exit_code_matched, all_evidence_matched);
    receipt.matched = matched;
    receipt.status = status;
    if let Some(note) = note {
        receipt.notes.push(note);
    }

    let exit_code = receipt.observed_exit_code.unwrap_or(1);
    finalize_receipt(&support_dir, receipt, exit_code)
}

// ── Internal helpers ────────────────────────────────────────────────

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
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
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
            if !versions_match(expected, &observed) {
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

fn finalize_receipt(
    support_dir: &Path,
    receipt: ReplayReceipt,
    command_exit_code: i32,
) -> Result<ReplayResult> {
    fs::create_dir_all(support_dir)
        .with_context(|| format!("creating {}", support_dir.display()))?;
    let receipt_json = support_dir.join("receipt.json");
    let receipt_md = support_dir.join("receipt.md");

    receipt
        .write_to_path(&receipt_json)
        .context("writing replay receipt.json")?;
    fs::write(&receipt_md, render_receipt_markdown(&receipt))
        .context("writing replay receipt.md")?;

    Ok(ReplayResult {
        workdir: support_dir.parent().unwrap_or(support_dir).to_path_buf(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_env_baseline_no_inherit() {
        let mut allowed = BTreeMap::new();
        allowed.insert("CI".to_string(), "true".to_string());
        allowed.insert("RUST_LOG".to_string(), "debug".to_string());

        let mut set_env = BTreeMap::new();
        set_env.insert("RUST_LOG".to_string(), "info".to_string());

        let mut host = BTreeMap::new();
        host.insert("HOME".to_string(), "/home/user".to_string());
        host.insert("PATH".to_string(), "/usr/bin".to_string());

        let result = build_env_baseline(&allowed, &set_env, &host, false);
        assert_eq!(result.len(), 2);
        assert_eq!(result["CI"], "true");
        assert_eq!(result["RUST_LOG"], "info"); // set_env wins
        assert!(!result.contains_key("HOME"));
        assert!(!result.contains_key("PATH"));
    }

    #[test]
    fn build_env_baseline_with_inherit() {
        let mut allowed = BTreeMap::new();
        allowed.insert("CI".to_string(), "true".to_string());

        let set_env = BTreeMap::new();

        let mut host = BTreeMap::new();
        host.insert("HOME".to_string(), "/home/user".to_string());

        let result = build_env_baseline(&allowed, &set_env, &host, true);
        assert_eq!(result.len(), 2);
        assert_eq!(result["CI"], "true");
        assert_eq!(result["HOME"], "/home/user");
    }

    #[test]
    fn classify_env_basic() {
        let mut allowed = BTreeMap::new();
        allowed.insert("CI".to_string(), "true".to_string());
        allowed.insert("RUST_LOG".to_string(), "debug".to_string());

        let mut set_env = BTreeMap::new();
        set_env.insert("RUST_LOG".to_string(), "info".to_string());

        let host = BTreeMap::new();

        let cls = classify_env(&allowed, &set_env, &host, false);
        assert_eq!(cls.restored, vec!["CI"]);
        assert_eq!(cls.overridden, vec!["RUST_LOG"]);
        assert!(cls.inherited.is_empty());
    }

    #[test]
    fn classify_env_with_inherit() {
        let allowed = BTreeMap::new();
        let set_env = BTreeMap::new();

        let mut host = BTreeMap::new();
        host.insert("HOME".to_string(), "/home/user".to_string());

        let cls = classify_env(&allowed, &set_env, &host, true);
        assert!(cls.restored.is_empty());
        assert!(cls.overridden.is_empty());
        assert_eq!(cls.inherited, vec!["HOME"]);
    }

    #[test]
    fn evidence_drift_no_mismatch() {
        let drift = compute_evidence_drift(
            Some(&sha256_bytes(b"hello")),
            Some(&sha256_bytes(b"world")),
            b"hello",
            b"world",
        );
        assert!(drift.is_empty());
    }

    #[test]
    fn evidence_drift_stdout_mismatch() {
        let drift = compute_evidence_drift(Some(&sha256_bytes(b"hello")), None, b"different", b"");
        assert_eq!(drift.len(), 1);
        assert_eq!(drift[0].subject, "stdout_digest");
    }

    #[test]
    fn output_drift_missing_file() {
        let outputs = vec![IndexedFile {
            original_path: "result.bin".to_string(),
            restore_path: Some("result.bin".to_string()),
            packet_path: "outputs/files/result.bin".to_string(),
            sha256: "abc123".to_string(),
            size_bytes: 100,
        }];

        let dir = tempfile::tempdir().unwrap();
        let (drift, matched) = compute_output_drift(&outputs, dir.path());
        assert!(!matched);
        assert_eq!(drift.len(), 1);
        assert!(drift[0].subject.starts_with("output_missing:"));
    }

    #[test]
    fn output_drift_matching_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("result.bin"), b"content").unwrap();
        let digest = sha256_bytes(b"content");

        let outputs = vec![IndexedFile {
            original_path: "result.bin".to_string(),
            restore_path: Some("result.bin".to_string()),
            packet_path: "outputs/files/result.bin".to_string(),
            sha256: digest,
            size_bytes: 7,
        }];

        let (drift, matched) = compute_output_drift(&outputs, dir.path());
        assert!(matched);
        assert!(drift.is_empty());
    }

    #[test]
    fn determine_match_both_match() {
        let (matched, status, note) = determine_match_status(true, true);
        assert!(matched);
        assert!(matches!(status, ReplayStatus::Matched));
        assert!(note.is_none());
    }

    #[test]
    fn determine_match_exit_match_evidence_diverge() {
        let (matched, status, note) = determine_match_status(true, false);
        assert!(!matched);
        assert!(matches!(status, ReplayStatus::Mismatched));
        assert_eq!(note.unwrap(), "exit code matched but evidence diverged");
    }

    #[test]
    fn determine_match_exit_mismatch() {
        let (matched, status, note) = determine_match_status(false, true);
        assert!(!matched);
        assert!(matches!(status, ReplayStatus::Mismatched));
        assert!(note.is_none());
    }

    #[test]
    fn compare_deltas_equal() {
        let delta = CaptureDelta {
            newly_dirty_paths: vec!["a.rs".to_string()],
            newly_modified_paths: vec![],
            newly_untracked_paths: vec![],
        };
        assert!(compare_capture_deltas(&delta, &delta).is_none());
    }

    #[test]
    fn compare_deltas_different() {
        let a = CaptureDelta {
            newly_dirty_paths: vec!["a.rs".to_string()],
            newly_modified_paths: vec![],
            newly_untracked_paths: vec![],
        };
        let b = CaptureDelta {
            newly_dirty_paths: vec!["b.rs".to_string()],
            newly_modified_paths: vec![],
            newly_untracked_paths: vec![],
        };
        let drift = compare_capture_deltas(&a, &b);
        assert!(drift.is_some());
        assert_eq!(drift.unwrap().subject, "capture_delta");
    }

    #[test]
    fn excluded_env_drift_records_info() {
        let mut host = BTreeMap::new();
        host.insert("SECRET".to_string(), "val".to_string());
        host.insert("CI".to_string(), "true".to_string());

        let mut final_env = BTreeMap::new();
        final_env.insert("CI".to_string(), "true".to_string());

        let drift = compute_excluded_env_drift(&host, &final_env);
        assert_eq!(drift.len(), 1);
        assert_eq!(drift[0].subject, "env_excluded:SECRET");
        assert!(matches!(drift[0].severity, Severity::Info));
    }

    #[test]
    fn collapse_env_excluded_drift_summary() {
        let mut host = BTreeMap::new();
        host.insert("SECRET".to_string(), "val".to_string());
        host.insert("HOME".to_string(), "/home/user".to_string());
        host.insert("CI".to_string(), "true".to_string());

        let mut final_env = BTreeMap::new();
        final_env.insert("CI".to_string(), "true".to_string());

        let (summary, keys) = collapse_env_excluded_drift(&host, &final_env);
        assert_eq!(summary.subject, "env_excluded_summary");
        assert_eq!(summary.observed, Some("2".to_string()));
        assert!(matches!(summary.severity, Severity::Info));
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"SECRET".to_string()));
        assert!(keys.contains(&"HOME".to_string()));
    }

    #[test]
    fn collapse_env_excluded_drift_none_excluded() {
        let mut host = BTreeMap::new();
        host.insert("CI".to_string(), "true".to_string());

        let mut final_env = BTreeMap::new();
        final_env.insert("CI".to_string(), "true".to_string());

        let (summary, keys) = collapse_env_excluded_drift(&host, &final_env);
        assert_eq!(summary.observed, Some("0".to_string()));
        assert!(keys.is_empty());
    }

    #[test]
    fn versions_match_same_semver_different_prefix() {
        assert!(versions_match(
            "rustc 1.78.0 (9b00956e5 2024-04-29)",
            "rustc 1.78.0 (abc123 2024-05-01)"
        ));
    }

    #[test]
    fn versions_match_different_semver() {
        assert!(!versions_match(
            "rustc 1.78.0 (9b00956e5 2024-04-29)",
            "rustc 1.79.0 (def456 2024-06-01)"
        ));
    }

    #[test]
    fn versions_match_no_semver_exact() {
        assert!(versions_match("some-tool v2", "some-tool v2"));
    }

    #[test]
    fn versions_match_no_semver_different() {
        assert!(!versions_match("some-tool v2", "some-tool v3"));
    }

    #[test]
    fn versions_match_bare_semver() {
        assert!(versions_match("1.2.3", "1.2.3"));
        assert!(!versions_match("1.2.3", "1.2.4"));
    }

    #[test]
    fn replay_options_default_verbose_false() {
        let opts = ReplayOptions::default();
        assert!(!opts.verbose);
    }
}
