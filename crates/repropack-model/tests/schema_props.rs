// Feature: repropack-v02-alpha, Property 7: Schema validation rejects non-conforming JSON
// **Validates: Requirements 9.1, 9.2, 9.4, 17.2, 17.3**

use proptest::prelude::*;
use repropack_model::validate;
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

/// Build a minimal valid receipt JSON value.
fn minimal_receipt() -> serde_json::Value {
    json!({
        "schema_version": "repropack.receipt.v1",
        "packet_id": "00000000-0000-4000-8000-000000000000",
        "replayed_at": "2026-01-15T12:00:00Z",
        "workdir": "/tmp/replay",
        "command_display": "echo hello",
        "status": "matched",
        "recorded_exit_code": null,
        "observed_exit_code": null,
        "matched": true,
        "drift": [],
        "notes": [],
        "stdout_path": null,
        "stderr_path": null
    })
}

/// Required fields in the manifest schema.
const MANIFEST_REQUIRED_FIELDS: &[&str] = &[
    "schema_version",
    "packet_id",
    "created_at",
    "capture_level",
    "replay_fidelity",
    "replay_policy",
    "command",
    "execution",
    "environment",
    "inputs",
    "outputs",
    "packet_files",
    "omissions",
    "notes",
];

/// Required fields in the receipt schema.
const RECEIPT_REQUIRED_FIELDS: &[&str] = &[
    "schema_version",
    "packet_id",
    "replayed_at",
    "workdir",
    "command_display",
    "status",
    "recorded_exit_code",
    "observed_exit_code",
    "matched",
    "drift",
    "notes",
    "stdout_path",
    "stderr_path",
];

// ── Property Tests ──────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Wrong schema_version on manifest is rejected.
    #[test]
    fn manifest_wrong_schema_version(version in "[a-z.]{1,30}") {
        // Skip if we accidentally generate the correct version
        prop_assume!(version != "repropack.manifest.v1");

        let mut doc = minimal_manifest();
        doc["schema_version"] = json!(version);

        let result = validate::validate_manifest(&doc);
        prop_assert!(result.is_err(), "Expected rejection for schema_version={version:?}");
    }

    /// Wrong schema_version on receipt is rejected.
    #[test]
    fn receipt_wrong_schema_version(version in "[a-z.]{1,30}") {
        prop_assume!(version != "repropack.receipt.v1");

        let mut doc = minimal_receipt();
        doc["schema_version"] = json!(version);

        let result = validate::validate_receipt(&doc);
        prop_assert!(result.is_err(), "Expected rejection for schema_version={version:?}");
    }

    /// Missing a required manifest field is rejected with a validation path.
    #[test]
    fn manifest_missing_required_field(
        field_idx in 0..MANIFEST_REQUIRED_FIELDS.len()
    ) {
        let field = MANIFEST_REQUIRED_FIELDS[field_idx];
        let mut doc = minimal_manifest();
        doc.as_object_mut().unwrap().remove(field);

        let result = validate::validate_manifest(&doc);
        prop_assert!(
            result.is_err(),
            "Expected rejection when missing field {field:?}"
        );
    }

    /// Missing a required receipt field is rejected with a validation path.
    #[test]
    fn receipt_missing_required_field(
        field_idx in 0..RECEIPT_REQUIRED_FIELDS.len()
    ) {
        let field = RECEIPT_REQUIRED_FIELDS[field_idx];
        let mut doc = minimal_receipt();
        doc.as_object_mut().unwrap().remove(field);

        let result = validate::validate_receipt(&doc);
        prop_assert!(
            result.is_err(),
            "Expected rejection when missing field {field:?}"
        );
    }

    /// Wrong type for manifest fields is rejected.
    #[test]
    fn manifest_wrong_type_for_field(
        field_idx in 0..3usize,
        bad_value in prop_oneof![
            Just(json!(12345)),
            Just(json!(true)),
            Just(json!([1, 2, 3])),
        ]
    ) {
        // Pick a string-typed field and replace with a wrong type
        let string_fields = ["packet_id", "created_at", "capture_level"];
        let field = string_fields[field_idx];

        let mut doc = minimal_manifest();
        doc[field] = bad_value;

        let result = validate::validate_manifest(&doc);
        prop_assert!(
            result.is_err(),
            "Expected rejection for wrong type on field {field:?}"
        );
    }

    /// Wrong type for receipt fields is rejected.
    #[test]
    fn receipt_wrong_type_for_field(
        field_idx in 0..3usize,
        bad_value in prop_oneof![
            Just(json!(12345)),
            Just(json!(true)),
            Just(json!([1, 2, 3])),
        ]
    ) {
        let string_fields = ["packet_id", "replayed_at", "command_display"];
        let field = string_fields[field_idx];

        let mut doc = minimal_receipt();
        doc[field] = bad_value;

        let result = validate::validate_receipt(&doc);
        prop_assert!(
            result.is_err(),
            "Expected rejection for wrong type on field {field:?}"
        );
    }
}
