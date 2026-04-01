use std::fmt::Write;

use repropack_model::{
    DoctorReadiness, DoctorReport, DriftItem, Omission, PacketManifest, ReplayReceipt, ReplayStatus,
};

pub fn render_manifest_markdown(manifest: &PacketManifest) -> String {
    let mut out = String::new();

    writeln!(&mut out, "# ReproPack summary").unwrap();
    writeln!(&mut out).unwrap();
    writeln!(&mut out, "- Packet: `{}`", manifest.packet_id).unwrap();
    if let Some(name) = &manifest.packet_name {
        writeln!(&mut out, "- Name: `{}`", name).unwrap();
    }
    writeln!(&mut out, "- Created: `{}`", manifest.created_at).unwrap();
    writeln!(
        &mut out,
        "- Capture level: `{}`",
        display_capture_level(manifest)
    )
    .unwrap();
    writeln!(
        &mut out,
        "- Replay fidelity: `{}`",
        display_replay_fidelity(manifest)
    )
    .unwrap();
    writeln!(
        &mut out,
        "- Replay policy: `{}`",
        display_replay_policy(manifest)
    )
    .unwrap();
    writeln!(&mut out).unwrap();

    writeln!(&mut out, "## Command").unwrap();
    writeln!(&mut out).unwrap();
    writeln!(&mut out, "```text").unwrap();
    writeln!(&mut out, "{}", manifest.command.display).unwrap();
    writeln!(&mut out, "```").unwrap();
    writeln!(&mut out).unwrap();
    writeln!(&mut out, "- cwd: `{}`", manifest.command.cwd).unwrap();
    if let Some(relative) = &manifest.command.cwd_relative_to_repo {
        writeln!(&mut out, "- cwd relative to repo: `{}`", relative).unwrap();
    }
    writeln!(&mut out).unwrap();

    writeln!(&mut out, "## Execution").unwrap();
    writeln!(&mut out).unwrap();
    writeln!(&mut out, "- success: `{}`", manifest.execution.success).unwrap();
    writeln!(
        &mut out,
        "- exit code: `{}`",
        format_option_i32(manifest.execution.exit_code)
    )
    .unwrap();
    writeln!(
        &mut out,
        "- signal: `{}`",
        format_option_i32(manifest.execution.signal)
    )
    .unwrap();
    writeln!(
        &mut out,
        "- duration ms: `{}`",
        manifest.execution.duration_ms
    )
    .unwrap();
    writeln!(&mut out, "- started: `{}`", manifest.execution.started_at).unwrap();
    writeln!(&mut out, "- finished: `{}`", manifest.execution.finished_at).unwrap();
    if let Some(spawn_error) = &manifest.execution.spawn_error {
        writeln!(&mut out, "- spawn error: `{}`", spawn_error).unwrap();
    }
    writeln!(&mut out).unwrap();

    if let Some(git) = &manifest.git {
        writeln!(&mut out, "## Git").unwrap();
        writeln!(&mut out).unwrap();
        writeln!(
            &mut out,
            "- commit: `{}`",
            git.commit_sha.as_deref().unwrap_or("n/a")
        )
        .unwrap();
        writeln!(
            &mut out,
            "- ref: `{}`",
            git.ref_name.as_deref().unwrap_or("n/a")
        )
        .unwrap();
        writeln!(&mut out, "- dirty: `{}`", git.is_dirty).unwrap();
        writeln!(&mut out, "- changed paths: `{}`", git.changed_paths.len()).unwrap();
        writeln!(
            &mut out,
            "- untracked paths: `{}`",
            git.untracked_paths.len()
        )
        .unwrap();
        writeln!(
            &mut out,
            "- bundle: `{}`",
            git.bundle_path.as_deref().unwrap_or("none")
        )
        .unwrap();
        writeln!(
            &mut out,
            "- diff: `{}`",
            git.diff_path.as_deref().unwrap_or("none")
        )
        .unwrap();
        writeln!(
            &mut out,
            "- worktree patch: `{}`",
            git.worktree_patch_path.as_deref().unwrap_or("none")
        )
        .unwrap();
        writeln!(&mut out).unwrap();
    }

    writeln!(&mut out, "## Environment").unwrap();
    writeln!(&mut out).unwrap();
    writeln!(
        &mut out,
        "- platform: `{}/{}/{}`",
        manifest.environment.platform.family,
        manifest.environment.platform.os,
        manifest.environment.platform.arch
    )
    .unwrap();
    writeln!(
        &mut out,
        "- allowed env vars: `{}`",
        manifest.environment.allowed_vars.len()
    )
    .unwrap();
    writeln!(
        &mut out,
        "- redacted env vars: `{}`",
        manifest.environment.redacted_keys.len()
    )
    .unwrap();
    writeln!(
        &mut out,
        "- tool versions: `{}`",
        manifest.environment.tool_versions.len()
    )
    .unwrap();
    writeln!(&mut out).unwrap();

    writeln!(&mut out, "## Inputs and outputs").unwrap();
    writeln!(&mut out).unwrap();
    writeln!(&mut out, "- inputs: `{}`", manifest.inputs.len()).unwrap();
    writeln!(&mut out, "- outputs: `{}`", manifest.outputs.len()).unwrap();
    writeln!(&mut out).unwrap();

    if !manifest.omissions.is_empty() {
        writeln!(&mut out, "## Omissions").unwrap();
        writeln!(&mut out).unwrap();
        for omission in &manifest.omissions {
            render_omission_markdown(&mut out, omission);
        }
        writeln!(&mut out).unwrap();
    }

    if !manifest.notes.is_empty() {
        writeln!(&mut out, "## Notes").unwrap();
        writeln!(&mut out).unwrap();
        for note in &manifest.notes {
            writeln!(&mut out, "- {}", note).unwrap();
        }
    }

    out
}

