//! Malformed packet rejection tests (Tasks 19.1, 19.4)
//!
//! These tests verify that the pack layer rejects bad input with clear errors.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use flate2::write::GzEncoder;
use flate2::Compression;
use repropack_model::IntegrityEntry;
use repropack_pack::{materialize, sha256_bytes, unpack_rpk};

/// Build a gzip-compressed tar archive containing a single entry at the given
/// path. Constructs the tar header manually to allow `..` path components.
fn build_raw_tar_gz(entry_path: &str, content: &[u8]) -> (PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let rpk_path = dir.path().join("malformed.rpk");

    let file = fs::File::create(&rpk_path).unwrap();
    let mut encoder = GzEncoder::new(file, Compression::default());

    let mut header = [0u8; 512];
    let path_bytes = entry_path.as_bytes();
    let name_len = path_bytes.len().min(100);
    header[..name_len].copy_from_slice(&path_bytes[..name_len]);

    header[100..108].copy_from_slice(b"0000644\0");
    header[108..116].copy_from_slice(b"0000000\0");
    header[116..124].copy_from_slice(b"0000000\0");
    let size_str = format!("{:011o}\0", content.len());
    header[124..136].copy_from_slice(size_str.as_bytes());
    header[136..148].copy_from_slice(b"00000000000\0");
    header[156] = b'0';

    header[148..156].copy_from_slice(b"        ");
    let cksum: u32 = header.iter().map(|&b| b as u32).sum();
    let cksum_str = format!("{:06o}\0 ", cksum);
    header[148..156].copy_from_slice(cksum_str.as_bytes());

    encoder.write_all(&header).unwrap();
    encoder.write_all(content).unwrap();
    let padding = (512 - (content.len() % 512)) % 512;
    if padding > 0 {
        encoder.write_all(&vec![0u8; padding]).unwrap();
    }
    encoder.write_all(&[0u8; 1024]).unwrap();
    encoder.finish().unwrap();

    (rpk_path, dir)
}

// ── 19.1: Path traversal archive entry is rejected ──────────────────
// Validates: Requirement 17.1

#[test]
fn path_traversal_entry_rejected() {
    let (rpk_path, _guard) = build_raw_tar_gz("../escape.txt", b"malicious content");

    let target_dir = tempfile::tempdir().unwrap();
    let result = unpack_rpk(&rpk_path, target_dir.path());

    assert!(
        result.is_err(),
        "unpack_rpk should reject path traversal entry"
    );

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("escapes target directory"),
        "error should mention directory escape, got: {err_msg}"
    );

    // Verify no file was written outside the target directory
    let escaped = target_dir.path().parent().unwrap().join("escape.txt");
    assert!(
        !escaped.exists(),
        "file should not have been written outside target directory"
    );
}

#[test]
fn nested_path_traversal_entry_rejected() {
    let (rpk_path, _guard) = build_raw_tar_gz("a/b/../../escape.txt", b"malicious");

    let target_dir = tempfile::tempdir().unwrap();
    let result = unpack_rpk(&rpk_path, target_dir.path());

    assert!(
        result.is_err(),
        "unpack_rpk should reject nested path traversal"
    );
}

// ── 19.4: Corrupted integrity digest is rejected ────────────────────
// Validates: Requirement 17.4

#[test]
fn corrupted_integrity_digest_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Write a file with known content
    fs::write(root.join("data.txt"), b"original content").unwrap();
    let correct_digest = sha256_bytes(b"original content");

    // Write integrity.json with the correct digest
    let entries = vec![IntegrityEntry {
        relative_path: "data.txt".to_string(),
        sha256: correct_digest,
        size_bytes: 16,
    }];
    fs::write(
        root.join("integrity.json"),
        serde_json::to_vec_pretty(&entries).unwrap(),
    )
    .unwrap();

    // Now tamper with the file
    fs::write(root.join("data.txt"), b"tampered content").unwrap();

    // materialize should fail
    let result = materialize(root);
    assert!(result.is_err(), "materialize should reject corrupted file");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("integrity mismatch"),
        "error should mention integrity mismatch, got: {err_msg}"
    );
    assert!(
        err_msg.contains("data.txt"),
        "error should identify the corrupted file, got: {err_msg}"
    );
}
