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
}
