//! ADR-023: what the app's own protocol `ul-asset` may serve the window.
//!
//! The webview loads nothing from the network and no file by itself. Every
//! picture of the facade comes through `ul-asset`, and this module is the
//! one place that decides what it serves:
//!
//! - `file/{path}`: an image file below the open workspace's root, after
//!   `..` and symbolic links are resolved, and not in a hidden folder (such
//!   as `.git` or `.underline`, which the workspace's notes never are in);
//!   `path` is relative to the root, each segment percent-encoded;
//! - `diagram/{key}.svg`: a diagram rendered by the facade (ADR-009).
//!
//! Every other path is refused. Web images (ADR-013) are not served yet:
//! the window loads none until the user's consent is settled (Q-19).
//!
//! The session only hands out an [`AssetPlan`] (the root, or the diagrams);
//! the plan is resolved, with its file-system work, without the session.

use std::fs::Metadata;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

pub use underline_facade::images::MAX_BYTES;

use crate::session::SvgDiagrams;

/// Why `ul-asset` does not serve a path.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AssetError {
    /// The path names nothing the protocol serves: outside the workspace,
    /// in a hidden folder, not an image, too large, or of an unknown kind.
    #[error("refused")]
    Refused,
    /// The file or the diagram is not there (any more).
    #[error("not found")]
    NotFound,
}

/// What `ul-asset` answers with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asset {
    /// A rendered diagram.
    Svg(Arc<str>),
    /// An image file of the workspace, read by the caller.
    File {
        /// The file, resolved.
        path: PathBuf,
        /// Its media type.
        mime: &'static str,
    },
}

/// What the session knows of a request, resolved by [`AssetPlan::resolve`]
/// without holding the session.
#[derive(Debug)]
pub struct AssetPlan(Plan);

#[derive(Debug)]
enum Plan {
    Diagram {
        diagrams: Arc<SvgDiagrams>,
        name: String,
    },
    File {
        root: PathBuf,
        relative: String,
    },
}

impl AssetPlan {
    /// The diagram `name` of `diagrams`.
    pub(crate) fn diagram(diagrams: Arc<SvgDiagrams>, name: &str) -> Self {
        Self(Plan::Diagram {
            diagrams,
            name: name.to_owned(),
        })
    }

    /// The file `relative` (percent-decoded) below `root`.
    pub(crate) fn file(root: PathBuf, relative: String) -> Self {
        Self(Plan::File { root, relative })
    }

    /// What the protocol serves for the plan; the file-system checks run
    /// here.
    ///
    /// # Errors
    ///
    /// [`AssetError::Refused`] for a path the protocol does not serve,
    /// [`AssetError::NotFound`] for a file or diagram that is not there.
    pub fn resolve(self) -> Result<Asset, AssetError> {
        match self.0 {
            Plan::Diagram { diagrams, name } => diagrams
                .svg(&name)
                .map(Asset::Svg)
                .ok_or(AssetError::NotFound),
            Plan::File { root, relative } => {
                let path = workspace_file(&root, &relative)?;
                let mime = image_mime(&path).ok_or(AssetError::Refused)?;
                Ok(Asset::File { path, mime })
            }
        }
    }
}

/// The media type of an image file, by its extension; `None` for every
/// other file, which the protocol does not serve.
#[must_use]
pub fn image_mime(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "avif" => "image/avif",
        _ => return None,
    })
}

/// Why a file of a note is not served (and not shown).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileProblem {
    /// Below no root, or the path is not relative.
    Outside,
    /// In a hidden folder or itself hidden.
    Hidden,
    /// Not an image by its extension.
    NotImage,
    /// Not there, or not a file.
    NotFound,
    /// Larger than [`MAX_BYTES`].
    TooLarge,
}

impl From<FileProblem> for AssetError {
    fn from(problem: FileProblem) -> Self {
        match problem {
            FileProblem::NotFound => Self::NotFound,
            FileProblem::Outside
            | FileProblem::Hidden
            | FileProblem::NotImage
            | FileProblem::TooLarge => Self::Refused,
        }
    }
}

