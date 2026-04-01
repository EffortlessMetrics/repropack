pub mod validate;

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

pub const MANIFEST_SCHEMA_VERSION: &str = "repropack.manifest.v1";
pub const RECEIPT_SCHEMA_VERSION: &str = "repropack.receipt.v1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureLevel {
    Metadata,
    Repo,
    Inputs,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReplayFidelity {
    Exact,
    Approximate,
    InspectOnly,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReplayPolicy {
    Safe,
    Confirm,
    Disabled,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PacketFileRole {
    Metadata,
    Git,
    Execution,
    Environment,
    Input,
    Output,
    Summary,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReplayStatus {
    Matched,
    Mismatched,
    Skipped,
    Blocked,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PacketManifest {
    pub schema_version: String,
    pub packet_id: String,
    pub packet_name: Option<String>,
    pub created_at: String,
    pub capture_level: CaptureLevel,
    pub replay_fidelity: ReplayFidelity,
    pub replay_policy: ReplayPolicy,
    pub command: CommandRecord,
    pub execution: ExecutionRecord,
    pub git: Option<GitState>,
    pub environment: EnvironmentRecord,
    pub inputs: Vec<IndexedFile>,
    pub outputs: Vec<IndexedFile>,
    pub packet_files: Vec<PacketFileRef>,
    pub omissions: Vec<Omission>,
    pub notes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redaction_report_path: Option<String>,
}

impl PacketManifest {
    pub fn new(
        packet_name: Option<String>,
        command: CommandRecord,
        execution: ExecutionRecord,
        environment: EnvironmentRecord,
    ) -> Self {
        Self {
            schema_version: MANIFEST_SCHEMA_VERSION.to_string(),
            packet_id: Uuid::new_v4().to_string(),
            packet_name,
            created_at: utc_now_string(),
            capture_level: CaptureLevel::Metadata,
            replay_fidelity: ReplayFidelity::InspectOnly,
            replay_policy: ReplayPolicy::Safe,
            command,
            execution,
            git: None,
            environment,
            inputs: Vec::new(),
            outputs: Vec::new(),
            packet_files: Vec::new(),
            omissions: Vec::new(),
            notes: Vec::new(),
            redaction_report_path: None,
        }
    }

    pub fn write_to_path(&self, path: &Path) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(self).map_err(json_err)?;
        fs::write(path, json)
    }

    pub fn read_from_path(path: &Path) -> io::Result<Self> {
        let bytes = fs::read(path)?;
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(json_err)?;
        validate::validate_manifest(&value)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        serde_json::from_value(value).map_err(json_err)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandRecord {
    pub program: String,
    pub args: Vec<String>,
    pub display: String,
    pub cwd: String,
    pub cwd_relative_to_repo: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionRecord {
    pub started_at: String,
    pub finished_at: String,
    pub duration_ms: u128,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub success: bool,
    pub spawn_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr_sha256: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitState {
    pub commit_sha: Option<String>,
    pub ref_name: Option<String>,
    pub base: Option<String>,
    pub head: Option<String>,
    pub is_dirty: bool,
    pub changed_paths: Vec<String>,
    pub untracked_paths: Vec<String>,
    pub bundle_path: Option<String>,
    pub diff_path: Option<String>,
    pub worktree_patch_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_pre: Option<GitSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_post: Option<GitSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_delta: Option<CaptureDelta>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnvironmentRecord {
    pub platform: PlatformFingerprint,
    pub allowed_vars: BTreeMap<String, String>,
    pub redacted_keys: Vec<String>,
    pub tool_versions: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlatformFingerprint {
    pub family: String,
    pub os: String,
    pub arch: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexedFile {
    pub original_path: String,
    pub restore_path: Option<String>,
    pub packet_path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PacketFileRef {
    pub role: PacketFileRole,
    pub relative_path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Omission {
    pub kind: String,
    pub subject: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DriftItem {
    pub subject: String,
    pub expected: Option<String>,
    pub observed: Option<String>,
    pub severity: Severity,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayReceipt {
    pub schema_version: String,
    pub packet_id: String,
    pub replayed_at: String,
    pub workdir: String,
    pub command_display: String,
    pub status: ReplayStatus,
    pub recorded_exit_code: Option<i32>,
    pub observed_exit_code: Option<i32>,
    pub matched: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_outputs: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_classification: Option<EnvClassification>,
    pub drift: Vec<DriftItem>,
    pub notes: Vec<String>,
    pub stdout_path: Option<String>,
    pub stderr_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_excluded_keys: Option<Vec<String>>,
}

impl ReplayReceipt {
    pub fn new(
        packet_id: impl Into<String>,
        workdir: impl Into<String>,
        command_display: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: RECEIPT_SCHEMA_VERSION.to_string(),
            packet_id: packet_id.into(),
            replayed_at: utc_now_string(),
            workdir: workdir.into(),
            command_display: command_display.into(),
            status: ReplayStatus::Skipped,
            recorded_exit_code: None,
            observed_exit_code: None,
            matched: false,
            matched_outputs: None,
            env_classification: None,
            drift: Vec::new(),
            notes: Vec::new(),
            stdout_path: None,
            stderr_path: None,
            env_excluded_keys: None,
        }
    }

    pub fn write_to_path(&self, path: &Path) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(self).map_err(json_err)?;
        fs::write(path, json)
    }

    pub fn read_from_path(path: &Path) -> io::Result<Self> {
        let bytes = fs::read(path)?;
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(json_err)?;
        validate::validate_receipt(&value)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        serde_json::from_value(value).map_err(json_err)
    }
}

// ── v0.2 new types ──────────────────────────────────────────────────

/// A point-in-time Git snapshot (used for both pre-run and post-run).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitSnapshot {
    pub commit_sha: Option<String>,
    pub is_dirty: bool,
    pub changed_paths: Vec<String>,
    pub untracked_paths: Vec<String>,
    pub worktree_patch_path: Option<String>,
}

/// The diff between pre-run and post-run Git state.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureDelta {
    pub newly_dirty_paths: Vec<String>,
    pub newly_modified_paths: Vec<String>,
    pub newly_untracked_paths: Vec<String>,
}

/// Environment variable classification in the replay receipt.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnvClassification {
    pub restored: Vec<String>,
    pub overridden: Vec<String>,
    pub inherited: Vec<String>,
}

/// A single entry in the integrity envelope.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntegrityEntry {
    pub relative_path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

// ── v0.3 new types ──────────────────────────────────────────────────

/// A single redaction entry describing what was redacted and why.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RedactionEntry {
    pub field_or_path: String,
    pub action: RedactionAction,
    pub reason: String,
}

/// The kind of redaction applied.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RedactionAction {
    Replaced,
    Removed,
    Cleared,
}

/// Doctor report assessing packet completeness and replay-worthiness.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DoctorReport {
    pub readiness: DoctorReadiness,
    pub omissions_by_kind: BTreeMap<String, Vec<Omission>>,
    pub redacted_env_keys: Vec<String>,
    pub tool_versions: BTreeMap<String, String>,
    pub missing_tools: Vec<String>,
    pub has_redaction_report: bool,
    pub redaction_summary: Option<RedactionSummary>,
    pub notes: Vec<String>,
}

/// Overall readiness verdict from the doctor check.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DoctorReadiness {
    Ready,
    Degraded,
    Blocked,
}

/// Summary counts for redaction operations.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RedactionSummary {
    pub replaced_values: usize,
    pub removed_files: usize,
}

// ── v0.3 configuration types ────────────────────────────────────────

/// Parsed `.repropack.toml` configuration.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct RepropackConfig {
    #[serde(default)]
    pub env_allow: Vec<String>,
    #[serde(default)]
    pub env_deny: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_file_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_packet_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_bundle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_policy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_profile: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub profile: BTreeMap<String, ProfileConfig>,
}

/// A named profile section within `.repropack.toml`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ProfileConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env_allow: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env_deny: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_file_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_packet_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_bundle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_policy: Option<String>,
}

/// Fully resolved configuration with no Option fields.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedConfig {
    pub env_allow: Vec<String>,
    pub env_deny: Vec<String>,
    pub max_file_size: u64,
    pub max_packet_size: u64,
    pub format: String,
    pub git_bundle: String,
    pub replay_policy: String,
}

impl RepropackConfig {
    /// Resolve a config by merging a named profile over top-level defaults.
    /// Profile values override top-level values; absent profile keys fall
    /// back to top-level defaults.
    pub fn resolve(&self, profile_name: Option<&str>) -> Result<ResolvedConfig, String> {
        let profile = match profile_name {
            Some(name) => {
                let p = self
                    .profile
                    .get(name)
                    .ok_or_else(|| format!("profile '{}' not found", name))?;
                Some(p)
            }
            None => None,
        };

        let env_allow = match profile {
            Some(p) if !p.env_allow.is_empty() => p.env_allow.clone(),
            _ => self.env_allow.clone(),
        };
        let env_deny = match profile {
            Some(p) if !p.env_deny.is_empty() => p.env_deny.clone(),
            _ => self.env_deny.clone(),
        };
        let max_file_size = profile
            .and_then(|p| p.max_file_size)
            .or(self.max_file_size)
            .unwrap_or(52_428_800);
        let max_packet_size = profile
            .and_then(|p| p.max_packet_size)
            .or(self.max_packet_size)
            .unwrap_or(524_288_000);
        let format = profile
            .and_then(|p| p.format.clone())
            .or_else(|| self.format.clone())
            .unwrap_or_else(|| "rpk".to_string());
        let git_bundle = profile
            .and_then(|p| p.git_bundle.clone())
            .or_else(|| self.git_bundle.clone())
            .unwrap_or_else(|| "auto".to_string());
        let replay_policy = profile
            .and_then(|p| p.replay_policy.clone())
            .or_else(|| self.replay_policy.clone())
            .unwrap_or_else(|| "safe".to_string());

        Ok(ResolvedConfig {
            env_allow,
            env_deny,
            max_file_size,
            max_packet_size,
            format,
            git_bundle,
            replay_policy,
        })
    }

    /// Parse from a TOML string.
    pub fn from_toml(toml_str: &str) -> Result<Self, String> {
        toml::from_str(toml_str).map_err(|e| e.to_string())
    }

    /// Serialize to a TOML string.
    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| e.to_string())
    }
}

