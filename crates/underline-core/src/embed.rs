//! Putting a file into a note (UC-005 A9–A11, A19, BR-012): a file from
//! outside the workspace is copied next to the note, a file of the workspace
//! is referred to where it is, and a large file is asked about first.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

/// Files larger than this are asked about before they are copied (A19).
pub const LARGE_FILE: u64 = 10 * 1024 * 1024;

/// Kinds of files that are inserted as images (BR-012).
pub const IMAGE_EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "gif", "svg", "webp"];

/// How a file ended up in the note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Embedded {
    /// The place the note refers to, relative to the note, with `/`.
    pub place: String,
    /// Whether it is inserted as an image.
    pub image: bool,
    /// The copy got another name, because the name was taken (A10).
    pub renamed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum EmbedError {
    /// A13: folders cannot be put into a note.
    #[error("{} is a folder; folders cannot be put into a note", .0.display())]
    Folder(PathBuf),
    /// A19: the file is larger than [`LARGE_FILE`]; the user decides.
    #[error("{} has {} MB", .path.display(), .bytes / (1024 * 1024))]
    Large { path: PathBuf, bytes: u64 },
    #[error("{} cannot be read: {source}", .path.display())]
    Unreadable { path: PathBuf, source: io::Error },
    #[error("cannot copy {} next to the note: {source}", .path.display())]
    Copy { path: PathBuf, source: io::Error },
}

/// Whether a file is inserted as an image (BR-012).
pub fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// UC-005 A9–A11: puts `file` into the note whose file is `note` in the
/// workspace at `root`. With `allow_large`, a large file is copied without
/// asking (the user agreed, A19).
pub fn embed_file(
    root: &Path,
    note: &Path,
    file: &Path,
    allow_large: bool,
) -> Result<Embedded, EmbedError> {
    let meta = fs::metadata(file).map_err(|source| EmbedError::Unreadable {
        path: file.to_owned(),
        source,
    })?;
    if meta.is_dir() {
        return Err(EmbedError::Folder(file.to_owned()));
    }
    let note_dir = note.parent().unwrap_or(root);
    let image = is_image(file);
    let absolute = std::path::absolute(file).unwrap_or_else(|_| file.to_owned());
    let root_abs = std::path::absolute(root).unwrap_or_else(|_| root.to_owned());
    // A11: a file of the workspace is referred to where it is.
    if absolute.starts_with(&root_abs) {
        let dir_abs = std::path::absolute(note_dir).unwrap_or_else(|_| note_dir.to_owned());
        return Ok(Embedded {
            place: relative(&dir_abs, &absolute),
            image,
            renamed: false,
        });
    }
    if meta.len() > LARGE_FILE && !allow_large {
        return Err(EmbedError::Large {
            path: file.to_owned(),
            bytes: meta.len(),
        });
    }
    let content = fs::read(file).map_err(|source| EmbedError::Unreadable {
        path: file.to_owned(),
        source,
    })?;
    let name = file
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_owned(), format!(".{ext}")),
        _ => (name.clone(), String::new()),
    };
    // A10: a file with the same content is used; otherwise the next number.
    for number in 1.. {
        let candidate = if number == 1 {
            name.clone()
        } else {
            format!("{stem}-{number}{extension}")
        };
        let target = note_dir.join(&candidate);
        match fs::read(&target) {
            Ok(existing) if existing == content => {
                return Ok(Embedded {
                    place: candidate,
                    image,
                    renamed: number > 1,
                })
            }
            Ok(_) => continue,
            Err(_) if target.exists() => continue,
            Err(_) => {
                crate::atomic::write_new_atomic(&target, &content).map_err(|source| {
                    EmbedError::Copy {
                        path: file.to_owned(),
                        source,
                    }
                })?;
                return Ok(Embedded {
                    place: candidate,
                    image,
                    renamed: number > 1,
                });
            }
        }
    }
    unreachable!("some number is free")
}

