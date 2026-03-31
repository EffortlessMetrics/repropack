// Feature: repropack-v02-alpha, Property 14: Pack/unpack round-trip preserves all file contents
// Feature: repropack-v02-alpha, Property 15: Pack determinism
// Feature: repropack-v02-alpha, Property 16: Path traversal rejection

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::write::GzEncoder;
use flate2::Compression;
use proptest::prelude::*;
use repropack_pack::{pack_dir, sha256_file, unpack_rpk};

/// Build a gzip-compressed tar archive containing a single entry at the given
/// path (which may contain `..` components). We construct the tar header
/// manually because the `tar` crate's `Builder::append_data` rejects `..` paths.
/// Returns (rpk_path, _tempdir_guard) — caller must hold the guard to keep the file alive.
fn build_raw_tar_gz_with_path(entry_path: &str, content: &[u8]) -> (PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let rpk_path = dir.path().join("evil.rpk");

    let file = fs::File::create(&rpk_path).unwrap();
    let mut encoder = GzEncoder::new(file, Compression::default());

    // Build a GNU tar header (512 bytes)
    let mut header = [0u8; 512];
    let path_bytes = entry_path.as_bytes();
    let name_len = path_bytes.len().min(100);
    header[..name_len].copy_from_slice(&path_bytes[..name_len]);

    // File mode: "0000644\0" at offset 100
    header[100..108].copy_from_slice(b"0000644\0");
    // Owner/group uid/gid: "0000000\0" at offsets 108 and 116
    header[108..116].copy_from_slice(b"0000000\0");
    header[116..124].copy_from_slice(b"0000000\0");
    // File size in octal at offset 124 (11 chars + NUL)
    let size_str = format!("{:011o}\0", content.len());
    header[124..136].copy_from_slice(size_str.as_bytes());
    // Mtime: "00000000000\0" at offset 136
    header[136..148].copy_from_slice(b"00000000000\0");
    // Type flag: '0' (regular file) at offset 156
    header[156] = b'0';

    // Compute checksum: sum of all bytes in header, treating checksum field (148..156) as spaces
    header[148..156].copy_from_slice(b"        ");
    let cksum: u32 = header.iter().map(|&b| b as u32).sum();
    let cksum_str = format!("{:06o}\0 ", cksum);
    header[148..156].copy_from_slice(cksum_str.as_bytes());

    encoder.write_all(&header).unwrap();
    // Write content, padded to 512-byte boundary
    encoder.write_all(content).unwrap();
    let padding = (512 - (content.len() % 512)) % 512;
    if padding > 0 {
        encoder.write_all(&vec![0u8; padding]).unwrap();
    }
    // Two 512-byte zero blocks as end-of-archive marker
    encoder.write_all(&[0u8; 1024]).unwrap();
    encoder.finish().unwrap();

    (rpk_path, dir)
}

/// Walk a directory and return a sorted map of relative_path → SHA-256 digest.
fn digest_tree(root: &Path) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
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
        let digest = sha256_file(entry.path()).unwrap();
        map.insert(rel, digest);
    }
    map
}

/// Strategy producing a Vec of (relative_path, file_contents) pairs.
fn arb_file_entries() -> impl Strategy<Value = Vec<(String, Vec<u8>)>> {
    // Generate 1..6 files with short path segments and small random content
    prop::collection::vec(
        (
            "[a-z]{1,4}(/[a-z]{1,4}){0,2}\\.[a-z]{1,3}",
            prop::collection::vec(any::<u8>(), 0..128),
        ),
        1..6,
    )
}

/// Materialize a set of file entries into a temp directory.
fn write_tree(entries: &[(String, Vec<u8>)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (rel, content) in entries {
        let path = dir.path().join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
    }
    dir
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// **Validates: Requirements 14.1, 14.2**
    ///
    /// For any directory tree, pack → unpack produces identical relative paths
    /// and identical file contents (verified by SHA-256 comparison).
    #[test]
    fn pack_unpack_round_trip(entries in arb_file_entries()) {
        let source_dir = write_tree(&entries);
        let archive_dir = tempfile::tempdir().unwrap();
        let rpk_path = archive_dir.path().join("test.rpk");

        pack_dir(source_dir.path(), &rpk_path).unwrap();

        let unpack_dir = tempfile::tempdir().unwrap();
        unpack_rpk(&rpk_path, unpack_dir.path()).unwrap();

        let source_digests = digest_tree(source_dir.path());
        let unpack_digests = digest_tree(unpack_dir.path());

        prop_assert_eq!(&source_digests, &unpack_digests,
            "pack/unpack round-trip produced different file tree");
    }

    /// **Validates: Requirements 14.3**
    ///
    /// Packing the same directory twice produces byte-identical archives.
    #[test]
    fn pack_determinism(entries in arb_file_entries()) {
        let source_dir = write_tree(&entries);

        let out1 = tempfile::tempdir().unwrap();
        let rpk1 = out1.path().join("a.rpk");
        pack_dir(source_dir.path(), &rpk1).unwrap();

        let out2 = tempfile::tempdir().unwrap();
        let rpk2 = out2.path().join("b.rpk");
        pack_dir(source_dir.path(), &rpk2).unwrap();

        let bytes1 = fs::read(&rpk1).unwrap();
        let bytes2 = fs::read(&rpk2).unwrap();
        prop_assert_eq!(bytes1, bytes2, "packing same dir twice produced different archives");
    }

    /// **Validates: Requirements 17.1**
    ///
    /// For any tar entry whose path contains `..`, `unpack_rpk` returns an error
    /// and does not write files outside the target directory.
    #[test]
    fn path_traversal_rejected(
        safe_name in "[a-z]{1,6}\\.[a-z]{1,3}",
        content in prop::collection::vec(any::<u8>(), 0..64),
        depth in 1..4usize,
    ) {
        // Build a path with `..` components, e.g. "../evil.txt" or "a/../../evil.txt"
        let traversal_path = if depth == 1 {
            format!("../{safe_name}")
        } else {
            let prefix: String = (0..depth - 1).map(|i| format!("d{i}/")).collect();
            let ups: String = (0..depth).map(|_| "../".to_string()).collect();
            format!("{prefix}{ups}{safe_name}")
        };

        // Create a gzip-compressed tar archive with the traversal entry using raw
        // header bytes, because the `tar` crate's Builder rejects `..` paths itself.
        let (rpk_path, _archive_guard) = build_raw_tar_gz_with_path(&traversal_path, &content);

        let target_dir = tempfile::tempdir().unwrap();
        let result = unpack_rpk(&rpk_path, target_dir.path());
        prop_assert!(result.is_err(), "unpack_rpk should reject path traversal entry: {}", traversal_path);

        // Verify no file was written outside the target directory
        let escaped_path = target_dir.path().parent().unwrap().join(&safe_name);
        prop_assert!(!escaped_path.exists(),
            "file escaped target directory: {}", escaped_path.display());
    }
}
