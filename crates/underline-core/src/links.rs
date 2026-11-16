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

/// UC-004 BR-005, A4: the schemes without `//` an address for another program
/// starts with. Only these: a note may be named `Note: x`, and `C:` is a drive
/// on Windows (OQ-245).
const WEB_SCHEMES: [&str; 4] = ["mailto:", "tel:", "sms:", "urn:"];

/// UC-004 BR-005: where `link`, written in the note at `note` (a file of the
/// workspace at `root`), leads.
pub fn resolve_link(root: &Path, note: &Path, link: &str) -> LinkTarget {
    let normal = normalize(root).unwrap_or_else(|| root.to_owned());
    resolve_link_with(
        root,
        note,
        link,
        |id| {
            NoteFormat::ALL
                .into_iter()
                .any(|format| normal.join(id.file_name(format)).is_file())
        },
        Path::is_file,
    )
}

/// UC-004 BR-005, BR-015: [`resolve_link`] with the questions to the disk
/// answered by the caller: `is_note` says whether the workspace holds the
/// note `id` (in any note format), `is_file` whether a file without the
/// extension of a note exists at a place. The backlinks and the outgoing
/// links (FR-078, FR-079) answer them from the notes they know, so the
/// links of 10,000 notes are resolved without asking the disk for each
/// (NFR-019).
pub fn resolve_link_with(
    root: &Path,
    note: &Path,
    link: &str,
    is_note: impl Fn(&NoteId) -> bool,
    is_file: impl Fn(&Path) -> bool,
) -> LinkTarget {
    let link = link.trim();
    // A scheme counts only before the `#`: a heading may name an address.
    let before_fragment = link.split_once('#').map_or(link, |(path, _)| path);
    if before_fragment.contains("://") && !link.starts_with("file://")
        || WEB_SCHEMES.iter().any(|scheme| {
            link.get(..scheme.len())
                .is_some_and(|start| start.eq_ignore_ascii_case(scheme))
        })
    {
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
    if is_note(&id) {
        return LinkTarget::Note { id, heading };
    }
    if is_note_extension || extension.is_none() {
        if extension.is_none() && is_file(&target) {
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

/// Decodes `%20` and the like in a link; a `%` that starts no escape stays.
pub fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            // On the bytes: a `%` before a character of several bytes is no
            // escape, and must not split it.
            let hex = bytes.get(i + 1..i + 3).and_then(|pair| {
                let high = char::from(pair[0]).to_digit(16)?;
                let low = char::from(pair[1]).to_digit(16)?;
                u8::try_from(high * 16 + low).ok()
            });
            if let Some(byte) = hex {
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

    /// UC-004 BR-005, FR-078: a `%` before a character of several bytes,
    /// or at the end, is taken as it is, never splitting the character.
    #[test]
    fn uc004_br005_percent_before_multibyte_characters() {
        assert_eq!(percent_decode("x%aé.md"), "x%aé.md");
        assert_eq!(percent_decode("%é"), "%é");
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("a%20b%C3%A9"), "a bé");
        let root = Path::new("/w");
        assert!(matches!(
            resolve_link_with(root, Path::new("/w/N.md"), "x%aé.md", |_| true, |_| false),
            LinkTarget::Note { .. }
        ));
    }
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

    /// UC-004 A4, BR-005 (chaos): an address with a scheme other than
    /// `mailto:` that has no `//` (`tel:`, `sms:`, `urn:`) is no note to
    /// create but an address for another program.
    #[test]
    fn uc004_br005_chaos_tel_is_no_note() {
        for link in ["tel:+491234567", "sms:+491234567", "urn:isbn:0451450523"] {
            let target = resolve_link_with(
                Path::new("/w"),
                Path::new("/w/N.md"),
                link,
                |_| false,
                |_| false,
            );
            assert!(
                matches!(target, LinkTarget::Web(_)),
                "{link} resolved to {target:?}"
            );
        }
    }

    /// UC-004 BR-005 (chaos): a heading named after a `#` may hold `://`;
    /// the link still leads to the note.
    #[test]
    fn uc004_br005_chaos_fragment_with_scheme_is_still_a_note_link() {
        let target = resolve_link_with(
            Path::new("/w"),
            Path::new("/w/N.md"),
            "Plan.md#see-http://x",
            |_| true,
            |_| false,
        );
        assert!(matches!(target, LinkTarget::Note { .. }), "{target:?}");
    }
}