/// The place of `to` relative to the folder `from`, with `/`.
pub fn relative(from: &Path, to: &Path) -> String {
    let from: Vec<Component> = from.components().collect();
    let to_parts: Vec<Component> = to.components().collect();
    let common = from
        .iter()
        .zip(&to_parts)
        .take_while(|(a, b)| a == b)
        .count();
    let mut parts: Vec<String> = vec!["..".to_owned(); from.len() - common];
    parts.extend(
        to_parts[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Sandbox(PathBuf);

    impl Sandbox {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("underline-embed-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(dir.join("ws/guide")).unwrap();
            fs::create_dir_all(dir.join("outside")).unwrap();
            fs::write(dir.join("ws/guide/Setup.md"), "# Setup\n").unwrap();
            Self(dir)
        }
        fn path(&self, p: &str) -> PathBuf {
            self.0.join(p)
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// TC-005-18 / TC-005-19: a file from outside is copied next to the note;
    /// the same content is reused, other content gets a number.
    #[test]
    fn uc005_tc18_tc19_copies_next_to_the_note() {
        let s = Sandbox::new("copy");
        let (root, note) = (s.path("ws"), s.path("ws/guide/Setup.md"));
        fs::write(s.path("outside/shot.png"), "image one").unwrap();
        fs::write(s.path("outside/manual.pdf"), "pdf").unwrap();
        let shot = embed_file(&root, &note, &s.path("outside/shot.png"), false).unwrap();
        assert_eq!(
            shot,
            Embedded {
                place: "shot.png".into(),
                image: true,
                renamed: false
            }
        );
        assert_eq!(
            fs::read_to_string(s.path("ws/guide/shot.png")).unwrap(),
            "image one"
        );
        let pdf = embed_file(&root, &note, &s.path("outside/manual.pdf"), false).unwrap();
        assert!(!pdf.image);

        let again = embed_file(&root, &note, &s.path("outside/shot.png"), false).unwrap();
        assert_eq!(again.place, "shot.png");
        fs::create_dir_all(s.path("other")).unwrap();
        fs::write(s.path("other/shot.png"), "image two").unwrap();
        let other = embed_file(&root, &note, &s.path("other/shot.png"), false).unwrap();
        assert_eq!(
            other,
            Embedded {
                place: "shot-2.png".into(),
                image: true,
                renamed: true
            }
        );
        let other_again = embed_file(&root, &note, &s.path("other/shot.png"), false).unwrap();
        assert_eq!(other_again.place, "shot-2.png");
    }

    /// TC-005-20: a file of the workspace is referred to where it is.
    #[test]
    fn uc005_tc20_refers_to_workspace_files() {
        let s = Sandbox::new("inside");
        fs::create_dir_all(s.path("ws/img")).unwrap();
        fs::write(s.path("ws/img/arch.png"), "x").unwrap();
        let (root, note) = (s.path("ws"), s.path("ws/guide/Setup.md"));
        let arch = embed_file(&root, &note, &s.path("ws/img/arch.png"), false).unwrap();
        assert_eq!(arch.place, "../img/arch.png");
        assert!(!s.path("ws/guide/arch.png").exists());
    }

    /// TC-005-22 / TC-005-28: folders are refused, large files asked about.
    #[test]
    fn uc005_tc22_tc28_folders_and_large_files() {
        let s = Sandbox::new("large");
        let (root, note) = (s.path("ws"), s.path("ws/guide/Setup.md"));
        assert!(matches!(
            embed_file(&root, &note, &s.path("outside"), false),
            Err(EmbedError::Folder(_))
        ));
        let big = s.path("outside/talk.mp4");
        let file = fs::File::create(&big).unwrap();
        file.set_len(LARGE_FILE + 1).unwrap();
        assert!(matches!(
            embed_file(&root, &note, &big, false),
            Err(EmbedError::Large { .. })
        ));
        assert!(embed_file(&root, &note, &big, true).is_ok());
    }
}
