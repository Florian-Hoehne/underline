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
///
/// A file that is there keeps who may read and write it (its permission
/// bits; the user's decision of 2026-11-16): the temporary file is created
/// so that only its owner may read and write it (`0600` on Unix) and gets
/// the old file's permissions before it replaces it, so the content is
/// never readable by more users than before. A new file gets the system's
/// default permissions. A symbolic link at `path` is replaced by the file,
/// as before; the file keeps the permissions of the file the link led to.
///
/// The permissions are kept, not enforced: a file made read-only (`0444`)
/// is still replaced, as the folder allows, and stays read-only. Only the
/// read, write and execute bits carry over (`kept_mode`); setuid, setgid
/// and sticky bits never do. Since the file is replaced by a new one, its
/// owner and group (other than the writing user's), access control lists,
/// extended attributes, and further hard links are not kept. On a file
/// system that holds no Unix permissions and refuses to set them (FAT,
/// some SMB and FUSE mounts), the file is written without them, and this
/// is logged once (`set_kept_permissions`).
///
/// # Errors
///
/// The details of the file that is there cannot be read (see
/// `existing_permissions`: a link that leads in a circle, a folder that
/// may not be searched), or the file cannot be written. Then nothing is
/// changed, rather than writing a file readable to more users.
pub fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    path.file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let permissions = existing_permissions(path)?.map(|p| kept_mode(&p));
    let temp = temporary_path(dir, "tmp");

    let result = (|| {
        let mut file = match &permissions {
            Some(_) => create_private(&temp)?,
            None => File::create(&temp)?,
        };
        file.write_all(contents)?;
        if let Some(permissions) = &permissions {
            set_kept_permissions(&file, permissions)?;
        }
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)?;
        sync_dir(dir)
    })();
    if result.is_err() {
        remove_temporary(&temp);
    }
    result
}

/// The permissions of `permissions` a replaced or moved file keeps: on
/// Unix only the read, write and execute bits (`& 0o777`), never setuid,
/// setgid or sticky, which a file Underline writes must not gain.
fn kept_mode(permissions: &fs::Permissions) -> fs::Permissions {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::Permissions::from_mode(permissions.mode() & 0o777)
    }
    #[cfg(not(unix))]
    {
        permissions.clone()
    }
}

/// Gives `file` the permissions `permissions`. A file system that holds no
/// Unix permissions and refuses to set them (`EPERM`, `ENOTSUP`,
/// `EOPNOTSUPP`: FAT, some SMB and FUSE mounts) does not fail the write,
/// since there is no mode to keep; this is logged once per run. Any other
/// error fails it.
fn set_kept_permissions(file: &File, permissions: &fs::Permissions) -> io::Result<()> {
    match file.set_permissions(permissions.clone()) {
        Err(err) if refuses_permissions(&err) => {
            log_permissions_refused(&err);
            Ok(())
        }
        result => result,
    }
}

/// Whether `err` says the file system does not set permissions at all.
fn refuses_permissions(err: &io::Error) -> bool {
    #[cfg(unix)]
    {
        err.raw_os_error()
            .is_some_and(|code| [libc::EPERM, libc::ENOTSUP, libc::EOPNOTSUPP].contains(&code))
    }
    #[cfg(not(unix))]
    {
        let _ = err;
        false
    }
}

/// Logs once per run that a file system refused to set permissions.
fn log_permissions_refused(err: &io::Error) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static LOGGED: AtomicBool = AtomicBool::new(false);
    if !LOGGED.swap(true, Ordering::Relaxed) {
        eprintln!(
            "ADR-005: the file system does not keep file permissions ({err}); \
             files on it are written without them"
        );
    }
}

/// Removes the temporary file `temp`, also when it was made read-only
/// (Windows refuses to remove a read-only file).
fn remove_temporary(temp: &Path) {
    let removed = fs::remove_file(temp);
    #[cfg(unix)]
    let _ = removed;
    #[cfg(not(unix))]
    if removed.is_err() {
        let Ok(meta) = fs::metadata(temp) else {
            return;
        };
        let mut permissions = meta.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        if fs::set_permissions(temp, permissions).is_ok() {
            let _ = fs::remove_file(temp);
        }
    }
}