/// The rules a requested path must keep before it is looked up: relative,
/// no backslash, no NUL, and on Windows no `:` (a drive or an alternate data
/// stream).
pub(crate) fn check_relative(relative: &str, windows: bool) -> Result<(), FileProblem> {
    if relative.is_empty() || relative.contains('\0') || relative.contains('\\') {
        return Err(FileProblem::Outside);
    }
    let path = Path::new(relative);
    let rooted = path
        .components()
        .any(|c| matches!(c, Component::Prefix(_) | Component::RootDir));
    if rooted || path.is_absolute() || relative.starts_with('/') {
        return Err(FileProblem::Outside);
    }
    if windows && relative.split('/').any(|segment| segment.contains(':')) {
        return Err(FileProblem::Outside);
    }
    Ok(())
}

/// The image file `path` below `root`, after `..` and symbolic links are
/// resolved: its resolved place, its place relative to the resolved root
/// and its metadata.
pub(crate) fn check_file(
    root: &Path,
    path: &Path,
) -> Result<(PathBuf, PathBuf, Metadata), FileProblem> {
    let root = root.canonicalize().map_err(|_| FileProblem::NotFound)?;
    let resolved = path.canonicalize().map_err(|_| FileProblem::NotFound)?;
    let relative = resolved
        .strip_prefix(&root)
        .map_err(|_| FileProblem::Outside)?
        .to_owned();
    let hidden = relative.components().any(|c| match c {
        Component::Normal(name) => name.to_string_lossy().starts_with('.'),
        _ => true,
    });
    if hidden {
        return Err(FileProblem::Hidden);
    }
    if image_mime(&resolved).is_none() {
        return Err(FileProblem::NotImage);
    }
    let meta = std::fs::metadata(&resolved).map_err(|_| FileProblem::NotFound)?;
    if !meta.is_file() {
        return Err(FileProblem::NotFound);
    }
    if meta.len() > MAX_BYTES {
        return Err(FileProblem::TooLarge);
    }
    Ok((resolved, relative, meta))
}

/// The image file `relative` (already percent-decoded) below `root`, after
/// `..` and symbolic links are resolved.
///
/// # Errors
///
/// [`AssetError::Refused`] for an absolute path, a path that leads outside
/// the root or into a hidden folder, a file that is not an image or is too
/// large; [`AssetError::NotFound`] when the file does not exist.
pub fn workspace_file(root: &Path, relative: &str) -> Result<PathBuf, AssetError> {
    check_relative(relative, cfg!(windows))?;
    let (resolved, _, _) = check_file(root, &root.join(relative))?;
    Ok(resolved)
}

/// The path of the protocol for the file `relative` to the root
/// (`file/…`), each segment percent-encoded.
#[must_use]
pub fn file_asset_path(relative: &Path) -> String {
    let mut out = String::from("file");
    for component in relative.components() {
        if let Component::Normal(segment) = component {
            out.push('/');
            out.push_str(&percent_encode(&segment.to_string_lossy()));
        }
    }
    out
}

