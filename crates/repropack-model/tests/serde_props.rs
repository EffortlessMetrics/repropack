// Feature: repropack-v02-alpha, Property 12: Manifest serde round-trip and schema conformance
// Feature: repropack-v02-alpha, Property 13: Receipt serde round-trip and schema conformance

use proptest::prelude::*;
use repropack_model::validate;
use repropack_model::*;
use std::collections::BTreeMap;

// ── Generators ──────────────────────────────────────────────────────

fn arb_nonempty_string() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_/.-]{1,30}"
}

fn arb_sha256() -> impl Strategy<Value = String> {
    "[0-9a-f]{64}"
}

fn arb_timestamp() -> impl Strategy<Value = String> {
    Just("2026-01-15T12:00:00Z".to_string())
}

fn arb_uuid() -> impl Strategy<Value = String> {
    "[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}"
}

fn arb_string_map(max_entries: usize) -> impl Strategy<Value = BTreeMap<String, String>> {
    prop::collection::btree_map("[A-Z_]{1,10}", "[a-zA-Z0-9._/-]{1,20}", 0..=max_entries)
}

fn arb_capture_level() -> impl Strategy<Value = CaptureLevel> {
    prop_oneof![
        Just(CaptureLevel::Metadata),
        Just(CaptureLevel::Repo),
        Just(CaptureLevel::Inputs),
    ]
}

fn arb_replay_fidelity() -> impl Strategy<Value = ReplayFidelity> {
    prop_oneof![
        Just(ReplayFidelity::Exact),
        Just(ReplayFidelity::Approximate),
        Just(ReplayFidelity::InspectOnly),
    ]
}

fn arb_replay_policy() -> impl Strategy<Value = ReplayPolicy> {
    prop_oneof![
        Just(ReplayPolicy::Safe),
        Just(ReplayPolicy::Confirm),
        Just(ReplayPolicy::Disabled),
    ]
}

fn arb_packet_file_role() -> impl Strategy<Value = PacketFileRole> {
    prop_oneof![
        Just(PacketFileRole::Metadata),
        Just(PacketFileRole::Git),
        Just(PacketFileRole::Execution),
        Just(PacketFileRole::Environment),
        Just(PacketFileRole::Input),
        Just(PacketFileRole::Output),
        Just(PacketFileRole::Summary),
    ]
}

fn arb_replay_status() -> impl Strategy<Value = ReplayStatus> {
    prop_oneof![
        Just(ReplayStatus::Matched),
        Just(ReplayStatus::Mismatched),
        Just(ReplayStatus::Skipped),
        Just(ReplayStatus::Blocked),
        Just(ReplayStatus::Error),
    ]
}

fn arb_severity() -> impl Strategy<Value = Severity> {
    prop_oneof![
        Just(Severity::Info),
        Just(Severity::Warning),
        Just(Severity::Error),
    ]
}

fn arb_command_record() -> impl Strategy<Value = CommandRecord> {
    (
        arb_nonempty_string(),
        prop::collection::vec(arb_nonempty_string(), 0..3),
        arb_nonempty_string(),
        arb_nonempty_string(),
        prop::option::of(arb_nonempty_string()),
    )
        .prop_map(
            |(program, args, display, cwd, cwd_relative_to_repo)| CommandRecord {
                program,
                args,
                display,
                cwd,
                cwd_relative_to_repo,
            },
        )
}

fn arb_execution_record() -> impl Strategy<Value = ExecutionRecord> {
    (
        arb_timestamp(),
        arb_timestamp(),
        0u64..1_000_000u64,
        prop::option::of(-128i32..128i32),
        prop::option::of(1i32..32i32),
        any::<bool>(),
        prop::option::of(arb_nonempty_string()),
        prop::option::of(arb_sha256()),
        prop::option::of(arb_sha256()),
    )
        .prop_map(
            |(
                started_at,
                finished_at,
                duration_ms,
                exit_code,
                signal,
                success,
                spawn_error,
                stdout_sha256,
                stderr_sha256,
            )| ExecutionRecord {
                started_at,
                finished_at,
                duration_ms: duration_ms as u128,
                exit_code,
                signal,
                success,
                spawn_error,
                stdout_sha256,
                stderr_sha256,
            },
        )
}

fn arb_git_snapshot() -> impl Strategy<Value = GitSnapshot> {
    (
        prop::option::of(arb_sha256()),
        any::<bool>(),
        prop::collection::vec(arb_nonempty_string(), 0..3),
        prop::collection::vec(arb_nonempty_string(), 0..3),
        prop::option::of(arb_nonempty_string()),
    )
        .prop_map(
            |(commit_sha, is_dirty, changed_paths, untracked_paths, worktree_patch_path)| {
                GitSnapshot {
                    commit_sha,
                    is_dirty,
                    changed_paths,
                    untracked_paths,
                    worktree_patch_path,
                }
            },
        )
}

