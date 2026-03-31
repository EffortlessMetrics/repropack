// Feature: repropack-v02-alpha, Property 4: Evidence digest mismatch produces drift
// Feature: repropack-v02-alpha, Property 5: matched_outputs is the conjunction of all output comparisons
// Feature: repropack-v02-alpha, Property 6: Receipt matched equals exit code match AND evidence match

use proptest::prelude::*;
use repropack_model::IndexedFile;
use repropack_pack::sha256_bytes;
use repropack_replay::{compute_evidence_drift, compute_output_drift, determine_match_status};
use std::fs;

proptest! {
    /// **Validates: Requirements 6.3, 6.4, 7.2**
    ///
    /// Property 4: For any pair of byte sequences where sha256(a) != sha256(b),
    /// when the manifest records sha256(a) and replay observes b, the receipt
    /// shall contain a DriftItem whose subject identifies the channel and whose
    /// expected/observed fields carry the respective digests.
    #[test]
    fn evidence_digest_mismatch_produces_drift(
        recorded_stdout in prop::collection::vec(any::<u8>(), 0..128),
        observed_stdout in prop::collection::vec(any::<u8>(), 0..128),
        recorded_stderr in prop::collection::vec(any::<u8>(), 0..128),
        observed_stderr in prop::collection::vec(any::<u8>(), 0..128),
    ) {
        let recorded_stdout_digest = sha256_bytes(&recorded_stdout);
        let recorded_stderr_digest = sha256_bytes(&recorded_stderr);

        let drift = compute_evidence_drift(
            Some(&recorded_stdout_digest),
            Some(&recorded_stderr_digest),
            &observed_stdout,
            &observed_stderr,
        );

        let observed_stdout_digest = sha256_bytes(&observed_stdout);
        let observed_stderr_digest = sha256_bytes(&observed_stderr);

        let stdout_differs = recorded_stdout_digest != observed_stdout_digest;
        let stderr_differs = recorded_stderr_digest != observed_stderr_digest;

        // If stdout digests differ, there must be a drift item for stdout_digest
        if stdout_differs {
            let stdout_drift = drift.iter().find(|d| d.subject == "stdout_digest");
            prop_assert!(
                stdout_drift.is_some(),
                "must produce drift item when stdout digest differs"
            );
            let item = stdout_drift.unwrap();
            prop_assert_eq!(
                item.expected.as_deref(),
                Some(recorded_stdout_digest.as_str())
            );
            prop_assert_eq!(
                item.observed.as_deref(),
                Some(observed_stdout_digest.as_str())
            );
        } else {
            prop_assert!(
                drift.iter().all(|d| d.subject != "stdout_digest"),
                "must not produce stdout drift when digests match"
            );
        }

        // If stderr digests differ, there must be a drift item for stderr_digest
        if stderr_differs {
            let stderr_drift = drift.iter().find(|d| d.subject == "stderr_digest");
            prop_assert!(
                stderr_drift.is_some(),
                "must produce drift item when stderr digest differs"
            );
            let item = stderr_drift.unwrap();
            prop_assert_eq!(
                item.expected.as_deref(),
                Some(recorded_stderr_digest.as_str())
            );
            prop_assert_eq!(
                item.observed.as_deref(),
                Some(observed_stderr_digest.as_str())
            );
        } else {
            prop_assert!(
                drift.iter().all(|d| d.subject != "stderr_digest"),
                "must not produce stderr drift when digests match"
            );
        }
    }
}

/// Strategy to generate a list of output files with matching or mismatching
/// digests in a temp directory. Returns (Vec<IndexedFile>, tempdir, expected_all_matched).
fn arb_output_scenario() -> impl Strategy<Value = (Vec<(String, Vec<u8>, bool)>,)> {
    // Generate 1..6 output files, each with content and a flag for whether it should match
    prop::collection::vec(
        (
            "[a-z]{1,5}\\.[a-z]{1,3}",                 // filename
            prop::collection::vec(any::<u8>(), 1..64), // file content
            any::<bool>(),                             // true = match, false = mismatch
        ),
        1..6,
    )
    .prop_map(|files| (files,))
}

