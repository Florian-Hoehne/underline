//! Where a link in a note leads (UC-004 A2–A4, BR-005, BR-007).

use std::path::{Component, Path, PathBuf};

use crate::format::NoteFormat;
use crate::note::NoteId;

/// What a followed link points to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTarget {
    /// A heading of the same note, by its anchor.
    Heading(String),
    /// A note of the workspace, with the anchor of a heading if the link
    /// names one (A2).
    Note { id: NoteId, heading: Option<String> },
    /// A note that does not exist yet (A3): its folder and title.
    MissingNote { folder: String, title: String },
    /// A web address (A4).
    Web(String),
    /// A file of the workspace that is not a note, or a file on this computer
    /// linked by its full place (A4, UC-005 A19).
    File(PathBuf),
    /// A relative link that leads outside the workspace; it is not followed.
    Outside,
}

/// UC-004 BR-005: where `link`, written in the note at `note` (a file of the
/// workspace at `root`), leads.
pub fn resolve_link(root: &Path, note: &Path, link: &str) -> LinkTarget {
    let link = link.trim();
    if link.contains("://") && !link.starts_with("file://") || link.starts_with("mailto:") {
        return LinkTarget::Web(link.to_owned());
    }
    if let Some(path) = link.strip_prefix("file://") {
        return LinkTarget::File(PathBuf::from(path));
    }
    let (path, heading) = match link.split_once('#') {
        Some((path, heading)) => (path, Some(heading.to_owned())),
        None => (link, None),
    };
    if path.is_empty() {
        return LinkTarget::Heading(heading.unwrap_or_default());
    }
    let path = percent_decode(path);
    if Path::new(&path).is_absolute() {
        return LinkTarget::File(PathBuf::from(path));
    }
    let base = note.parent().unwrap_or(root);
    let Some(target) = normalize(&base.join(&path)) else {
        return LinkTarget::Outside;
    };
    let root = normalize(root).unwrap_or_else(|| root.to_owned());
    let Ok(relative) = target.strip_prefix(&root) else {
        return LinkTarget::Outside;
    };
    let relative = relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");
    let extension = Path::new(&relative).extension().and_then(|e| e.to_str());
    let is_note_extension = extension.and_then(NoteFormat::from_extension).is_some();
    let stem = if is_note_extension {
        relative
            .rsplit_once('.')
            .map_or(relative.as_str(), |(s, _)| s)
    } else {
        relative.as_str()
    };
    let id = NoteId::from_relative(stem);
    let exists = NoteFormat::ALL
        .into_iter()
        .any(|format| root.join(id.file_name(format)).is_file());
    if exists {
        return LinkTarget::Note { id, heading };
    }
    if is_note_extension || extension.is_none() {
        if extension.is_none() && target.is_file() {
            return LinkTarget::File(target);
        }
        return LinkTarget::MissingNote {
            folder: id.folder().unwrap_or("").to_owned(),
            title: id.title(),
        };
    }
    LinkTarget::File(target)
}

/// The anchor GitHub gives a heading: lower case, spaces as dashes, other
/// punctuation dropped.
pub fn heading_anchor(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

/// The id Asciidoctor gives a section title: `_`, then the title in lower
/// case with every run of other characters as one `_`.
pub fn asciidoc_anchor(heading: &str) -> String {
    let mut id = String::from("_");
    for c in heading.trim().to_lowercase().chars() {
        if c.is_alphanumeric() {
            id.push(c);
        } else if !id.ends_with('_') {
            id.push('_');
        }
    }
    let trimmed = id.trim_end_matches('_').len().max(1);
    id.truncate(trimmed);
    id
}

/// `path` without `.` and `..`, or `None` when it climbs above its root.
pub(crate) fn normalize(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            other => out.push(other),
        }
    }
    Some(out)
}

/// Decodes `%20` and the like in a link.
pub(crate) fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&text[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// TC-004-12 to TC-004-15: where links lead.
    #[test]
    fn uc004_tc12_to_tc15_where_links_lead() {
        let root = std::env::temp_dir().join(format!("underline-links-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("guide")).unwrap();
        fs::create_dir_all(root.join("docs")).unwrap();
        for file in [
            "Home.md",
            "guide/Index.md",
            "guide/Install.md",
            "guide/Upgrade.adoc",
            "docs/manual.pdf",
        ] {
            fs::write(root.join(file), "").unwrap();
        }
        let note = root.join("guide/Index.md");
        let id = |s: &str| NoteId::from_relative(s);
        assert_eq!(
            resolve_link(&root, &note, "Install.md"),
            LinkTarget::Note {
                id: id("guide/Install"),
                heading: None
            }
        );
        assert_eq!(
            resolve_link(&root, &note, "Upgrade"),
            LinkTarget::Note {
                id: id("guide/Upgrade"),
                heading: None
            }
        );
        assert_eq!(
            resolve_link(&root, &note, "../Home.md#welcome"),
            LinkTarget::Note {
                id: id("Home"),
                heading: Some("welcome".into())
            }
        );
        assert_eq!(
            resolve_link(&root, &note, "#usage"),
            LinkTarget::Heading("usage".into())
        );
        assert_eq!(
            resolve_link(&root, &note, "Roadmap.md"),
            LinkTarget::MissingNote {
                folder: "guide".into(),
                title: "Roadmap".into()
            }
        );
        assert_eq!(
            resolve_link(&root, &note, "https://example.org/x"),
            LinkTarget::Web("https://example.org/x".into())
        );
        assert_eq!(
            resolve_link(&root, &note, "../docs/manual.pdf"),
            LinkTarget::File(root.join("docs/manual.pdf"))
        );
        assert_eq!(
            resolve_link(&root, &note, "../../etc/passwd"),
            LinkTarget::Outside
        );
        assert_eq!(
            resolve_link(&root, &note, "/home/me/talk.mp4"),
            LinkTarget::File(PathBuf::from("/home/me/talk.mp4"))
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn heading_anchors_as_github_makes_them() {
        assert_eq!(heading_anchor("Getting Started!"), "getting-started");
        assert_eq!(heading_anchor("Setup > Linux"), "setup--linux");
        assert_eq!(heading_anchor("Über uns"), "über-uns");
    }

    #[test]
    fn heading_anchors_as_asciidoctor_makes_them() {
        assert_eq!(asciidoc_anchor("Getting Started!"), "_getting_started");
        assert_eq!(asciidoc_anchor("Setup > Linux"), "_setup_linux");
    }
}
