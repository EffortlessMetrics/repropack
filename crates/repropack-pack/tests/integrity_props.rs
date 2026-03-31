// Feature: repropack-v02-alpha, Property 8: Integrity envelope lists all packet files except itself
// Feature: repropack-v02-alpha, Property 9: Integrity mismatch is rejected on materialize

use std::collections::BTreeSet;
use std::fs;

use proptest::prelude::*;
use repropack_model::IntegrityEntry;
use repropack_pack::{materialize, sha256_bytes, sha256_file, verify_integrity};

/// Strategy producing a Vec of (relative_path, file_contents) pairs for a packet directory.
fn arb_packet_files() -> impl Strategy<Value = Vec<(String, Vec<u8>)>> {
    prop::collection::vec(
        (
            "[a-z]{1,4}(/[a-z]{1,4}){0,1}\\.[a-z]{1,3}",
            prop::collection::vec(any::<u8>(), 0..128),
        ),
        1..6,
    )
}

/// Write files into a directory and generate a correct integrity.json.
fn write_packet_with_integrity(entries: &[(String, Vec<u8>)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let mut integrity_entries = Vec::new();

    for (rel, content) in entries {
        let path = dir.path().join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();

        integrity_entries.push(IntegrityEntry {
            relative_path: rel.clone(),
            sha256: sha256_bytes(content),
            size_bytes: content.len() as u64,
        });
    }

    let json = serde_json::to_vec_pretty(&integrity_entries).unwrap();
    fs::write(dir.path().join("integrity.json"), json).unwrap();
    dir
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// **Validates: Requirements 10.1**
    ///
    /// For any packet directory, integrity.json lists one entry for every file
    /// except itself, and each entry's sha256 and size_bytes match the actual file.
    #[test]
    fn integrity_envelope_lists_all_files(entries in arb_packet_files()) {
        let dir = write_packet_with_integrity(&entries);
        let root = dir.path();

        // Read back integrity.json
        let bytes = fs::read(root.join("integrity.json")).unwrap();
        let parsed: Vec<IntegrityEntry> = serde_json::from_slice(&bytes).unwrap();

        // Collect all files in the directory except integrity.json
        let mut actual_files = BTreeSet::new();
        for entry in walkdir::WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let rel = entry
                .path()
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if rel != "integrity.json" {
                actual_files.insert(rel);
            }
        }

        // Integrity entries should cover exactly the non-integrity files
        let envelope_files: BTreeSet<String> =
            parsed.iter().map(|e| e.relative_path.clone()).collect();
        prop_assert_eq!(&actual_files, &envelope_files,
            "integrity.json should list all files except itself");

        // Each entry's digest and size should match the actual file
        for entry in &parsed {
            let file_path = root.join(&entry.relative_path);
            let actual_digest = sha256_file(&file_path).unwrap();
            prop_assert_eq!(&entry.sha256, &actual_digest,
                "digest mismatch for {}", entry.relative_path);

            let actual_size = fs::metadata(&file_path).unwrap().len();
            prop_assert_eq!(entry.size_bytes, actual_size,
                "size mismatch for {}", entry.relative_path);
        }

        // verify_integrity should pass
        prop_assert!(verify_integrity(root).is_ok(),
            "verify_integrity should pass on a valid packet");
    }

    /// **Validates: Requirements 10.3, 17.4**
    ///
    /// For any packet with integrity.json where at least one entry's sha256
    /// does not match the actual file, materialize returns an error identifying
    /// the mismatched file.
    #[test]
    fn integrity_mismatch_rejected_on_materialize(
        entries in arb_packet_files(),
        tamper_idx in any::<prop::sample::Index>(),
        tamper_byte in any::<u8>(),
    ) {
        // Create a valid packet directory with integrity.json
        let dir = write_packet_with_integrity(&entries);
        let root = dir.path();

        // Pick a file to tamper with
        let idx = tamper_idx.index(entries.len());
        let (tampered_rel, original_content) = &entries[idx];
        let tampered_path = root.join(tampered_rel);

        // Append a byte to change the content (guarantees different digest)
        let mut tampered_content = original_content.clone();
        tampered_content.push(tamper_byte);
        fs::write(&tampered_path, &tampered_content).unwrap();

        // verify_integrity should fail
        let result = verify_integrity(root);
        prop_assert!(result.is_err(),
            "verify_integrity should reject tampered file: {}", tampered_rel);

        let err_msg = result.unwrap_err().to_string();
        prop_assert!(err_msg.contains("integrity mismatch"),
            "error should mention integrity mismatch, got: {}", err_msg);
        prop_assert!(err_msg.contains(tampered_rel),
            "error should identify the tampered file '{}', got: {}", tampered_rel, err_msg);

        // materialize (on directory) should also fail
        let mat_result = materialize(root);
        prop_assert!(mat_result.is_err(),
            "materialize should reject packet with tampered file");
    }
}