/// `text` with every byte but the unreserved characters of a URL
/// percent-encoded.
#[must_use]
pub fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// `text` with its percent escapes decoded; `None` when it is not valid
/// UTF-8 afterwards or holds a broken escape.
#[must_use]
pub fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let hex = text.get(at + 1..at + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The protocol path of a request: the URL's path without its leading `/`
/// and without a query (`?v=…` only makes a changed picture load again).
#[must_use]
pub fn request_path(uri_path: &str) -> &str {
    let path = uri_path.split(['?', '#']).next().unwrap_or("");
    path.trim_start_matches('/')
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "underline-session-assets-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A workspace `dir/ws` with `img/plan.png` and `notes.md`, and a
    /// picture beside it, outside.
    fn workspace(name: &str) -> (PathBuf, PathBuf) {
        let dir = temp_dir(name);
        let root = dir.join("ws");
        fs::create_dir_all(root.join("img")).unwrap();
        fs::create_dir_all(root.join(".underline")).unwrap();
        fs::write(root.join("img/plan.png"), b"png").unwrap();
        fs::write(root.join("img/a b ü.svg"), b"<svg/>").unwrap();
        fs::write(root.join(".underline/cache.png"), b"png").unwrap();
        fs::write(root.join("notes.md"), b"# Notes").unwrap();
        fs::write(dir.join("secret.png"), b"secret").unwrap();
        (dir, root)
    }

    #[test]
    fn adr023_an_image_below_the_root_is_served() {
        let (_dir, root) = workspace("inside");

        let served = workspace_file(&root, "img/plan.png").unwrap();

        assert_eq!(served, root.canonicalize().unwrap().join("img/plan.png"));
        // `..` that stays inside is resolved.
        assert!(workspace_file(&root, "img/../img/plan.png").is_ok());
    }

    #[test]
    fn adr023_paths_that_escape_the_root_are_refused() {
        let (dir, root) = workspace("escape");

        assert_eq!(
            workspace_file(&root, "../secret.png"),
            Err(AssetError::Refused)
        );
        assert_eq!(
            workspace_file(&root, "img/../../secret.png"),
            Err(AssetError::Refused)
        );
        let absolute = dir.join("secret.png").display().to_string();
        assert_eq!(workspace_file(&root, &absolute), Err(AssetError::Refused));
        assert_eq!(
            workspace_file(&root, "img\\..\\..\\secret.png"),
            Err(AssetError::Refused)
        );
        assert_eq!(workspace_file(&root, ""), Err(AssetError::Refused));
        // Decoding first does not open a way out.
        let decoded = percent_decode("%2e%2e/secret.png").unwrap();
        assert_eq!(workspace_file(&root, &decoded), Err(AssetError::Refused));
    }

    #[test]
    fn adr023_hidden_folders_are_not_served() {
        let (_dir, root) = workspace("hidden");

        assert_eq!(
            workspace_file(&root, ".underline/cache.png"),
            Err(AssetError::Refused)
        );
        assert_eq!(
            check_file(&root, &root.join(".underline/cache.png")).map(|_| ()),
            Err(FileProblem::Hidden)
        );
    }

    /// The rules for Windows are checked on every system.
    #[test]
    fn adr023_windows_style_paths_are_refused() {
        for path in [
            "C:\\Windows\\win.ini",
            "C:/Windows/win.png",
            "img/plan.png:hidden",
            "\\\\server\\share\\a.png",
            "//server/share/a.png",
            "/etc/a.png",
        ] {
            assert_eq!(
                check_relative(path, true),
                Err(FileProblem::Outside),
                "{path}"
            );
        }
        // A colon is a name on other systems.
        assert_eq!(check_relative("img/a:b.png", false), Ok(()));
        assert_eq!(check_relative("img/plan.png", true), Ok(()));
    }

    #[cfg(unix)]
    #[test]
    fn adr023_a_symbolic_link_out_of_the_root_is_refused() {
        let (dir, root) = workspace("symlink");
        std::os::unix::fs::symlink(dir.join("secret.png"), root.join("img/link.png")).unwrap();
        std::os::unix::fs::symlink(&dir, root.join("up")).unwrap();

        assert_eq!(
            workspace_file(&root, "img/link.png"),
            Err(AssetError::Refused)
        );
        assert_eq!(
            workspace_file(&root, "up/secret.png"),
            Err(AssetError::Refused)
        );
    }

    #[test]
    fn adr023_only_images_are_served() {
        let (_dir, root) = workspace("images-only");

        assert_eq!(workspace_file(&root, "notes.md"), Err(AssetError::Refused));
        assert_eq!(
            workspace_file(&root, "img/missing.png"),
            Err(AssetError::NotFound)
        );
    }

    #[test]
    fn adr023_file_paths_round_trip_through_percent_encoding() {
        let (_dir, root) = workspace("encoding");

        let path = file_asset_path(Path::new("img/a b ü.svg"));

        assert_eq!(path, "file/img/a%20b%20%C3%BC.svg");
        let relative = percent_decode(path.strip_prefix("file/").unwrap()).unwrap();
        assert!(workspace_file(&root, &relative).is_ok());
        assert_eq!(percent_decode("%zz"), None);
        assert_eq!(percent_decode("%"), None);
    }

    #[test]
    fn adr023_the_request_path_drops_the_slash_and_the_query() {
        assert_eq!(request_path("/file/a.png?v=12"), "file/a.png");
        assert_eq!(request_path("/diagram/00ff.svg"), "diagram/00ff.svg");
        assert_eq!(request_path(""), "");
    }
}