fn arb_capture_delta() -> impl Strategy<Value = CaptureDelta> {
    (
        prop::collection::vec(arb_nonempty_string(), 0..3),
        prop::collection::vec(arb_nonempty_string(), 0..3),
        prop::collection::vec(arb_nonempty_string(), 0..3),
    )
        .prop_map(
            |(newly_dirty_paths, newly_modified_paths, newly_untracked_paths)| CaptureDelta {
                newly_dirty_paths,
                newly_modified_paths,
                newly_untracked_paths,
            },
        )
}

fn arb_git_state() -> impl Strategy<Value = GitState> {
    // Split into two stages to stay within proptest's 12-element tuple limit.
    let base = (
        prop::option::of(arb_sha256()),
        prop::option::of(arb_nonempty_string()),
        prop::option::of(arb_nonempty_string()),
        prop::option::of(arb_nonempty_string()),
        any::<bool>(),
        prop::collection::vec(arb_nonempty_string(), 0..3),
        prop::collection::vec(arb_nonempty_string(), 0..3),
        prop::option::of(arb_nonempty_string()),
        prop::option::of(arb_nonempty_string()),
        prop::option::of(arb_nonempty_string()),
    );
    let v02_fields = (
        prop::option::of(arb_git_snapshot()),
        prop::option::of(arb_git_snapshot()),
        prop::option::of(arb_capture_delta()),
    );
    (base, v02_fields).prop_map(
        |(
            (
                commit_sha,
                ref_name,
                base_field,
                head,
                is_dirty,
                changed_paths,
                untracked_paths,
                bundle_path,
                diff_path,
                worktree_patch_path,
            ),
            (git_pre, git_post, capture_delta),
        )| GitState {
            commit_sha,
            ref_name,
            base: base_field,
            head,
            is_dirty,
            changed_paths,
            untracked_paths,
            bundle_path,
            diff_path,
            worktree_patch_path,
            git_pre,
            git_post,
            capture_delta,
        },
    )
}

fn arb_platform_fingerprint() -> impl Strategy<Value = PlatformFingerprint> {
    (
        arb_nonempty_string(),
        arb_nonempty_string(),
        arb_nonempty_string(),
    )
        .prop_map(|(family, os, arch)| PlatformFingerprint { family, os, arch })
}

fn arb_environment_record() -> impl Strategy<Value = EnvironmentRecord> {
    (
        arb_platform_fingerprint(),
        arb_string_map(3),
        prop::collection::vec("[A-Z_]{1,10}", 0..3),
        arb_string_map(3),
    )
        .prop_map(
            |(platform, allowed_vars, redacted_keys, tool_versions)| EnvironmentRecord {
                platform,
                allowed_vars,
                redacted_keys,
                tool_versions,
            },
        )
}

fn arb_indexed_file() -> impl Strategy<Value = IndexedFile> {
    (
        arb_nonempty_string(),
        prop::option::of(arb_nonempty_string()),
        arb_nonempty_string(),
        arb_sha256(),
        0u64..1_000_000u64,
    )
        .prop_map(
            |(original_path, restore_path, packet_path, sha256, size_bytes)| IndexedFile {
                original_path,
                restore_path,
                packet_path,
                sha256,
                size_bytes,
            },
        )
}

fn arb_packet_file_ref() -> impl Strategy<Value = PacketFileRef> {
    (
        arb_packet_file_role(),
        arb_nonempty_string(),
        arb_sha256(),
        0u64..1_000_000u64,
    )
        .prop_map(|(role, relative_path, sha256, size_bytes)| PacketFileRef {
            role,
            relative_path,
            sha256,
            size_bytes,
        })
}

fn arb_omission() -> impl Strategy<Value = Omission> {
    (
        arb_nonempty_string(),
        arb_nonempty_string(),
        arb_nonempty_string(),
    )
        .prop_map(|(kind, subject, reason)| Omission {
            kind,
            subject,
            reason,
        })
}

fn arb_drift_item() -> impl Strategy<Value = DriftItem> {
    (
        arb_nonempty_string(),
        prop::option::of(arb_nonempty_string()),
        prop::option::of(arb_nonempty_string()),
        arb_severity(),
    )
        .prop_map(|(subject, expected, observed, severity)| DriftItem {
            subject,
            expected,
            observed,
            severity,
        })
}

fn arb_env_classification() -> impl Strategy<Value = EnvClassification> {
    (
        prop::collection::vec("[A-Z_]{1,10}", 0..3),
        prop::collection::vec("[A-Z_]{1,10}", 0..3),
        prop::collection::vec("[A-Z_]{1,10}", 0..3),
    )
        .prop_map(|(restored, overridden, inherited)| EnvClassification {
            restored,
            overridden,
            inherited,
        })
}