proptest! {
    /// **Validates: Requirements 7.4**
    ///
    /// Property 5: matched_outputs is true iff every recorded output file is
    /// present after replay and its SHA-256 digest matches the recorded digest.
    #[test]
    fn matched_outputs_is_conjunction_of_all_comparisons(
        scenario in arb_output_scenario(),
    ) {
        let (files,) = scenario;
        let dir = tempfile::tempdir().unwrap();

        let mut indexed_files = Vec::new();
        let mut expect_all_matched = true;

        for (name, content, should_match) in &files {
            let recorded_digest = sha256_bytes(content);

            if *should_match {
                // Write the exact content so digest matches
                fs::write(dir.path().join(name), content).unwrap();
            } else {
                // Write different content so digest mismatches
                let mut altered = content.clone();
                altered.push(0xFF);
                fs::write(dir.path().join(name), &altered).unwrap();
                expect_all_matched = false;
            }

            indexed_files.push(IndexedFile {
                original_path: name.clone(),
                restore_path: Some(name.clone()),
                packet_path: format!("outputs/files/{}", name),
                sha256: recorded_digest,
                size_bytes: content.len() as u64,
            });
        }

        let (_drift, all_matched) = compute_output_drift(&indexed_files, dir.path());

        prop_assert_eq!(
            all_matched,
            expect_all_matched,
            "matched_outputs must be true iff all output digests match"
        );
    }

    /// **Validates: Requirements 7.4**
    ///
    /// Property 5 (missing file case): When any recorded output file is missing
    /// after replay, matched_outputs must be false.
    #[test]
    fn matched_outputs_false_when_file_missing(
        name in "[a-z]{1,5}\\.[a-z]{1,3}",
        content in prop::collection::vec(any::<u8>(), 1..64),
    ) {
        let dir = tempfile::tempdir().unwrap();
        // Don't write the file — it's missing

        let indexed_files = vec![IndexedFile {
            original_path: name.clone(),
            restore_path: Some(name.clone()),
            packet_path: format!("outputs/files/{}", name),
            sha256: sha256_bytes(&content),
            size_bytes: content.len() as u64,
        }];

        let (drift, all_matched) = compute_output_drift(&indexed_files, dir.path());

        prop_assert!(!all_matched, "matched_outputs must be false when file is missing");
        prop_assert!(
            drift.iter().any(|d| d.subject.starts_with("output_missing:")),
            "must produce output_missing drift item"
        );
    }
}

proptest! {
    /// **Validates: Requirements 8.1, 8.2**
    ///
    /// Property 6: receipt.matched is true iff observed_exit_code == recorded_exit_code
    /// AND all evidence digests match. When exit codes match but at least one
    /// evidence digest differs, receipt.status shall be mismatched.
    #[test]
    fn receipt_matched_equals_exit_and_evidence(
        exit_code_matched in any::<bool>(),
        evidence_all_matched in any::<bool>(),
    ) {
        let (matched, status, note) = determine_match_status(exit_code_matched, evidence_all_matched);

        // matched is true iff both conditions hold
        prop_assert_eq!(
            matched,
            exit_code_matched && evidence_all_matched,
            "matched must be true iff exit code matches AND all evidence matches"
        );

        if exit_code_matched && evidence_all_matched {
            prop_assert!(
                matches!(status, repropack_model::ReplayStatus::Matched),
                "status must be Matched when both conditions hold"
            );
            prop_assert!(note.is_none(), "no note when fully matched");
        } else if exit_code_matched && !evidence_all_matched {
            prop_assert!(
                matches!(status, repropack_model::ReplayStatus::Mismatched),
                "status must be Mismatched when exit matches but evidence diverges"
            );
            prop_assert!(
                note.is_some(),
                "must produce note when exit matches but evidence diverges"
            );
            prop_assert_eq!(
                note.as_deref(),
                Some("exit code matched but evidence diverged")
            );
        } else {
            prop_assert!(
                matches!(status, repropack_model::ReplayStatus::Mismatched),
                "status must be Mismatched when exit code differs"
            );
        }
    }
}
