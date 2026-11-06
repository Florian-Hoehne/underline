//! Atomic writes of every file Underline writes (ADR-005).

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

/// Writes `contents` to `path` so that the file holds either its previous or
/// its new content, never a half-written one (NFR-006, ADR-005).
///
/// The bytes go to a hidden temporary file in the same directory, which is
/// flushed to disk and then renamed over `path`. The directory is flushed
/// afterwards so the rename itself survives a power loss.
pub fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let temp = dir.join(format!(
        ".{}.underline-tmp-{}",
        name.to_string_lossy(),
        std::process::id()
    ));

    let result = (|| {
        let mut file = File::create(&temp)?;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)?;
        sync_dir(dir)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(unix)]
fn sync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

#[cfg(not(unix))]
fn sync_dir(_dir: &Path) -> io::Result<()> {
    // Windows offers no directory flush; the rename is already durable there.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_content_and_leaves_no_temporary_file() {
        let dir = std::env::temp_dir().join(format!("underline-atomic-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Home.md");

        write_atomic(&path, b"# Old\n").unwrap();
        write_atomic(&path, b"# New\n").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "# New\n");
        let entries: Vec<_> = fs::read_dir(&dir).unwrap().collect();
        assert_eq!(entries.len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }
}
