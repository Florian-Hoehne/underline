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
    path.file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let temp = temporary_path(dir, "tmp");

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

/// Writes a new file at `path` like [`write_atomic`], but never replaces a
/// file that is there: then the error has the kind
/// [`io::ErrorKind::AlreadyExists`] and nothing is changed (UC-003 step 5).
///
/// The content is flushed in a hidden temporary file, which is then linked
/// to `path`; linking fails when `path` exists, also when it appeared in the
/// meantime. Where links are not supported, the file is created exclusively.
pub fn write_new_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    path.file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let temp = temporary_path(dir, "new");
    let result = (|| {
        let mut file = File::create(&temp)?;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        match fs::hard_link(&temp, path) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => return Err(err),
            Err(_) => {
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)?;
                file.write_all(contents)?;
                file.sync_all()?;
            }
        }
        sync_dir(dir)
    })();
    let _ = fs::remove_file(&temp);
    result
}

/// A hidden temporary file in `dir`, short enough for any file name limit
/// and unique within this process.
fn temporary_path(dir: &Path, kind: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    dir.join(format!(".underline-{kind}-{}-{n}", std::process::id()))
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

    /// UC-003 step 5: a new file never replaces one that is there.
    #[test]
    fn uc003_write_new_never_replaces_a_file() {
        let dir = std::env::temp_dir().join(format!("underline-atomic-new-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Race.md");
        write_new_atomic(&path, b"first").unwrap();
        let err = write_new_atomic(&path, b"second").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&path).unwrap(), "first");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }
}
