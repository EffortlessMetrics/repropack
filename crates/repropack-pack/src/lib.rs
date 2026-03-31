use std::fs;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use sha2::{Digest, Sha256};
use tar::{Archive, Builder, Header};
use tempfile::TempDir;
use walkdir::WalkDir;

pub struct MaterializedPacket {
    pub root: PathBuf,
    _tempdir: Option<TempDir>,
}

impl MaterializedPacket {
    pub fn manifest_path(&self) -> PathBuf {
        self.root.join("manifest.json")
    }
}

pub fn materialize(packet: &Path) -> Result<MaterializedPacket> {
    if packet.is_dir() {
        return Ok(MaterializedPacket {
            root: packet.to_path_buf(),
            _tempdir: None,
        });
    }

    let tempdir = tempfile::tempdir().context("creating temp dir for packet materialization")?;
    unpack_rpk(packet, tempdir.path()).context("unpacking packet archive")?;
    Ok(MaterializedPacket {
        root: tempdir.path().to_path_buf(),
        _tempdir: Some(tempdir),
    })
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
        let bytes = fs::read(&full_path)
            .with_context(|| format!("reading {}", full_path.display()))?;

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
    fs::create_dir_all(target_dir)
        .with_context(|| format!("creating {}", target_dir.display()))?;

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
            fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }

        entry
            .unpack(&output_path)
            .with_context(|| format!("unpacking {}", output_path.display()))?;
    }

    Ok(())
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
                return Err(anyhow!("archive member has invalid root: {}", path.display()))
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
}
