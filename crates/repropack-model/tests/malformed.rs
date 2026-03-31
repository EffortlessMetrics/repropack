//! Malformed packet rejection tests (Tasks 19.2, 19.3)
//!
//! These tests verify that the model layer rejects malformed manifests
//! with clear errors.

use std::fs;

use repropack_model::PacketManifest;
use serde_json::json;

/// Build a minimal valid manifest JSON value.
fn minimal_manifest() -> serde_json::Value {
    json!({
        "schema_version": "repropack.manifest.v1",
        "packet_id": "00000000-0000-4000-8000-000000000000",
        "created_at": "2026-01-15T12:00:00Z",
        "capture_level": "metadata",
        "replay_fidelity": "exact",
        "replay_policy": "safe",
        "command": {
            "program": "echo",
            "args": ["hello"],
            "display": "echo hello",
            "cwd": "/tmp"
        },
        "execution": {
            "started_at": "2026-01-15T12:00:00Z",
            "finished_at": "2026-01-15T12:00:01Z",
            "duration_ms": 1000,
            "success": false
        },
        "environment": {
            "platform": {
                "family": "unix",
                "os": "linux",
                "arch": "x86_64"
            },
            "allowed_vars": {},
            "redacted_keys": [],
            "tool_versions": {}
        },
        "inputs": [],
        "outputs": [],
        "packet_files": [],
        "omissions": [],
        "notes": []
    })
}

// ── 19.2: Unrecognized schema_version is rejected ───────────────────
// Validates: Requirement 17.2

#[test]
fn unrecognized_schema_version_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest_path = tmp.path().join("manifest.json");

    let mut doc = minimal_manifest();
    doc["schema_version"] = json!("repropack.manifest.v99");

    fs::write(&manifest_path, serde_json::to_vec_pretty(&doc).unwrap()).unwrap();

    let result = PacketManifest::read_from_path(&manifest_path);
    assert!(
        result.is_err(),
        "read_from_path should reject unrecognized schema_version"
    );

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("validation error"),
        "error should mention validation, got: {err_msg}"
    );
}

#[test]
fn completely_wrong_schema_version_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest_path = tmp.path().join("manifest.json");

    let mut doc = minimal_manifest();
    doc["schema_version"] = json!("not-a-repropack-schema");

    fs::write(&manifest_path, serde_json::to_vec_pretty(&doc).unwrap()).unwrap();

    let result = PacketManifest::read_from_path(&manifest_path);
    assert!(
        result.is_err(),
        "read_from_path should reject wrong schema_version"
    );
}

// ── 19.3: Missing required fields are rejected ──────────────────────
// Validates: Requirement 17.3

#[test]
fn missing_required_field_rejected_with_path() {
    let tmp = tempfile::tempdir().unwrap();

    // Test removing several required fields
    let required_fields = [
        "schema_version",
        "packet_id",
        "command",
        "execution",
        "environment",
    ];

    for field in &required_fields {
        let manifest_path = tmp.path().join(format!("manifest-no-{field}.json"));

        let mut doc = minimal_manifest();
        doc.as_object_mut().unwrap().remove(*field);

        fs::write(&manifest_path, serde_json::to_vec_pretty(&doc).unwrap()).unwrap();

        let result = PacketManifest::read_from_path(&manifest_path);
        assert!(
            result.is_err(),
            "read_from_path should reject manifest missing '{field}'"
        );

        let err_msg = result.unwrap_err().to_string();
        // The error should contain a validation message indicating the missing field
        assert!(
            err_msg.contains("validation error"),
            "error for missing '{field}' should mention validation, got: {err_msg}"
        );
    }
}

#[test]
fn missing_nested_required_field_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest_path = tmp.path().join("manifest.json");

    // Remove a required field from the command object
    let mut doc = minimal_manifest();
    doc["command"].as_object_mut().unwrap().remove("program");

    fs::write(&manifest_path, serde_json::to_vec_pretty(&doc).unwrap()).unwrap();

    let result = PacketManifest::read_from_path(&manifest_path);
    assert!(
        result.is_err(),
        "read_from_path should reject manifest with missing nested field"
    );

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("validation error"),
        "error should mention validation, got: {err_msg}"
    );
}
