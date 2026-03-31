use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use repropack_model::{GitState, Omission};

#[derive(Clone, Copy, Debug)]
pub enum BundleMode {
    Auto,
    Always,
    Never,
}

#[derive(Clone, Debug, Default)]
pub struct GitCaptureOptions {
    pub base: Option<String>,
    pub head: Option<String>,
    pub bundle_mode: Option<BundleMode>,
}

#[derive(Clone, Debug)]
pub struct GitCaptureOutcome {
    pub state: GitState,
    pub omissions: Vec<Omission>,
}

pub fn discover_repo_root(cwd: &Path) -> Result<PathBuf> {
    let root = git_output(cwd, ["rev-parse", "--show-toplevel"])
        .context("discovering repository root")?;
    Ok(PathBuf::from(root))
}

pub fn capture_git_state(cwd: &Path, out_dir: &Path, options: &GitCaptureOptions) -> Result<GitCaptureOutcome> {
    let repo_root = discover_repo_root(cwd)?;
    fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;

    let commit_sha = git_output(&repo_root, ["rev-parse", "HEAD"]).ok();
    let ref_name = git_output(&repo_root, ["symbolic-ref", "--quiet", "--short", "HEAD"]).ok();

    let dirty_output = git_output(&repo_root, ["status", "--porcelain"]).unwrap_or_default();
    let is_dirty = !dirty_output.trim().is_empty();

    let untracked_paths = git_lines(&repo_root, ["ls-files", "--others", "--exclude-standard"])
        .unwrap_or_default();

    let changed_paths = if let (Some(base), Some(head)) = (&options.base, &options.head) {
        git_lines(
            &repo_root,
            ["diff", "--name-only", &format!("{base}..{head}")],
        )
        .unwrap_or_default()
    } else {
        let mut paths = git_lines(&repo_root, ["diff", "--name-only", "HEAD"]).unwrap_or_default();
        paths.extend(untracked_paths.clone());
        paths.sort();
        paths.dedup();
        paths
    };

    fs::write(
        out_dir.join("changed-paths.txt"),
        changed_paths.join("\n"),
    )
    .context("writing changed-paths.txt")?;

    let diff_path = if let (Some(base), Some(head)) = (&options.base, &options.head) {
        let patch = git_bytes(
            &repo_root,
            ["diff", "--binary", &format!("{base}..{head}")],
        )
        .unwrap_or_default();
        if patch.is_empty() {
            None
        } else {
            let path = out_dir.join("diff.patch");
            fs::write(&path, patch).context("writing diff.patch")?;
            Some("git/diff.patch".to_string())
        }
    } else {
        None
    };

    let worktree_patch_path = if is_dirty {
        let patch = git_bytes(&repo_root, ["diff", "--binary"]).unwrap_or_default();
        if patch.is_empty() {
            None
        } else {
            let path = out_dir.join("worktree.patch");
            fs::write(&path, patch).context("writing worktree.patch")?;
            Some("git/worktree.patch".to_string())
        }
    } else {
        None
    };

    let mut omissions = Vec::new();
    let mut bundle_path = None;

    match options.bundle_mode.unwrap_or(BundleMode::Auto) {
        BundleMode::Never => {}
        BundleMode::Auto | BundleMode::Always => {
            let bundle_target = out_dir.join("repo.bundle");
            let bundle_target_string = bundle_target.to_string_lossy().to_string();
            let bundle_ref = options
                .head
                .clone()
                .or_else(|| commit_sha.clone())
                .unwrap_or_else(|| "HEAD".to_string());

            match git_status(
                &repo_root,
                [
                    "bundle",
                    "create",
                    bundle_target_string.as_str(),
                    bundle_ref.as_str(),
                ],
            ) {
                Ok(_) => {
                    bundle_path = Some("git/repo.bundle".to_string());
                }
                Err(err) => {
                    omissions.push(Omission {
                        kind: "bundle".to_string(),
                        subject: bundle_target.display().to_string(),
                        reason: format!("bundle capture failed: {err}"),
                    });
                }
            }
        }
    }

    let state = GitState {
        commit_sha,
        ref_name,
        base: options.base.clone(),
        head: options.head.clone(),
        is_dirty,
        changed_paths,
        untracked_paths,
        bundle_path,
        diff_path,
        worktree_patch_path,
    };

    fs::write(
        out_dir.join("commit.json"),
        serde_json::to_vec_pretty(&state)?,
    )
    .context("writing commit.json")?;

    Ok(GitCaptureOutcome { state, omissions })
}

pub fn clone_bundle(bundle: &Path, destination: &Path) -> Result<()> {
    let status = Command::new("git")
        .arg("clone")
        .arg(bundle)
        .arg(destination)
        .status()
        .context("running git clone for bundle")?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("git clone failed with status {status}"))
    }
}

pub fn checkout_commit(repo_root: &Path, commit: &str) -> Result<()> {
    git_status(repo_root, ["checkout", "--detach", commit])
}

pub fn apply_patch(repo_root: &Path, patch: &Path) -> Result<()> {
    let patch_string = patch.to_string_lossy().to_string();
    git_status(repo_root, ["apply", "--whitespace=nowarn", patch_string.as_str()])
}

fn git_output<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("running git {:?}", args))?;

    if !output.status.success() {
        return Err(anyhow!(
            "git {:?} failed with status {}",
            args,
            output.status
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_lines<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<Vec<String>> {
    let stdout = git_output(cwd, args)?;
    let mut lines: Vec<String> = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    lines.sort();
    lines.dedup();
    Ok(lines)
}

fn git_bytes<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("running git {:?}", args))?;

    if !output.status.success() {
        return Err(anyhow!(
            "git {:?} failed with status {}",
            args,
            output.status
        ));
    }

    Ok(output.stdout)
}

fn git_status<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<()> {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .with_context(|| format!("running git {:?}", args))?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("git {:?} failed with status {}", args, status))
    }
}
