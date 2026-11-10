//! The sections of a note, for folding (FR-027, UC-004 BR-012): a heading
//! with everything below it up to the next heading of the same or a higher
//! level. Found line by line, so it is cheap enough for every change; lines
//! inside code blocks and other delimited blocks are not headings.

use std::ops::Range;

use crate::NoteFormat;

/// Names a section so that it stays the same section while other parts of
/// the note change: the text of its heading line and how many headings with
/// the same text come before it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SectionKey {
    pub heading: String,
    pub occurrence: usize,
}

/// One section of a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub key: SectionKey,
    /// The heading level, 1 for the highest.
    pub level: usize,
    /// The bytes of the heading line, with its line break.
    pub heading: Range<usize>,
    /// The bytes below the heading that folding hides.
    pub body: Range<usize>,
}

/// UC-004 BR-012: the sections of `text`, a note in `format`, in order.
pub fn sections(text: &str, format: NoteFormat) -> Vec<Section> {
    let mut headings: Vec<(usize, Range<usize>, String)> = Vec::new();
    let mut fence: Option<String> = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let start = offset;
        offset += line.len();
        let content = line.trim_end_matches(['\n', '\r']);
        match format {
            NoteFormat::Markdown => {
                let trimmed = content.trim_start();
                let indent = content.len() - trimmed.len();
                let marker: String = trimmed
                    .chars()
                    .take_while(|&c| c == '`' || c == '~')
                    .collect();
                if indent < 4
                    && marker.len() >= 3
                    && marker
                        .chars()
                        .all(|c| c == marker.chars().next().unwrap_or('`'))
                {
                    match &fence {
                        None => {
                            fence = Some(marker);
                            continue;
                        }
                        Some(open)
                            if marker.starts_with(open.as_str())
                                && trimmed[marker.len()..].trim().is_empty() =>
                        {
                            fence = None;
                            continue;
                        }
                        _ => {}
                    }
                }
                if fence.is_some() || indent >= 4 {
                    continue;
                }
                let level = trimmed.chars().take_while(|&c| c == '#').count();
                let rest = &trimmed[level..];
                if (1..=6).contains(&level) && (rest.is_empty() || rest.starts_with([' ', '\t'])) {
                    headings.push((level, start..offset, content.trim().to_owned()));
                }
            }
            NoteFormat::AsciiDoc => {
                if is_asciidoc_delimiter(content) {
                    match &fence {
                        None => fence = Some(content.to_owned()),
                        Some(open) if open == content => fence = None,
                        _ => {}
                    }
                    continue;
                }
                if fence.is_some() {
                    continue;
                }
                let level = content.chars().take_while(|&c| c == '=').count();
                let rest = &content[level..];
                if (1..=6).contains(&level) && rest.starts_with(' ') && !rest.trim().is_empty() {
                    headings.push((level, start..offset, content.trim().to_owned()));
                }
            }
        }
    }
    let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let mut sections = Vec::with_capacity(headings.len());
    for (i, (level, heading, line)) in headings.iter().enumerate() {
        let end = headings[i + 1..]
            .iter()
            .find(|(other, _, _)| other <= level)
            .map_or(text.len(), |(_, range, _)| range.start);
        let occurrence = seen.entry(line.as_str()).or_insert(0);
        sections.push(Section {
            key: SectionKey {
                heading: line.clone(),
                occurrence: *occurrence,
            },
            level: *level,
            heading: heading.clone(),
            body: heading.end.min(end)..end,
        });
        *occurrence += 1;
    }
    sections
}

/// A line that opens or closes a delimited AsciiDoc block.
fn is_asciidoc_delimiter(line: &str) -> bool {
    if line == "--" {
        return true;
    }
    if line.starts_with("|===") {
        return line[1..].chars().all(|c| c == '=');
    }
    let Some(first) = line.chars().next() else {
        return false;
    };
    line.len() >= 4
        && matches!(first, '-' | '.' | '=' | '*' | '_' | '+' | '/')
        && line.chars().all(|c| c == first)
}

/// The innermost section at byte `offset`: on its heading or in its body.
pub fn section_at(sections: &[Section], offset: usize) -> Option<&Section> {
    sections
        .iter()
        .filter(|s| s.heading.start <= offset && offset < s.body.end.max(s.heading.end))
        .max_by_key(|s| s.level)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TC-004-09: folding `Setup` hides `Linux` too; `Usage` follows.
    #[test]
    fn uc004_tc09_sections_nest_by_level() {
        let note = "# Intro\ntext\n# Setup\nintro\n## Linux\napt\n```sh\n# not a heading\n```\n# Usage\nrun\n";
        let found = sections(note, NoteFormat::Markdown);
        let titles: Vec<_> = found
            .iter()
            .map(|s| (s.key.heading.as_str(), s.level))
            .collect();
        assert_eq!(
            titles,
            [
                ("# Intro", 1),
                ("# Setup", 1),
                ("## Linux", 2),
                ("# Usage", 1)
            ]
        );
        let setup = &found[1];
        assert_eq!(
            &note[setup.body.clone()],
            "intro\n## Linux\napt\n```sh\n# not a heading\n```\n"
        );
        let linux = &found[2];
        assert_eq!(linux.body.end, setup.body.end);
        assert!(note[found[3].body.clone()].starts_with("run"));
        // The innermost section at a place.
        let apt = note.find("apt").unwrap();
        assert_eq!(section_at(&found, apt).unwrap().key.heading, "## Linux");
        assert_eq!(
            section_at(&found, note.find("intro").unwrap())
                .unwrap()
                .key
                .heading,
            "# Setup"
        );
        assert!(section_at(&found, 0).is_some());
    }

    #[test]
    fn same_headings_are_told_apart_and_asciidoc_blocks_skipped() {
        let note = "== Notes\nx\n== Notes\ny\n----\n== code\n----\n";
        let found = sections(note, NoteFormat::AsciiDoc);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].key.occurrence, 0);
        assert_eq!(found[1].key.occurrence, 1);
        assert_eq!(&note[found[1].body.clone()], "y\n----\n== code\n----\n");
    }
}