pub fn render_manifest_html(manifest: &PacketManifest) -> String {
    let summary_md = render_manifest_markdown(manifest);
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>ReproPack summary</title>\
         <style>body{{font-family:system-ui,sans-serif;max-width:960px;margin:2rem auto;padding:0 1rem;line-height:1.45}}\
         code,pre{{font-family:ui-monospace,SFMono-Regular,Consolas,monospace}}pre{{background:#f5f5f5;padding:1rem;border-radius:8px;overflow:auto}}\
         h1,h2{{line-height:1.2}}</style></head><body><pre>{}</pre></body></html>",
        escape_html(&summary_md)
    )
}

pub fn render_receipt_markdown(receipt: &ReplayReceipt) -> String {
    let mut out = String::new();

    writeln!(&mut out, "# ReproPack replay receipt").unwrap();
    writeln!(&mut out).unwrap();
    writeln!(&mut out, "- Packet: `{}`", receipt.packet_id).unwrap();
    writeln!(&mut out, "- Replayed at: `{}`", receipt.replayed_at).unwrap();
    writeln!(&mut out, "- Workdir: `{}`", receipt.workdir).unwrap();
    writeln!(
        &mut out,
        "- Status: `{}`",
        display_replay_status(&receipt.status)
    )
    .unwrap();
    writeln!(
        &mut out,
        "- Recorded exit code: `{}`",
        format_option_i32(receipt.recorded_exit_code)
    )
    .unwrap();
    writeln!(
        &mut out,
        "- Observed exit code: `{}`",
        format_option_i32(receipt.observed_exit_code)
    )
    .unwrap();
    writeln!(&mut out, "- Matched: `{}`", receipt.matched).unwrap();
    if let Some(matched_outputs) = receipt.matched_outputs {
        writeln!(&mut out, "- Matched outputs: `{}`", matched_outputs).unwrap();
    }
    writeln!(&mut out).unwrap();

    writeln!(&mut out, "## Command").unwrap();
    writeln!(&mut out).unwrap();
    writeln!(&mut out, "```text").unwrap();
    writeln!(&mut out, "{}", receipt.command_display).unwrap();
    writeln!(&mut out, "```").unwrap();
    writeln!(&mut out).unwrap();

    if !receipt.drift.is_empty() {
        writeln!(&mut out, "## Drift").unwrap();
        writeln!(&mut out).unwrap();
        for drift in &receipt.drift {
            if drift.subject == "env_excluded_summary" {
                let count = drift.observed.as_deref().unwrap_or("0");
                writeln!(
                    &mut out,
                    "- [{}] {} host environment variables excluded",
                    display_severity(&drift.severity),
                    count
                )
                .unwrap();
            } else {
                writeln!(
                    &mut out,
                    "- [{}] {} | expected=`{}` observed=`{}`",
                    display_severity(&drift.severity),
                    drift.subject,
                    drift.expected.as_deref().unwrap_or("n/a"),
                    drift.observed.as_deref().unwrap_or("n/a")
                )
                .unwrap();
            }
        }
        writeln!(&mut out).unwrap();
    }

    if let Some(env_cls) = &receipt.env_classification {
        writeln!(&mut out, "## Environment classification").unwrap();
        writeln!(&mut out).unwrap();
        writeln!(&mut out, "- Restored: `{}` keys", env_cls.restored.len()).unwrap();
        if !env_cls.restored.is_empty() {
            for key in &env_cls.restored {
                writeln!(&mut out, "  - `{}`", key).unwrap();
            }
        }
        writeln!(
            &mut out,
            "- Overridden: `{}` keys",
            env_cls.overridden.len()
        )
        .unwrap();
        if !env_cls.overridden.is_empty() {
            for key in &env_cls.overridden {
                writeln!(&mut out, "  - `{}`", key).unwrap();
            }
        }
        writeln!(&mut out, "- Inherited: `{}` keys", env_cls.inherited.len()).unwrap();
        if !env_cls.inherited.is_empty() {
            for key in &env_cls.inherited {
                writeln!(&mut out, "  - `{}`", key).unwrap();
            }
        }
        writeln!(&mut out).unwrap();
    }

    if !receipt.notes.is_empty() {
        writeln!(&mut out, "## Notes").unwrap();
        writeln!(&mut out).unwrap();
        for note in &receipt.notes {
            writeln!(&mut out, "- {}", note).unwrap();
        }
    }

    out
}

/// Render a `DoctorReport` as human-readable text.
pub fn render_doctor_text(report: &DoctorReport) -> String {
    let mut out = String::new();

    // Readiness status
    let readiness = match report.readiness {
        DoctorReadiness::Ready => "ready",
        DoctorReadiness::Degraded => "degraded",
        DoctorReadiness::Blocked => "blocked",
    };
    writeln!(&mut out, "Readiness: {}", readiness).unwrap();
    writeln!(&mut out).unwrap();

    // Omission groups
    if !report.omissions_by_kind.is_empty() {
        writeln!(&mut out, "Omissions:").unwrap();
        for (kind, omissions) in &report.omissions_by_kind {
            writeln!(&mut out, "  {} ({})", kind, omissions.len()).unwrap();
            for omission in omissions {
                writeln!(&mut out, "    - {}: {}", omission.subject, omission.reason).unwrap();
            }
        }
        writeln!(&mut out).unwrap();
    }

    // Redacted env keys
    if !report.redacted_env_keys.is_empty() {
        writeln!(&mut out, "Redacted environment keys:").unwrap();
        for key in &report.redacted_env_keys {
            writeln!(&mut out, "  - {}", key).unwrap();
        }
        writeln!(&mut out).unwrap();
    }

    // Tool versions
    if !report.tool_versions.is_empty() {
        writeln!(&mut out, "Tool versions:").unwrap();
        for (tool, version) in &report.tool_versions {
            let missing_marker = if report.missing_tools.contains(tool) {
                " [MISSING]"
            } else {
                ""
            };
            writeln!(&mut out, "  - {}: {}{}", tool, version, missing_marker).unwrap();
        }
        writeln!(&mut out).unwrap();
    }

    // Flag missing tools not already listed in tool_versions
    let extra_missing: Vec<_> = report
        .missing_tools
        .iter()
        .filter(|t| !report.tool_versions.contains_key(t.as_str()))
        .collect();
    if !extra_missing.is_empty() {
        writeln!(&mut out, "Missing tools:").unwrap();
        for tool in extra_missing {
            writeln!(&mut out, "  - {}", tool).unwrap();
        }
        writeln!(&mut out).unwrap();
    }

    // Redaction summary
    if let Some(summary) = &report.redaction_summary {
        writeln!(&mut out, "Redaction summary:").unwrap();
        writeln!(&mut out, "  Replaced values: {}", summary.replaced_values).unwrap();
        writeln!(&mut out, "  Removed files: {}", summary.removed_files).unwrap();
        if report.has_redaction_report {
            writeln!(
                &mut out,
                "  This packet was scrubbed and is not replayable."
            )
            .unwrap();
        }
        writeln!(&mut out).unwrap();
    } else if report.has_redaction_report {
        writeln!(&mut out, "This packet was scrubbed and is not replayable.").unwrap();
        writeln!(&mut out).unwrap();
    }

    // Notes
    if !report.notes.is_empty() {
        writeln!(&mut out, "Notes:").unwrap();
        for note in &report.notes {
            writeln!(&mut out, "  - {}", note).unwrap();
        }
    }

    out
}

/// Render a `DoctorReport` as JSON.
pub fn render_doctor_json(report: &DoctorReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
}

/// Render human-readable explanations for each `DriftItem` in a receipt.
pub fn render_explain_output(receipt: &ReplayReceipt) -> String {
    let mut out = String::new();

    writeln!(
        &mut out,
        "Replay explanation (status: {})",
        display_replay_status(&receipt.status)
    )
    .unwrap();
    writeln!(&mut out).unwrap();

    if receipt.drift.is_empty() {
        writeln!(&mut out, "No drift items recorded.").unwrap();
        return out;
    }

    for (i, drift) in receipt.drift.iter().enumerate() {
        write!(&mut out, "{}. ", i + 1).unwrap();
        render_drift_explanation(&mut out, drift);
    }

    out
}

fn render_drift_explanation(out: &mut String, drift: &DriftItem) {
    let subject = drift.subject.as_str();
    let expected = drift.expected.as_deref().unwrap_or("n/a");
    let observed = drift.observed.as_deref().unwrap_or("n/a");

    if subject == "stdout_digest" || subject == "stderr_digest" {
        let stream = if subject == "stdout_digest" {
            "stdout"
        } else {
            "stderr"
        };
        writeln!(out, "Command produced different {} output", stream).unwrap();
        writeln!(out, "   expected digest: {}", expected).unwrap();
        writeln!(out, "   observed digest: {}", observed).unwrap();
    } else if let Some(path) = subject.strip_prefix("output_digest:") {
        writeln!(out, "Output file changed: {}", path).unwrap();
        writeln!(out, "   expected digest: {}", expected).unwrap();
        writeln!(out, "   observed digest: {}", observed).unwrap();
    } else if let Some(path) = subject.strip_prefix("output_missing:") {
        writeln!(out, "Expected output file not found: {}", path).unwrap();
    } else if subject == "capture_delta" {
        writeln!(out, "Repository side effects differ").unwrap();
        writeln!(out, "   expected: {}", expected).unwrap();
        writeln!(out, "   observed: {}", observed).unwrap();
    } else if let Some(tool) = subject.strip_prefix("tool_version:") {
        writeln!(out, "Tool version mismatch: {}", tool).unwrap();
        writeln!(out, "   expected: {}", expected).unwrap();
        writeln!(out, "   observed: {}", observed).unwrap();
    } else if subject == "env_excluded_summary" {
        writeln!(out, "{} host environment variables excluded", observed).unwrap();
    } else {
        writeln!(
            out,
            "[{}] {} | expected={} observed={}",
            display_severity(&drift.severity),
            subject,
            expected,
            observed
        )
        .unwrap();
    }
}

/// Render a GitHub Actions job summary for a packet (and optional receipt).
pub fn render_gh_summary(manifest: &PacketManifest, receipt: Option<&ReplayReceipt>) -> String {
    let mut out = String::new();

    writeln!(&mut out, "## ReproPack Summary").unwrap();
    writeln!(&mut out).unwrap();

    // Packet name or ID
    if let Some(name) = &manifest.packet_name {
        writeln!(&mut out, "**Packet:** {}", name).unwrap();
    } else {
        writeln!(&mut out, "**Packet:** `{}`", manifest.packet_id).unwrap();
    }

    // Commit SHA
    if let Some(git) = &manifest.git {
        if let Some(sha) = &git.commit_sha {
            writeln!(&mut out, "**Commit:** `{}`", sha).unwrap();
        }
    }

    // Command
    writeln!(&mut out, "**Command:** `{}`", manifest.command.display).unwrap();

    // Exit code
    writeln!(
        &mut out,
        "**Exit code:** `{}`",
        format_option_i32(manifest.execution.exit_code)
    )
    .unwrap();

    // Replay fidelity
    writeln!(
        &mut out,
        "**Replay fidelity:** `{}`",
        display_replay_fidelity(manifest)
    )
    .unwrap();

    // Omission count
    writeln!(&mut out, "**Omissions:** {}", manifest.omissions.len()).unwrap();

    // Receipt info
    if let Some(receipt) = receipt {
        writeln!(&mut out).unwrap();
        writeln!(&mut out, "### Replay Results").unwrap();
        writeln!(&mut out).unwrap();
        writeln!(
            &mut out,
            "**Status:** `{}`",
            display_replay_status(&receipt.status)
        )
        .unwrap();
        writeln!(&mut out, "**Matched:** `{}`", receipt.matched).unwrap();
        writeln!(&mut out, "**Drift items:** {}", receipt.drift.len()).unwrap();
    }

    out
}

/// Render a receipt as Markdown, with optional verbose mode for env-excluded details.
pub fn render_receipt_markdown_verbose(receipt: &ReplayReceipt, verbose: bool) -> String {
    let mut out = String::new();

    writeln!(&mut out, "# ReproPack replay receipt").unwrap();
    writeln!(&mut out).unwrap();
    writeln!(&mut out, "- Packet: `{}`", receipt.packet_id).unwrap();
    writeln!(&mut out, "- Replayed at: `{}`", receipt.replayed_at).unwrap();
    writeln!(&mut out, "- Workdir: `{}`", receipt.workdir).unwrap();
    writeln!(
        &mut out,
        "- Status: `{}`",
        display_replay_status(&receipt.status)
    )
    .unwrap();
    writeln!(
        &mut out,
        "- Recorded exit code: `{}`",
        format_option_i32(receipt.recorded_exit_code)
    )
    .unwrap();
    writeln!(
        &mut out,
        "- Observed exit code: `{}`",
        format_option_i32(receipt.observed_exit_code)
    )
    .unwrap();
    writeln!(&mut out, "- Matched: `{}`", receipt.matched).unwrap();
    if let Some(matched_outputs) = receipt.matched_outputs {
        writeln!(&mut out, "- Matched outputs: `{}`", matched_outputs).unwrap();
    }
    writeln!(&mut out).unwrap();

    writeln!(&mut out, "## Command").unwrap();
    writeln!(&mut out).unwrap();
    writeln!(&mut out, "```text").unwrap();
    writeln!(&mut out, "{}", receipt.command_display).unwrap();
    writeln!(&mut out, "```").unwrap();
    writeln!(&mut out).unwrap();

    if !receipt.drift.is_empty() {
        writeln!(&mut out, "## Drift").unwrap();
        writeln!(&mut out).unwrap();
        for drift in &receipt.drift {
            if drift.subject == "env_excluded_summary" {
                let count = drift.observed.as_deref().unwrap_or("0");
                if verbose {
                    // Expand full list from env_excluded_keys
                    writeln!(
                        &mut out,
                        "- [{}] {} host environment variables excluded:",
                        display_severity(&drift.severity),
                        count
                    )
                    .unwrap();
                    if let Some(keys) = &receipt.env_excluded_keys {
                        for key in keys {
                            writeln!(&mut out, "  - `{}`", key).unwrap();
                        }
                    }
                } else {
                    writeln!(
                        &mut out,
                        "- [{}] {} host environment variables excluded",
                        display_severity(&drift.severity),
                        count
                    )
                    .unwrap();
                }
            } else {
                writeln!(
                    &mut out,
                    "- [{}] {} | expected=`{}` observed=`{}`",
                    display_severity(&drift.severity),
                    drift.subject,
                    drift.expected.as_deref().unwrap_or("n/a"),
                    drift.observed.as_deref().unwrap_or("n/a")
                )
                .unwrap();
            }
        }
        writeln!(&mut out).unwrap();
    }

    if let Some(env_cls) = &receipt.env_classification {
        writeln!(&mut out, "## Environment classification").unwrap();
        writeln!(&mut out).unwrap();
        writeln!(&mut out, "- Restored: `{}` keys", env_cls.restored.len()).unwrap();
        if !env_cls.restored.is_empty() {
            for key in &env_cls.restored {
                writeln!(&mut out, "  - `{}`", key).unwrap();
            }
        }
        writeln!(
            &mut out,
            "- Overridden: `{}` keys",
            env_cls.overridden.len()
        )
        .unwrap();
        if !env_cls.overridden.is_empty() {
            for key in &env_cls.overridden {
                writeln!(&mut out, "  - `{}`", key).unwrap();
            }
        }
        writeln!(&mut out, "- Inherited: `{}` keys", env_cls.inherited.len()).unwrap();
        if !env_cls.inherited.is_empty() {
            for key in &env_cls.inherited {
                writeln!(&mut out, "  - `{}`", key).unwrap();
            }
        }
        writeln!(&mut out).unwrap();
    }

    if !receipt.notes.is_empty() {
        writeln!(&mut out, "## Notes").unwrap();
        writeln!(&mut out).unwrap();
        for note in &receipt.notes {
            writeln!(&mut out, "- {}", note).unwrap();
        }
    }

    out
}

fn render_omission_markdown(out: &mut String, omission: &Omission) {
    writeln!(
        out,
        "- `{}` on `{}`: {}",
        omission.kind, omission.subject, omission.reason
    )
    .unwrap();
}

fn display_capture_level(manifest: &PacketManifest) -> &'static str {
    match manifest.capture_level {
        repropack_model::CaptureLevel::Metadata => "metadata",
        repropack_model::CaptureLevel::Repo => "repo",
        repropack_model::CaptureLevel::Inputs => "inputs",
    }
}

