use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use repropack_model::{CaptureDelta, GitSnapshot, GitState, Omission};

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
    let root =
        git_output(cwd, ["rev-parse", "--show-toplevel"]).context("discovering repository root")?;
    Ok(PathBuf::from(root))
}

pub fn capture_git_state(
    cwd: &Path,
    out_dir: &Path,
    options: &GitCaptureOptions,
) -> Result<GitCaptureOutcome> {
    let repo_root = discover_repo_root(cwd)?;
    fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;

    let commit_sha = git_output(&repo_root, ["rev-parse", "HEAD"]).ok();
    let ref_name = git_output(&repo_root, ["symbolic-ref", "--quiet", "--short", "HEAD"]).ok();

    let dirty_output = git_output(&repo_root, ["status", "--porcelain"]).unwrap_or_default();
    let is_dirty = !dirty_output.trim().is_empty();

    let untracked_paths =
        git_lines(&repo_root, ["ls-files", "--others", "--exclude-standard"]).unwrap_or_default();

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

    fs::write(out_dir.join("changed-paths.txt"), changed_paths.join("\n"))
        .context("writing changed-paths.txt")?;

    let diff_path = if let (Some(base), Some(head)) = (&options.base, &options.head) {
        let patch = git_bytes(&repo_root, ["diff", "--binary", &format!("{base}..{head}")])
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

            let primary_result = git_status(
                &repo_root,
                [
                    "bundle",
                    "create",
                    bundle_target_string.as_str(),
                    bundle_ref.as_str(),
                ],
            );

            match primary_result {
                Ok(_) => {
                    bundle_path = Some("git/repo.bundle".to_string());
                }
                Err(primary_err) => {
                    // Fallback: retry with --all (handles root commits where <sha> alone fails)
                    match git_status(
                        &repo_root,
                        ["bundle", "create", bundle_target_string.as_str(), "--all"],
                    ) {
                        Ok(_) => {
                            bundle_path = Some("git/repo.bundle".to_string());
                        }
                        Err(fallback_err) => {
                            omissions.push(Omission {
                                kind: "bundle".to_string(),
                                subject: bundle_target.display().to_string(),
                                reason: format!(
                                    "bundle capture failed: {primary_err}; fallback --all also failed: {fallback_err}"
                                ),
                            });
                        }
                    }
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
        git_pre: None,
        git_post: None,
        capture_delta: None,
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
    git_status(
        repo_root,
        ["apply", "--whitespace=nowarn", patch_string.as_str()],
    )
}

// ── v0.2: snapshot and delta ────────────────────────────────────────

/// Outcome of a single git snapshot capture.
#[derive(Clone, Debug)]
pub struct GitSnapshotOutcome {
    pub snapshot: GitSnapshot,
    pub omissions: Vec<Omission>,
}

/// Capture a point-in-time Git snapshot (commit SHA, dirty status, changed
/// paths, untracked paths, optional worktree patch).
///
/// `label` is typically `"pre"` or `"post"` and controls the patch filename.
///
/// When `exclude_paths` is non-empty, pathspec exclusions (`-- ':!<path>'`)
/// are appended to `git status`, `git diff`, and `git ls-files` commands so
/// that the listed paths do not appear in the snapshot.
pub fn capture_git_snapshot(
    repo_root: &Path,
    out_dir: &Path,
    label: &str,
    exclude_paths: &[&str],
) -> Result<GitSnapshotOutcome> {
    fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;

    let commit_sha = git_output(repo_root, ["rev-parse", "HEAD"]).ok();

    // Build pathspec exclusion args: ["--", ":!path1", ":!path2", ...]
    let pathspec_args: Vec<String> = if exclude_paths.is_empty() {
        Vec::new()
    } else {
        let mut args = vec!["--".to_string()];
        for path in exclude_paths {
            args.push(format!(":!{path}"));
        }
        args
    };

    let porcelain = {
        let mut args: Vec<&str> = vec!["status", "--porcelain"];
        let pathspec_refs: Vec<&str> = pathspec_args.iter().map(|s| s.as_str()).collect();
        args.extend_from_slice(&pathspec_refs);
        git_output_dynamic(repo_root, &args).unwrap_or_default()
    };
    let is_dirty = !porcelain.trim().is_empty();

    // Changed paths: tracked files with modifications relative to HEAD.
    let changed_paths = {
        let mut args: Vec<&str> = vec!["diff", "--name-only", "HEAD"];
        let pathspec_refs: Vec<&str> = pathspec_args.iter().map(|s| s.as_str()).collect();
        args.extend_from_slice(&pathspec_refs);
        let mut paths = git_lines_dynamic(repo_root, &args).unwrap_or_default();
        paths.sort();
        paths.dedup();
        paths
    };

    // Untracked paths: files not tracked by git.
    let untracked_paths = {
        let mut args: Vec<&str> = vec!["ls-files", "--others", "--exclude-standard"];
        let pathspec_refs: Vec<&str> = pathspec_args.iter().map(|s| s.as_str()).collect();
        args.extend_from_slice(&pathspec_refs);
        git_lines_dynamic(repo_root, &args).unwrap_or_default()
    };

    // Write worktree patch when dirty.
    let worktree_patch_path = if is_dirty {
        let patch_bytes = git_bytes(repo_root, ["diff", "--binary"]).unwrap_or_default();
        if patch_bytes.is_empty() {
            None
        } else {
            let filename = format!("{label}-worktree.patch");
            let disk_path = out_dir.join(&filename);
            fs::write(&disk_path, &patch_bytes)
                .with_context(|| format!("writing {}", disk_path.display()))?;
            Some(format!("git/{filename}"))
        }
    } else {
        None
    };

    let snapshot = GitSnapshot {
        commit_sha,
        is_dirty,
        changed_paths,
        untracked_paths,
        worktree_patch_path,
    };

    Ok(GitSnapshotOutcome {
        snapshot,
        omissions: Vec::new(),
    })
}

/// Compute the delta between a pre-run and post-run snapshot.
///
/// - `newly_dirty_paths`    = post.changed_paths − pre.changed_paths
/// - `newly_untracked_paths` = post.untracked_paths − pre.untracked_paths
/// - `newly_modified_paths`  = intersection(pre.changed_paths, post.changed_paths)
pub fn compute_capture_delta(pre: &GitSnapshot, post: &GitSnapshot) -> CaptureDelta {
    use std::collections::BTreeSet;

    let pre_changed: BTreeSet<&str> = pre.changed_paths.iter().map(String::as_str).collect();
    let post_changed: BTreeSet<&str> = post.changed_paths.iter().map(String::as_str).collect();

    let pre_untracked: BTreeSet<&str> = pre.untracked_paths.iter().map(String::as_str).collect();
    let post_untracked: BTreeSet<&str> = post.untracked_paths.iter().map(String::as_str).collect();

    let newly_dirty_paths: Vec<String> = post_changed
        .difference(&pre_changed)
        .map(|s| s.to_string())
        .collect();

    let newly_modified_paths: Vec<String> = post_changed
        .intersection(&pre_changed)
        .map(|s| s.to_string())
        .collect();

    let newly_untracked_paths: Vec<String> = post_untracked
        .difference(&pre_untracked)
        .map(|s| s.to_string())
        .collect();

    CaptureDelta {
        newly_dirty_paths,
        newly_modified_paths,
        newly_untracked_paths,
    }
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

fn git_output_dynamic(cwd: &Path, args: &[&str]) -> Result<String> {
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

fn git_lines_dynamic(cwd: &Path, args: &[&str]) -> Result<Vec<String>> {
    let stdout = git_output_dynamic(cwd, args)?;
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
