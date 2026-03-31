use std::fs;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use repropack_model::IntegrityEntry;
use sha2::{Digest, Sha256};
use tar::{Archive, Builder, Header};
use tempfile::TempDir;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct MaterializedPacket {
    pub root: PathBuf,
    pub warnings: Vec<String>,
    _tempdir: Option<TempDir>,
}

impl MaterializedPacket {
    pub fn manifest_path(&self) -> PathBuf {
        self.root.join("manifest.json")
    }
}

pub fn materialize(packet: &Path) -> Result<MaterializedPacket> {
    if packet.is_dir() {
        let warnings = verify_or_warn(packet)?;
        return Ok(MaterializedPacket {
            root: packet.to_path_buf(),
            warnings,
            _tempdir: None,
        });
    }

    let tempdir = tempfile::tempdir().context("creating temp dir for packet materialization")?;
    unpack_rpk(packet, tempdir.path()).context("unpacking packet archive")?;
    let warnings = verify_or_warn(tempdir.path())?;
    Ok(MaterializedPacket {
        root: tempdir.path().to_path_buf(),
        warnings,
        _tempdir: Some(tempdir),
    })
}

/// Check integrity.json if present; return warnings if absent.
fn verify_or_warn(packet_root: &Path) -> Result<Vec<String>> {
    let integrity_path = packet_root.join("integrity.json");
    if integrity_path.exists() {
        verify_integrity(packet_root)?;
        Ok(Vec::new())
    } else {
        Ok(vec![
            "integrity.json absent; proceeding without verification".to_string(),
        ])
    }
}

pub fn pack_dir(source: &Path, target_rpk: &Path) -> Result<()> {
    if !source.is_dir() {
        return Err(anyhow!("source is not a directory: {}", source.display()));
    }

    if let Some(parent) = target_rpk.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }

    let file = fs::File::create(target_rpk)
        .with_context(|| format!("creating archive {}", target_rpk.display()))?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = Builder::new(encoder);

    let mut files: Vec<PathBuf> = WalkDir::new(source)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.path().strip_prefix(source).unwrap().to_path_buf())
        .collect();
    files.sort();

    for relative in files {
        let full_path = source.join(&relative);
        let bytes =
            fs::read(&full_path).with_context(|| format!("reading {}", full_path.display()))?;

        let mut header = Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(detect_mode(&full_path));
        header.set_mtime(0);
        header.set_cksum();

        builder
            .append_data(&mut header, &relative, Cursor::new(bytes))
            .with_context(|| format!("writing {}", relative.display()))?;
    }

    builder.finish().context("finishing tar stream")?;
    let encoder = builder.into_inner().context("recovering gzip encoder")?;
    encoder.finish().context("finishing gzip stream")?;
    Ok(())
}

pub fn unpack_rpk(source_rpk: &Path, target_dir: &Path) -> Result<()> {
    fs::create_dir_all(target_dir).with_context(|| format!("creating {}", target_dir.display()))?;

    let file = fs::File::open(source_rpk)
        .with_context(|| format!("opening archive {}", source_rpk.display()))?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    for entry in archive.entries().context("reading archive entries")? {
        let mut entry = entry.context("reading archive member")?;
        let relative = entry.path().context("reading archive path")?.into_owned();
        ensure_safe_relative(&relative)?;

        let output_path = target_dir.join(&relative);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }

        entry
            .unpack(&output_path)
            .with_context(|| format!("unpacking {}", output_path.display()))?;
    }

    Ok(())
}

/// Compute SHA-256 of a byte slice, returning the hex-encoded digest string.
pub fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 16 * 1024];

    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Verify all files in a materialized packet against `integrity.json`.