/// Writes `contents` like [`write_atomic`] to a file that may hold secrets
/// of the user, such as the configuration of an agent tool (UC-016 BR-002,
/// Q-110): a symbolic link at `path` stays a link, and the file it leads to
/// is written; the file keeps who may read and write it; a new file is
/// created so that only its owner may read and write it (`0600` on Unix).
/// A file the user may not write (made read-only, `0444`) is refused, not
/// replaced, though the folder would allow replacing it.
///
/// The new file is written beside the old one and renamed over it, so
/// only the permission bits carry over: an owner other than the writing
/// user, access control lists, extended attributes, and further hard
/// links of the old file are not kept (the other hard links keep the old
/// content).
///
/// # Errors
///
/// The link cannot be followed (it leads in a circle), the file is not
/// writable for the user, or it cannot be written.
pub fn write_atomic_private(path: &Path, contents: &[u8]) -> io::Result<()> {
    let target = link_target(path)?;
    let dir = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    target
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let permissions = match fs::metadata(&target) {
        Ok(meta) => {
            // Opening to write (without changing it) asks the system
            // whether this user may write the file.
            fs::OpenOptions::new().write(true).open(&target)?;
            Some(meta.permissions())
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => None,
        Err(err) => return Err(err),
    };
    let temp = temporary_path(dir, "tmp");
    let result = (|| {
        let mut file = create_private(&temp)?;
        file.write_all(contents)?;
        if let Some(permissions) = &permissions {
            file.set_permissions(permissions.clone())?;
        }
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, &target)?;
        sync_dir(dir)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// The file a chain of symbolic links at `path` ends in, which need not
/// exist yet; `path` itself when it is no link.
fn link_target(path: &Path) -> io::Result<std::path::PathBuf> {
    let mut current = path.to_owned();
    // As the system does: more links in a row than this lead in a circle.
    for _ in 0..40 {
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let next = fs::read_link(&current)?;
                current = match current.parent() {
                    Some(parent) if next.is_relative() => parent.join(next),
                    _ => next,
                };
            }
            Ok(_) => return Ok(current),
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(current),
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::other(format!(
        "{} is a link that leads in a circle",
        path.display()
    )))
}

/// The permissions of the file at `path` (behind a symbolic link, of the
/// file it leads to), `None` when there is no file there: nothing at
/// `path`, a link that leads nowhere, or something that is no file.
///
/// # Errors
///
/// The details cannot be read: a link that leads in a circle, or a folder
/// on the way that the user may not search. The write then fails rather
/// than guess the permissions.
fn existing_permissions(path: &Path) -> io::Result<Option<fs::Permissions>> {
    match fs::metadata(path) {
        Ok(meta) if meta.is_file() => Ok(Some(meta.permissions())),
        Ok(_) => Ok(None),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

/// A new file only its owner may read and write (Unix); elsewhere a new
/// file as the system creates it.
fn create_private(path: &Path) -> io::Result<File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// Writes a new file at `path` like [`write_atomic`], but never replaces a
/// file that is there: then the error has the kind
/// [`io::ErrorKind::AlreadyExists`] and nothing is changed (UC-003 step 5).
///
/// The content is flushed in a hidden temporary file, which is then linked
/// to `path`; linking fails when `path` exists, also when it appeared in the
/// meantime. Where links are not supported, the file is created exclusively.
pub fn write_new_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    write_new_atomic_with(path, contents, None)
}

/// Writes a new file at `path` like [`write_new_atomic`] with the
/// permissions `permissions`, such as those of the file it takes the place
/// of (a renamed note keeps its permission bits, the user's decision of
/// 2026-11-16). On Unix the content is written to a file only its owner
/// may read and write (`0600`), which gets `permissions` before it appears
/// at `path`; elsewhere (where only a read-only flag exists) `path` gets
/// them once it is there, so the temporary file can be removed. `None`
/// gives the system's default permissions.
///
/// As for [`write_atomic`], the permissions are kept, not enforced (a
/// read-only file is written and stays read-only); only the read, write
/// and execute bits carry over, never setuid, setgid or sticky; owner,
/// group, access control lists and extended attributes are not kept; and
/// a file system that refuses to set permissions does not fail the write.
///
/// # Errors
///
/// A file is at `path` ([`io::ErrorKind::AlreadyExists`]), or the file
/// cannot be written.
pub fn write_new_atomic_with(
    path: &Path,
    contents: &[u8],
    permissions: Option<&fs::Permissions>,
) -> io::Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    path.file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let permissions = permissions.map(kept_mode);
    // Unix: the permissions go on before the file appears at `path`.
    let before = permissions.as_ref().filter(|_| cfg!(unix));
    let temp = temporary_path(dir, "new");
    let result = (|| {
        let mut file = match &permissions {
            Some(_) => create_private(&temp)?,
            None => File::create(&temp)?,
        };
        file.write_all(contents)?;
        if let Some(permissions) = before {
            set_kept_permissions(&file, permissions)?;
        }
        file.sync_all()?;
        drop(file);
        match fs::hard_link(&temp, path) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => return Err(err),
            Err(_) => {
                let mut file = match &permissions {
                    Some(_) => create_private(path)?,
                    None => fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(path)?,
                };
                file.write_all(contents)?;
                if let Some(permissions) = before {
                    set_kept_permissions(&file, permissions)?;
                }
                file.sync_all()?;
            }
        }
        sync_dir(dir)
    })();
    remove_temporary(&temp);
    // Elsewhere: a read-only flag only now, since a hard link shares it
    // and the temporary file could not be removed with it. The flag
    // protects nothing from other users, so failing to set it is logged.
    #[cfg(not(unix))]
    if let (Ok(()), Some(permissions)) = (&result, &permissions) {
        if let Err(err) = fs::set_permissions(path, permissions.clone()) {
            log_permissions_refused(&err);
        }
    }
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

    /// ADR-005, decision of 2026-11-16: a replaced file keeps its
    /// permission bits, also read-only ones; a new file gets the default.
    #[cfg(unix)]
    #[test]
    fn adr005_write_keeps_the_mode_of_the_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("underline-mode-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;

        let default = dir.join("Default.md");
        fs::write(&default, "").unwrap();
        let default_mode = mode(&default);
        let new = dir.join("New.md");
        write_atomic(&new, b"# New\n").unwrap();
        assert_eq!(mode(&new), default_mode, "a new note gets the default");

        for kept in [0o600, 0o640, 0o444, 0o755] {
            let path = dir.join(format!("Note-{kept:o}.md"));
            fs::write(&path, "old").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(kept)).unwrap();
            write_atomic(&path, b"new").unwrap();
            assert_eq!(mode(&path), kept);
            assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        }
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 6, "no temporary file");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// ADR-005 (review of 2026-11-16): setuid, setgid and sticky bits are
    /// never carried over, by a replace or a new file with a mode.
    #[cfg(unix)]
    #[test]
    fn adr005_special_bits_are_never_kept() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("underline-mode-bits-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o7777;
        let path = dir.join("Tool.md");
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o6755)).unwrap();

        write_atomic(&path, b"new").unwrap();
        assert_eq!(mode(&path), 0o755);

        let new = dir.join("Moved.md");
        write_new_atomic_with(&new, b"x", Some(&fs::Permissions::from_mode(0o7640))).unwrap();
        assert_eq!(mode(&new), 0o640);
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2, "no temporary file");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// ADR-005 (review of 2026-11-16): a file system that refuses to set
    /// permissions (`EPERM`, `ENOTSUP`, `EOPNOTSUPP`) does not fail a save;
    /// any other error does.
    #[cfg(unix)]
    #[test]
    fn adr005_a_file_system_without_modes_does_not_fail_the_save() {
        for code in [libc::EPERM, libc::ENOTSUP, libc::EOPNOTSUPP] {
            assert!(refuses_permissions(&io::Error::from_raw_os_error(code)));
        }
        for code in [libc::EACCES, libc::EIO, libc::EROFS, libc::ENOSPC] {
            assert!(!refuses_permissions(&io::Error::from_raw_os_error(code)));
        }
        assert!(!refuses_permissions(&io::Error::other("no code")));
    }

    /// ADR-005: a symbolic link at the path is replaced by the file, as
    /// before the decision of 2026-11-16; the file keeps the mode of the
    /// file the link led to, which stays as it was.
    #[cfg(unix)]
    #[test]
    fn adr005_write_replaces_a_link_and_keeps_its_targets_mode() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = std::env::temp_dir().join(format!("underline-mode-link-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let real = dir.join("real.md");
        fs::write(&real, "old").unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
        let link = dir.join("Note.md");
        symlink(&real, &link).unwrap();

        write_atomic(&link, b"new").unwrap();

        let meta = fs::symlink_metadata(&link).unwrap();
        assert!(meta.file_type().is_file());
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::read_to_string(&link).unwrap(), "new");
        assert_eq!(fs::read_to_string(&real).unwrap(), "old");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// UC-006, decision of 2026-11-16: a new file can be given the mode of
    /// the file it takes the place of.
    #[cfg(unix)]
    #[test]
    fn uc006_write_new_with_the_mode_of_the_old_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("underline-new-mode-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Moved.md");
        write_new_atomic_with(&path, b"x", Some(&fs::Permissions::from_mode(0o600))).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
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

    /// UC-016 BR-002, Q-110: a file only its owner may read stays so, and
    /// a new file is created so.
    #[cfg(unix)]
    #[test]
    fn q110_private_write_keeps_the_mode_and_creates_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("underline-private-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;

        let kept = dir.join("secret.json");
        fs::write(&kept, "{}").unwrap();
        fs::set_permissions(&kept, fs::Permissions::from_mode(0o600)).unwrap();
        write_atomic_private(&kept, b"{\"a\": 1}").unwrap();
        assert_eq!(mode(&kept), 0o600);
        assert_eq!(fs::read_to_string(&kept).unwrap(), "{\"a\": 1}");

        let shared = dir.join("shared.json");
        fs::write(&shared, "{}").unwrap();
        fs::set_permissions(&shared, fs::Permissions::from_mode(0o640)).unwrap();
        write_atomic_private(&shared, b"x").unwrap();
        assert_eq!(mode(&shared), 0o640);

        let new = dir.join("new.json");
        write_atomic_private(&new, b"x").unwrap();
        assert_eq!(mode(&new), 0o600);
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            3,
            "no temporary file left"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    /// UC-016 BR-002, Q-110: a symbolic link stays a link; the file it
    /// leads to (also through a relative link, or one not there yet) is
    /// written. A circle of links is refused.
    #[cfg(unix)]
    #[test]
    fn q110_private_write_goes_through_a_link() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = std::env::temp_dir().join(format!("underline-link-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("dotfiles")).unwrap();
        let real = dir.join("dotfiles/config.json");
        fs::write(&real, "old").unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
        let link = dir.join("config.json");
        symlink("dotfiles/config.json", &link).unwrap();

        write_atomic_private(&link, b"new").unwrap();
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_to_string(&real).unwrap(), "new");
        assert_eq!(
            fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o600
        );

        let dangling = dir.join("later.json");
        symlink(dir.join("dotfiles/later.json"), &dangling).unwrap();
        write_atomic_private(&dangling, b"made").unwrap();
        assert!(fs::symlink_metadata(&dangling)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::read_to_string(dir.join("dotfiles/later.json")).unwrap(),
            "made"
        );

        let a = dir.join("a");
        let b = dir.join("b");
        symlink(&b, &a).unwrap();
        symlink(&a, &b).unwrap();
        assert!(write_atomic_private(&a, b"x").is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// UC-016 BR-002, Q-110: a file the user made read-only is refused
    /// and stays as it was.
    #[cfg(unix)]
    #[test]
    fn q110_private_write_refuses_a_read_only_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("underline-ro-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
        if fs::OpenOptions::new().write(true).open(&path).is_ok() {
            // Run with the rights to write anything (root): nothing to try.
            fs::remove_dir_all(&dir).unwrap();
            return;
        }

        let err = write_atomic_private(&path, b"new").unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(fs::read_to_string(&path).unwrap(), "old");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }
}