fn display_replay_fidelity(manifest: &PacketManifest) -> &'static str {
    match manifest.replay_fidelity {
        repropack_model::ReplayFidelity::Exact => "exact",
        repropack_model::ReplayFidelity::Approximate => "approximate",
        repropack_model::ReplayFidelity::InspectOnly => "inspect_only",
    }
}

fn display_replay_policy(manifest: &PacketManifest) -> &'static str {
    match manifest.replay_policy {
        repropack_model::ReplayPolicy::Safe => "safe",
        repropack_model::ReplayPolicy::Confirm => "confirm",
        repropack_model::ReplayPolicy::Disabled => "disabled",
    }
}

fn display_replay_status(status: &ReplayStatus) -> &'static str {
    match status {
        ReplayStatus::Matched => "matched",
        ReplayStatus::Mismatched => "mismatched",
        ReplayStatus::Skipped => "skipped",
        ReplayStatus::Blocked => "blocked",
        ReplayStatus::Error => "error",
    }
}

fn display_severity(severity: &repropack_model::Severity) -> &'static str {
    match severity {
        repropack_model::Severity::Info => "info",
        repropack_model::Severity::Warning => "warning",
        repropack_model::Severity::Error => "error",
    }
}

fn format_option_i32(value: Option<i32>) -> String {
    value
        .map(|inner| inner.to_string())
        .unwrap_or_else(|| "n/a".to_string())
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use repropack_model::{
        CommandRecord, EnvironmentRecord, ExecutionRecord, PacketManifest, PlatformFingerprint,
    };

    use super::*;

    #[test]
    fn html_renderer_escapes_angle_brackets() {
        let manifest = PacketManifest::new(
            None,
            CommandRecord {
                program: "echo".to_string(),
                args: vec!["<tag>".to_string()],
                display: "echo <tag>".to_string(),
                cwd: ".".to_string(),
                cwd_relative_to_repo: Some(".".to_string()),
            },
            ExecutionRecord {
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:01Z".to_string(),
                duration_ms: 1,
                exit_code: Some(0),
                signal: None,
                success: true,
                spawn_error: None,
                stdout_sha256: None,
                stderr_sha256: None,
            },
            EnvironmentRecord {
                platform: PlatformFingerprint {
                    family: "unix".to_string(),
                    os: "linux".to_string(),
                    arch: "x86_64".to_string(),
                },
                allowed_vars: BTreeMap::new(),
                redacted_keys: Vec::new(),
                tool_versions: BTreeMap::new(),
            },
        );

        let html = render_manifest_html(&manifest);
        assert!(html.contains("&lt;tag&gt;"));
    }
}