fn arb_packet_manifest() -> impl Strategy<Value = PacketManifest> {
    // Split into two groups to stay within tuple limits.
    let identity = (
        arb_uuid(),
        prop::option::of(arb_nonempty_string()),
        arb_timestamp(),
        arb_capture_level(),
        arb_replay_fidelity(),
        arb_replay_policy(),
        arb_command_record(),
        arb_execution_record(),
    );
    let collections = (
        prop::option::of(arb_git_state()),
        arb_environment_record(),
        prop::collection::vec(arb_indexed_file(), 0..3),
        prop::collection::vec(arb_indexed_file(), 0..3),
        prop::collection::vec(arb_packet_file_ref(), 0..3),
        prop::collection::vec(arb_omission(), 0..3),
        prop::collection::vec(arb_nonempty_string(), 0..3),
    );
    (identity, collections).prop_map(
        |(
            (
                packet_id,
                packet_name,
                created_at,
                capture_level,
                replay_fidelity,
                replay_policy,
                command,
                execution,
            ),
            (git, environment, inputs, outputs, packet_files, omissions, notes),
        )| PacketManifest {
            schema_version: MANIFEST_SCHEMA_VERSION.to_string(),
            packet_id,
            packet_name,
            created_at,
            capture_level,
            replay_fidelity,
            replay_policy,
            command,
            execution,
            git,
            environment,
            inputs,
            outputs,
            packet_files,
            omissions,
            notes,
        },
    )
}

fn arb_replay_receipt() -> impl Strategy<Value = ReplayReceipt> {
    let identity = (
        arb_uuid(),
        arb_timestamp(),
        arb_nonempty_string(),
        arb_nonempty_string(),
        arb_replay_status(),
        prop::option::of(-128i32..128i32),
        prop::option::of(-128i32..128i32),
        any::<bool>(),
    );
    let extras = (
        prop::option::of(any::<bool>()),
        prop::option::of(arb_env_classification()),
        prop::collection::vec(arb_drift_item(), 0..3),
        prop::collection::vec(arb_nonempty_string(), 0..3),
        prop::option::of(arb_nonempty_string()),
        prop::option::of(arb_nonempty_string()),
    );
    (identity, extras).prop_map(
        |(
            (
                packet_id,
                replayed_at,
                workdir,
                command_display,
                status,
                recorded_exit_code,
                observed_exit_code,
                matched,
            ),
            (matched_outputs, env_classification, drift, notes, stdout_path, stderr_path),
        )| ReplayReceipt {
            schema_version: RECEIPT_SCHEMA_VERSION.to_string(),
            packet_id,
            replayed_at,
            workdir,
            command_display,
            status,
            recorded_exit_code,
            observed_exit_code,
            matched,
            matched_outputs,
            env_classification,
            drift,
            notes,
            stdout_path,
            stderr_path,
        },
    )
}

// ── Property Tests ──────────────────────────────────────────────────

// Feature: repropack-v02-alpha, Property 12: Manifest serde round-trip and schema conformance
// **Validates: Requirements 12.4, 15.1, 15.3**
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn manifest_serde_round_trip(manifest in arb_packet_manifest()) {
        // Serialize to JSON
        let json = serde_json::to_vec_pretty(&manifest).unwrap();

        // Deserialize back
        let reparsed: PacketManifest = serde_json::from_slice(&json).unwrap();
        prop_assert_eq!(&manifest, &reparsed);

        // Validate against schema
        let value: serde_json::Value = serde_json::from_slice(&json).unwrap();
        prop_assert!(
            validate::validate_manifest(&value).is_ok(),
            "Serialized manifest failed schema validation: {:?}",
            validate::validate_manifest(&value).err()
        );
    }
}

// Feature: repropack-v02-alpha, Property 13: Receipt serde round-trip and schema conformance
// **Validates: Requirements 12.5, 15.2, 15.4**
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn receipt_serde_round_trip(receipt in arb_replay_receipt()) {
        // Serialize to JSON
        let json = serde_json::to_vec_pretty(&receipt).unwrap();

        // Deserialize back
        let reparsed: ReplayReceipt = serde_json::from_slice(&json).unwrap();
        prop_assert_eq!(&receipt, &reparsed);

        // Validate against schema
        let value: serde_json::Value = serde_json::from_slice(&json).unwrap();
        prop_assert!(
            validate::validate_receipt(&value).is_ok(),
            "Serialized receipt failed schema validation: {:?}",
            validate::validate_receipt(&value).err()
        );
    }
}