/// Size cap configuration for capture.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SizeCaps {
    pub max_file_bytes: u64,
    pub max_packet_bytes: u64,
}

impl Default for SizeCaps {
    fn default() -> Self {
        Self {
            max_file_bytes: 50 * 1024 * 1024,    // 50 MiB
            max_packet_bytes: 500 * 1024 * 1024, // 500 MiB
        }
    }
}

/// Extract the semantic version component from a tool version string.
/// E.g., "rustc 1.78.0 (9b00956e5 2024-04-29)" → "1.78.0"
/// Returns None if no semver-like pattern is found.
pub fn extract_semver(version_string: &str) -> Option<String> {
    let re = regex::Regex::new(r"\b(\d+\.\d+\.\d+)\b").expect("valid regex");
    re.find(version_string).map(|m| m.as_str().to_string())
}

pub fn utc_now_string() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn json_err(err: serde_json::Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_round_trip() {
        let manifest = PacketManifest::new(
            Some("sample".to_string()),
            CommandRecord {
                program: "cargo".to_string(),
                args: vec!["test".to_string()],
                display: "cargo test".to_string(),
                cwd: "/tmp/repo".to_string(),
                cwd_relative_to_repo: Some(".".to_string()),
            },
            ExecutionRecord {
                started_at: utc_now_string(),
                finished_at: utc_now_string(),
                duration_ms: 10,
                exit_code: Some(1),
                signal: None,
                success: false,
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

        let json = serde_json::to_vec_pretty(&manifest).unwrap();
        let reparsed: PacketManifest = serde_json::from_slice(&json).unwrap();
        assert_eq!(reparsed.schema_version, MANIFEST_SCHEMA_VERSION);
    }

    #[test]
    fn extract_semver_from_rustc() {
        assert_eq!(
            extract_semver("rustc 1.78.0 (9b00956e5 2024-04-29)"),
            Some("1.78.0".to_string()),
        );
    }

    #[test]
    fn extract_semver_bare() {
        assert_eq!(extract_semver("3.12.4"), Some("3.12.4".to_string()));
    }

    #[test]
    fn extract_semver_none_for_garbage() {
        assert_eq!(extract_semver("no version here"), None);
    }
}