///
/// Reads `integrity.json` from `packet_root`, parses it as `Vec<IntegrityEntry>`,
/// then checks each entry's SHA-256 digest against the actual file on disk.
/// Returns `Ok(())` if all digests match, or an error identifying the first mismatch.
pub fn verify_integrity(packet_root: &Path) -> Result<()> {
    let integrity_path = packet_root.join("integrity.json");
    let bytes = fs::read(&integrity_path)
        .with_context(|| format!("reading {}", integrity_path.display()))?;
    let entries: Vec<IntegrityEntry> = serde_json::from_slice(&bytes)
        .with_context(|| format!("parsing {}", integrity_path.display()))?;

    for entry in &entries {
        let file_path = packet_root.join(&entry.relative_path);
        let actual_digest = sha256_file(&file_path).with_context(|| {
            format!(
                "computing digest for integrity check: {}",
                entry.relative_path
            )
        })?;
        if actual_digest != entry.sha256 {
            return Err(anyhow!(
                "integrity mismatch for {}: expected {}, observed {}",
                entry.relative_path,
                entry.sha256,
                actual_digest
            ));
        }
    }

    Ok(())
}

fn ensure_safe_relative(path: &Path) -> Result<()> {
    if path.is_absolute() {
        return Err(anyhow!("archive member is absolute: {}", path.display()));
    }

    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir => {
                return Err(anyhow!(
                    "archive member escapes target directory: {}",
                    path.display()
                ))
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(anyhow!(
                    "archive member has invalid root: {}",
                    path.display()
                ))
            }
        }
    }

    Ok(())
}

#[cfg(unix)]
fn detect_mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path)
        .map(|meta| meta.permissions().mode())
        .unwrap_or(0o644)
}

#[cfg(not(unix))]
fn detect_mode(_path: &Path) -> u32 {
    0o644
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_dir_entries() {
        let bad = PathBuf::from("../evil");
        assert!(ensure_safe_relative(&bad).is_err());
    }

    #[test]
    fn sha256_bytes_empty() {
        let digest = sha256_bytes(b"");
        assert_eq!(
            digest,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_bytes_hello() {
        let digest = sha256_bytes(b"hello");
        assert_eq!(
            digest,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn verify_integrity_passes_on_valid_packet() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Write a sample file
        fs::write(root.join("hello.txt"), b"hello").unwrap();
        let digest = sha256_bytes(b"hello");

        // Write integrity.json
        let entries = vec![IntegrityEntry {
            relative_path: "hello.txt".to_string(),
            sha256: digest,
            size_bytes: 5,
        }];
        let json = serde_json::to_vec_pretty(&entries).unwrap();
        fs::write(root.join("integrity.json"), json).unwrap();

        assert!(verify_integrity(root).is_ok());
    }

    #[test]
    fn verify_integrity_fails_on_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Write a sample file
        fs::write(root.join("hello.txt"), b"hello").unwrap();

        // Write integrity.json with wrong digest
        let entries = vec![IntegrityEntry {
            relative_path: "hello.txt".to_string(),
            sha256: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            size_bytes: 5,
        }];
        let json = serde_json::to_vec_pretty(&entries).unwrap();
        fs::write(root.join("integrity.json"), json).unwrap();

        let err = verify_integrity(root).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("integrity mismatch for hello.txt"));
        assert!(msg.contains("expected 000000"));
    }

    #[test]
    fn materialize_dir_with_integrity_passes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("data.txt"), b"test data").unwrap();
        let digest = sha256_bytes(b"test data");
        let entries = vec![IntegrityEntry {
            relative_path: "data.txt".to_string(),
            sha256: digest,
            size_bytes: 9,
        }];
        fs::write(
            root.join("integrity.json"),
            serde_json::to_vec_pretty(&entries).unwrap(),
        )
        .unwrap();

        let result = materialize(root).unwrap();
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn materialize_dir_without_integrity_warns() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("data.txt"), b"test data").unwrap();

        let result = materialize(root).unwrap();
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("integrity.json absent"));
    }

    #[test]
    fn materialize_dir_with_bad_integrity_fails() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("data.txt"), b"test data").unwrap();
        let entries = vec![IntegrityEntry {
            relative_path: "data.txt".to_string(),
            sha256: "bad_digest".to_string(),
            size_bytes: 9,
        }];
        fs::write(
            root.join("integrity.json"),
            serde_json::to_vec_pretty(&entries).unwrap(),
        )
        .unwrap();

        let err = materialize(root).unwrap_err();
        assert!(err.to_string().contains("integrity mismatch"));
    }
}
