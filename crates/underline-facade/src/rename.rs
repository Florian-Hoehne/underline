//! UC-006 Rename Note: finds the links and the first heading in the markup
//! of the notes, changes them for a rename, and carries the rename out over
//! the whole workspace.

use std::ops::Range;

use comrak::nodes::NodeValue;
use comrak::{parse_document, Arena, Options};
use underline_core::{write_atomic, NewNoteProblem, NoteFormat, NoteId, NoteMove, Workspace};

/// A change of a note's text: the bytes in the range are replaced.
pub type Edit = (Range<usize>, String);

/// A link in the markup of a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkSpan {
    /// The bytes of the place the link names, as written.
    pub target: Range<usize>,
    /// The bytes of the text the link shows, when it has its own text.
    pub text: Option<Range<usize>>,
}

/// Every link of `source` that names a place: links, images, and
/// references, not those in code.
pub fn link_spans(source: &str, format: NoteFormat) -> Vec<LinkSpan> {
    let mut spans = match format {
        NoteFormat::Markdown => markdown_links(source),
        NoteFormat::AsciiDoc => asciidoc_links(source),
    };
    spans.sort_by_key(|span| span.target.start);
    spans.dedup_by_key(|span| span.target.start);
    spans
}

/// The bytes of the text of the first heading of `source`, if it has one.
pub fn first_heading(source: &str, format: NoteFormat) -> Option<Range<usize>> {
    let marker = match format {
        NoteFormat::Markdown => '#',
        NoteFormat::AsciiDoc => '=',
    };
    let mut fence: Option<String> = None;
    let mut at = 0;
    for line in source.split_inclusive('\n') {
        let start = at;
        at += line.len();
        let content = line.trim_end_matches(['\n', '\r']);
        if let Some(open) = &fence {
            if content.trim_end() == open {
                fence = None;
            }
            continue;
        }
        if let Some(delimiter) = block_delimiter(content, format) {
            fence = Some(delimiter);
            continue;
        }
        let level = content.chars().take_while(|c| *c == marker).count();
        if !(1..=6).contains(&level) || !content[level..].starts_with([' ', '\t']) {
            continue;
        }
        let mut text = &content[level..];
        if format == NoteFormat::Markdown {
            // A closing sequence of `#` is not part of the heading.
            let trimmed = text.trim_end();
            let without = trimmed.trim_end_matches('#');
            if without.len() < trimmed.len() && without.ends_with([' ', '\t']) {
                text = without;
            }
        }
        let leading = text.len() - text.trim_start().len();
        let begin = start + level + leading;
        return Some(begin..begin + text.trim().len());
    }
    None
}

/// The line that opens a block whose content is not markup: a fenced or
/// delimited code, literal, passthrough, or comment block.
fn block_delimiter(line: &str, format: NoteFormat) -> Option<String> {
    let line = line.trim_end();
    match format {
        NoteFormat::Markdown => {
            let trimmed = line.trim_start();
            let fence = trimmed.chars().next().filter(|c| *c == '`' || *c == '~')?;
            let run = trimmed.chars().take_while(|c| *c == fence).count();
            (run >= 3 && line.len() - trimmed.len() < 4).then(|| trimmed[..run].to_owned())
        }
        NoteFormat::AsciiDoc => {
            let first = line.chars().next()?;
            let delimited = matches!(first, '-' | '.' | '+' | '/')
                && line.len() >= 4
                && line.chars().all(|c| c == first);
            (delimited || line == "```").then(|| line.to_owned())
        }
    }
}

fn markdown_links(source: &str) -> Vec<LinkSpan> {
    let arena = Arena::new();
    let mut options = Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.tasklist = true;
    let root = parse_document(&arena, source, &options);
    let starts: Vec<usize> = std::iter::once(0)
        .chain(source.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    let offset = |line: usize, column: usize| {
        starts.get(line.saturating_sub(1)).copied().unwrap_or(0) + column.saturating_sub(1)
    };
    let mut spans = Vec::new();
    let mut code_lines = Vec::new();
    for node in root.descendants() {
        let data = node.data.borrow();
        match &data.value {
            NodeValue::Link(_) | NodeValue::Image(_) => {
                let start = offset(data.sourcepos.start.line, data.sourcepos.start.column);
                if let Some(span) = inline_link(source, start) {
                    spans.push(span);
                }
            }
            NodeValue::CodeBlock(_) | NodeValue::HtmlBlock(_) => {
                code_lines.push(data.sourcepos.start.line..=data.sourcepos.end.line);
            }
            _ => {}
        }
    }
    // Reference definitions, `[label]: place`, outside code.
    for (index, line) in source.split_inclusive('\n').enumerate() {
        if code_lines.iter().any(|lines| lines.contains(&(index + 1))) {
            continue;
        }
        if let Some(target) = definition(line) {
            let start = starts[index];
            spans.push(LinkSpan {
                target: start + target.start..start + target.end,
                text: None,
            });
        }
    }
    spans
}

/// An inline link or image `[text](place "title")` starting at `start`.
fn inline_link(source: &str, start: usize) -> Option<LinkSpan> {
    let bytes = source.as_bytes();
    let image = bytes.get(start) == Some(&b'!');
    let open = start + usize::from(image);
    if bytes.get(open) != Some(&b'[') {
        return None;
    }
    let mut depth = 0usize;
    let mut i = open;
    let close = loop {
        match bytes.get(i)? {
            b'\\' => i += 1,
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    break i;
                }
            }
            b'`' => {
                let run = bytes[i..].iter().take_while(|b| **b == b'`').count();
                let fence = &source[i..i + run];
                i += run + source[i + run..].find(fence)? + run - 1;
            }
            _ => {}
        }
        i += 1;
    };
    if bytes.get(close + 1) != Some(&b'(') {
        return None;
    }
    let mut i = close + 2;
    while matches!(bytes.get(i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
        i += 1;
    }
    let target = if bytes.get(i) == Some(&b'<') {
        let end = i + 1 + source[i + 1..].find(['>', '\n'])?;
        i + 1..end
    } else {
        let begin = i;
        let mut parens = 0usize;
        while let Some(&b) = bytes.get(i) {
            match b {
                b'\\' => i += 1,
                b'(' => parens += 1,
                b')' if parens == 0 => break,
                b')' => parens -= 1,
                b if b.is_ascii_whitespace() => break,
                _ => {}
            }
            i += 1;
        }
        begin..i.min(source.len())
    };
    (!target.is_empty()).then(|| LinkSpan {
        target,
        text: (!image && close > open + 1).then_some(open + 1..close),
    })
}

/// The place of a reference definition `[label]: place` on `line`.
fn definition(line: &str) -> Option<Range<usize>> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 || !line[indent..].starts_with('[') {
        return None;
    }
    let close = indent + line[indent..].find("]:")?;
    if close == indent + 1 || line[indent + 1..close].contains(['[', ']']) {
        return None;
    }
    let rest = &line[close + 2..];
    let skip = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    let begin = close + 2 + skip;
    let place = &line[begin..];
    if let Some(inner) = place.strip_prefix('<') {
        let end = inner.find('>')?;
        return Some(begin + 1..begin + 1 + end);
    }
    let end = place.find(char::is_whitespace).unwrap_or(place.len());
    (end > 0).then_some(begin..begin + end)
}

fn asciidoc_links(source: &str) -> Vec<LinkSpan> {
    let mut spans = Vec::new();
    let mut block: Option<String> = None;
    let mut at = 0;
    for line in source.split_inclusive('\n') {
        let start = at;
        at += line.len();
        let content = line.trim_end_matches(['\n', '\r']);
        if let Some(open) = &block {
            if content.trim_end() == open {
                block = None;
            }
            continue;
        }
        if let Some(delimiter) = block_delimiter(content, NoteFormat::AsciiDoc) {
            block = Some(delimiter);
            continue;
        }
        if content.starts_with("//") {
            continue;
        }
        asciidoc_line(content, start, &mut spans);
    }
    spans
}

/// The links of one AsciiDoc line: `xref:`, `link:`, `image:`, `image::`,
/// `include::` macros and `<<place,text>>` references to other documents.
fn asciidoc_line(line: &str, start: usize, spans: &mut Vec<LinkSpan>) {
    let bytes = line.as_bytes();
    let mut code = false;
    let mut i = 0;
    while i < line.len() {
        if !line.is_char_boundary(i) {
            i += 1;
            continue;
        }
        if bytes[i] == b'`' {
            code = !code;
            i += 1;
            continue;
        }
        let boundary = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
        if code || !boundary {
            i += 1;
            continue;
        }
        let rest = &line[i..];
        if let Some(inner) = rest.strip_prefix("<<") {
            if let Some(close) = inner.find(">>") {
                let inside = &inner[..close];
                let comma = inside.find(',');
                let place = &inside[..comma.unwrap_or(inside.len())];
                let leading = place.len() - place.trim_start().len();
                let place = place.trim();
                if is_document_reference(place) {
                    let begin = start + i + 2 + leading;
                    let text = comma.map(|c| {
                        let text = &inside[c + 1..];
                        let skip = text.len() - text.trim_start().len();
                        let from = start + i + 2 + c + 1 + skip;
                        from..from + text.trim().len()
                    });
                    spans.push(LinkSpan {
                        target: begin..begin + place.len(),
                        text: text.filter(|t| !t.is_empty()),
                    });
                }
                i += 2 + close + 2;
                continue;
            }
        }
        let macro_len = ["include::", "image::", "image:", "xref:", "link:"]
            .iter()
            .find(|name| rest.starts_with(**name))
            .map(|name| name.len());
        if let Some(len) = macro_len {
            let after = &rest[len..];
            if let Some(open) = after
                .find(['[', ' ', '\t'])
                .filter(|o| after.as_bytes()[*o] == b'[')
            {
                let place = &after[..open];
                let name = &rest[..len];
                let close = after[open..].find(']').map(|c| open + c);
                let wanted = !place.is_empty() && (name != "xref:" || is_document_reference(place));
                if wanted {
                    let begin = start + i + len;
                    let text = close
                        .filter(|_| matches!(name, "xref:" | "link:"))
                        .map(|c| (open + 1, c))
                        .filter(|(from, to)| to > from && !after[*from..*to].contains('='))
                        .map(|(from, to)| begin + from..begin + to);
                    spans.push(LinkSpan {
                        target: begin..begin + place.len(),
                        text,
                    });
                }
                i += len + close.map_or(open, |c| c + 1);
                continue;
            }
        }
        i += 1;
    }
}

/// Whether an AsciiDoc reference names another document, not only an id in
/// the same one: it names a file or has a `#`.
fn is_document_reference(place: &str) -> bool {
    !place.is_empty()
        && !place.contains(char::is_whitespace)
        && (place.contains('#')
            || place
                .rsplit_once('.')
                .is_some_and(|(_, e)| !e.contains('/')))
}

/// UC-006 BR-003 to BR-005: the edits the rename `planned` makes in the text
/// of the note at `note` (its file relative to the workspace, before the
/// rename), and how many links to the renamed note they change.
pub fn rename_edits(
    planned: &NoteMove,
    note: &str,
    format: NoteFormat,
    text: &str,
) -> (Vec<Edit>, usize) {
    let mut edits = Vec::new();
    let mut links = 0;
    if note == planned.from_file() {
        if let Some(heading) = first_heading(text, format) {
            if let Some(title) = planned.retitle(&text[heading.clone()]) {
                edits.push((heading, title));
            }
        }
    }
    for span in link_spans(text, format) {
        let target = &text[span.target.clone()];
        let to_renamed = planned.links_to(note, target);
        if let Some(place) = planned.relink(note, target) {
            edits.push((span.target.clone(), place));
        }
        if to_renamed {
            links += 1;
            if let Some(range) = span.text {
                if let Some(title) = planned.retitle(&text[range.clone()]) {
                    edits.push((range, title));
                }
            }
        }
    }
    edits.sort_by_key(|(range, _)| range.start);
    edits.dedup_by_key(|(range, _)| range.start);
    (edits, links)
}

/// `text` with `edits`, which are sorted and do not overlap.
pub fn apply_edits(text: &str, edits: &[Edit]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for (range, replacement) in edits {
        out.push_str(&text[at..range.start]);
        out.push_str(replacement);
        at = range.end;
    }
    out.push_str(&text[at..]);
    out
}

/// The notes that are open in tabs, as the rename sees them (UC-006 A7).
pub trait OpenNotes {
    /// The text of the note `id` as its tab holds it, when it is open.
    fn text(&self, id: &NoteId) -> Option<String>;
    /// Changes the tab of the note `id`; the tab keeps the user's changes and
    /// cursor and saves them with its next autosave.
    fn edit(&mut self, id: &NoteId, edits: &[Edit]);
}

/// No note is open.
pub struct NoOpenNotes;

impl OpenNotes for NoOpenNotes {
    fn text(&self, _: &NoteId) -> Option<String> {
        None
    }
    fn edit(&mut self, _: &NoteId, _: &[Edit]) {}
}

/// What a rename did (UC-006 step 7, BR-007).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renamed {
    /// The note under its new name.
    pub id: NoteId,
    /// The edits made in the renamed note itself, for its open tab.
    pub own_edits: Vec<Edit>,
    /// How many links to the note were updated, in how many other notes.
    pub links: usize,
    pub notes: usize,
    /// Notes that link to the note but could not be updated (A6).
    pub not_updated: Vec<NoteId>,
}

/// UC-006 steps 5–6: renames and moves the note as `planned` and updates
/// the links to it in every note of the workspace. The note itself is read
/// from its file, so a tab with unsaved changes is saved first; other notes
/// open in tabs are changed there (A7).
pub fn rename_note(
    workspace: &mut Workspace,
    planned: &NoteMove,
    open: &mut dyn OpenNotes,
) -> Result<Renamed, NewNoteProblem> {
    let own = workspace
        .read(&planned.from)
        .map_err(|err| NewNoteProblem::Io {
            action: "read the note",
            source: std::io::Error::other(err.to_string()),
        })?
        .body;
    let (own_edits, own_links) = rename_edits(planned, &planned.from_file(), planned.format, &own);
    workspace.move_note(planned, &apply_edits(&own, &own_edits))?;
    let mut renamed = Renamed {
        id: planned.to.clone(),
        own_edits,
        links: own_links,
        notes: 0,
        not_updated: Vec::new(),
    };
    let page = planned.from.as_str().rsplit('/').next().unwrap_or_default();
    let notes = workspace.listing().notes.clone();
    for entry in notes.iter().filter(|entry| entry.id != planned.to) {
        let file = entry.id.file_name(entry.format);
        let path = workspace.root().join(&file);
        let in_tab = open.text(&entry.id);
        let text = match &in_tab {
            Some(text) => text.clone(),
            None => match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(_) => continue,
            },
        };
        if !text.contains(page) && !text.contains('%') {
            continue;
        }
        let (edits, links) = rename_edits(planned, &file, entry.format, &text);
        if edits.is_empty() {
            continue;
        }
        if in_tab.is_some() {
            open.edit(&entry.id, &edits);
        } else if write_atomic(&path, apply_edits(&text, &edits).as_bytes()).is_err() {
            renamed.not_updated.push(entry.id.clone());
            continue;
        }
        renamed.links += links;
        renamed.notes += 1;
    }
    Ok(renamed)
}

/// UC-007 BR-001: how many links in how many other notes of the workspace
/// lead to the note `id`, as the question before deleting it names them.
pub fn links_to_note(workspace: &Workspace, id: &NoteId) -> (usize, usize) {
    let Some(format) = workspace.format_of(id) else {
        return (0, 0);
    };
    let target = NoteMove {
        from: id.clone(),
        to: id.clone(),
        format,
        title: id.title(),
    };
    let page = id.as_str().rsplit('/').next().unwrap_or_default();
    let (mut links, mut notes) = (0, 0);
    for entry in workspace.listing().notes.iter().filter(|e| &e.id != id) {
        let file = entry.id.file_name(entry.format);
        let Ok(text) = std::fs::read_to_string(workspace.root().join(&file)) else {
            continue;
        };
        if !text.contains(page) && !text.contains('%') {
            continue;
        }
        let found = link_spans(&text, entry.format)
            .iter()
            .filter(|span| target.links_to(&file, &text[span.target.clone()]))
            .count();
        if found > 0 {
            links += found;
            notes += 1;
        }
    }
    (links, notes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use underline_core::Rename;

    fn targets(source: &str, format: NoteFormat) -> Vec<(&str, Option<&str>)> {
        link_spans(source, format)
            .into_iter()
            .map(|s| (&source[s.target], s.text.map(|t| &source[t])))
            .collect()
    }

    #[test]
    fn finds_markdown_links_but_not_in_code() {
        let source = "# T\n\nSee [the *plan*](Plan.md \"t\") and ![chart](img/c.png),\n[x](<my note.md>) and [ref][r] and `[no](No.md)`.\n\n```\n[no](No.md)\n```\n\n| a | [cell](Cell.md) |\n|---|---|\n\n[r]: Ref.md \"title\"\n";
        assert_eq!(
            targets(source, NoteFormat::Markdown),
            [
                ("Plan.md", Some("the *plan*")),
                ("img/c.png", None),
                ("my note.md", Some("x")),
                ("Cell.md", Some("cell")),
                ("Ref.md", None),
            ]
        );
    }

    #[test]
    fn finds_asciidoc_links_but_not_in_code() {
        let source = "= T\n\nSee xref:Setup.adoc[Setup], <<Setup.adoc#install,install>>, <<local-id>>,\nlink:Plan.md[the plan] and image:c.png[] and `link:No.md[]`.\n\nimage::big.png[Big]\ninclude::part.adoc[]\n\n----\nlink:No.md[]\n----\n// link:No.md[]\n";
        assert_eq!(
            targets(source, NoteFormat::AsciiDoc),
            [
                ("Setup.adoc", Some("Setup")),
                ("Setup.adoc#install", Some("install")),
                ("Plan.md", Some("the plan")),
                ("c.png", None),
                ("big.png", None),
                ("part.adoc", None),
            ]
        );
    }

    #[test]
    fn finds_the_first_heading() {
        let md = "```\n# not\n```\n## Plan ##\n# Later\n";
        assert_eq!(
            &md[first_heading(md, NoteFormat::Markdown).unwrap()],
            "Plan"
        );
        let adoc = "= Setup\n";
        assert_eq!(
            &adoc[first_heading(adoc, NoteFormat::AsciiDoc).unwrap()],
            "Setup"
        );
        assert_eq!(first_heading("text\n", NoteFormat::Markdown), None);
    }

    struct Sandbox(PathBuf);

    impl Sandbox {
        fn new(name: &str, files: &[(&str, &str)]) -> Self {
            let root =
                std::env::temp_dir().join(format!("underline-tc006-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            for (path, content) in files {
                let path = root.join(path);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, content).unwrap();
            }
            Self(root)
        }

        fn read(&self, path: &str) -> String {
            fs::read_to_string(self.0.join(path)).unwrap()
        }

        fn rename(&self, from: &str, folder: &str, title: &str) -> Renamed {
            self.rename_with(from, folder, title, &mut NoOpenNotes)
        }

        fn rename_with(
            &self,
            from: &str,
            folder: &str,
            title: &str,
            open: &mut dyn OpenNotes,
        ) -> Renamed {
            let mut workspace = Workspace::open(&self.0).unwrap();
            let planned = workspace
                .plan_rename(
                    &NoteId::from_relative(from),
                    &Rename {
                        folder: folder.into(),
                        title: title.into(),
                    },
                )
                .unwrap()
                .unwrap();
            rename_note(&mut workspace, &planned, open).unwrap()
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// TC-006-01: the links to the note follow it.
    #[test]
    fn uc006_tc01_links_follow_the_note() {
        let sandbox = Sandbox::new(
            "tc01",
            &[
                ("Plan.md", "# Plan\n\n## Goals\n"),
                ("Ideas.md", "[the plan](Plan.md)\n"),
                ("Week.md", "[goals](Plan#goals)\n"),
                ("team/Review.md", "[plan](../Plan.md)\n"),
                ("Other.md", "Nothing about plans.\n"),
            ],
        );
        let renamed = sandbox.rename("Plan", "", "Road Map");
        assert_eq!((renamed.links, renamed.notes), (3, 3));
        assert!(renamed.not_updated.is_empty());
        assert_eq!(sandbox.read("Road-Map.md"), "# Road Map\n\n## Goals\n");
        assert!(!sandbox.0.join("Plan.md").exists());
        assert_eq!(sandbox.read("Ideas.md"), "[the plan](Road-Map.md)\n");
        assert_eq!(sandbox.read("Week.md"), "[goals](Road-Map#goals)\n");
        assert_eq!(sandbox.read("team/Review.md"), "[plan](../Road-Map.md)\n");
        assert_eq!(sandbox.read("Other.md"), "Nothing about plans.\n");
    }

    /// TC-006-02: a moved note keeps its links, the links to it follow.
    #[test]
    fn uc006_tc02_moved_note() {
        let sandbox = Sandbox::new(
            "tc02",
            &[
                (
                    "Plan.md",
                    "# Plan\n[ideas](Ideas.md) ![picture](img/chart.png) [section](#goals) [self](Plan.md#goals) [site](https://example.org)\n",
                ),
                ("Ideas.md", "[plan](Plan.md)\n"),
                ("img/chart.png", ""),
            ],
        );
        sandbox.rename("Plan", "projects/2027", "Plan");
        assert_eq!(
            sandbox.read("projects/2027/Plan.md"),
            "# Plan\n[ideas](../../Ideas.md) ![picture](../../img/chart.png) [section](#goals) [self](Plan.md#goals) [site](https://example.org)\n"
        );
        assert_eq!(sandbox.read("Ideas.md"), "[plan](projects/2027/Plan.md)\n");
    }

    /// TC-006-03 / TC-006-04: link texts and the first heading that show the
    /// old title; everything else stays byte for byte.
    #[test]
    fn uc006_tc03_tc04_texts_and_heading() {
        let index = "[Getting Started](Getting-Started.md)\r\n[start here](Getting-Started.md)\r\nGetting Started\r\n\r\n```\nGetting-Started.md\n```\n[Other](Other.md)  \n";
        let sandbox = Sandbox::new(
            "tc03",
            &[
                ("Getting-Started.md", "# Getting Started\n"),
                ("Notes.md", "# My notes on things\n"),
                ("Index.md", index),
            ],
        );
        let renamed = sandbox.rename("Getting-Started", "", "First Steps");
        assert_eq!((renamed.links, renamed.notes), (2, 1));
        assert_eq!(
            sandbox.read("Index.md"),
            index
                .replacen(
                    "[Getting Started](Getting-Started.md)",
                    "[First Steps](First-Steps.md)",
                    1
                )
                .replacen(
                    "[start here](Getting-Started.md)",
                    "[start here](First-Steps.md)",
                    1
                )
        );
        assert_eq!(sandbox.read("First-Steps.md"), "# First Steps\n");
        sandbox.rename("Notes", "", "Journal");
        assert_eq!(sandbox.read("Journal.md"), "# My notes on things\n");
    }

    /// TC-006-05: AsciiDoc links in AsciiDoc markup, Markdown links to an
    /// AsciiDoc note.
    #[test]
    fn uc006_tc05_asciidoc() {
        let sandbox = Sandbox::new(
            "tc05",
            &[
                ("Setup.adoc", "= Setup\n"),
                (
                    "Guide.adoc",
                    "xref:Setup.adoc[Setup] and <<Setup.adoc#install,install>>\n",
                ),
                ("Intro.md", "[setup](Setup.adoc)\n"),
            ],
        );
        let renamed = sandbox.rename("Setup", "", "Installation");
        assert_eq!((renamed.links, renamed.notes), (3, 2));
        assert_eq!(sandbox.read("Installation.adoc"), "= Installation\n");
        assert_eq!(
            sandbox.read("Guide.adoc"),
            "xref:Installation.adoc[Installation] and <<Installation.adoc#install,install>>\n"
        );
        assert_eq!(sandbox.read("Intro.md"), "[setup](Installation.adoc)\n");
    }

    /// TC-006-12: a linking note that cannot be written is named.
    #[cfg(unix)]
    #[test]
    fn uc006_tc12_linking_note_cannot_be_updated() {
        use std::os::unix::fs::PermissionsExt;
        let sandbox = Sandbox::new(
            "tc12",
            &[
                ("Plan.md", "# Plan\n"),
                ("Ideas.md", "[p](Plan.md)\n"),
                ("locked/Week.md", "[p](../Plan.md)\n"),
            ],
        );
        let locked = sandbox.0.join("locked");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
        let writable = fs::write(locked.join("probe"), "").is_ok();
        let renamed = sandbox.rename("Plan", "", "Road Map");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        if writable {
            return; // running as root: permissions are not enforced
        }
        assert_eq!((renamed.links, renamed.notes), (1, 1));
        assert_eq!(renamed.not_updated, [NoteId::from_relative("locked/Week")]);
        assert_eq!(sandbox.read("locked/Week.md"), "[p](../Plan.md)\n");
        assert_eq!(sandbox.read("Ideas.md"), "[p](Road-Map.md)\n");
    }

    /// TC-006-13: a linking note open in a tab is changed there.
    #[test]
    fn uc006_tc13_open_linking_note() {
        struct Tab(String, Vec<Edit>);
        impl OpenNotes for Tab {
            fn text(&self, id: &NoteId) -> Option<String> {
                (id.as_str() == "Ideas").then(|| self.0.clone())
            }
            fn edit(&mut self, _: &NoteId, edits: &[Edit]) {
                self.1 = edits.to_vec();
            }
        }
        let sandbox = Sandbox::new(
            "tc13",
            &[("Plan.md", "# Plan\n"), ("Ideas.md", "[p](Plan.md)\n")],
        );
        let mut tab = Tab("[p](Plan.md)\ndraft".into(), Vec::new());
        let renamed = sandbox.rename_with("Plan", "", "Road Map", &mut tab);
        assert_eq!(renamed.notes, 1);
        assert_eq!(tab.1, [(4..11, "Road-Map.md".to_owned())]);
        assert_eq!(sandbox.read("Ideas.md"), "[p](Plan.md)\n");
    }

    /// TC-007-03: the question before deleting counts the links to the note.
    #[test]
    fn uc007_tc03_counts_the_links_to_a_note() {
        let sandbox = Sandbox::new(
            "tc703",
            &[
                ("Plan.md", "[self](Plan.md)\n"),
                ("Ideas.md", "[a](Plan.md) [b](Plan#goals) [c](Other.md)\n"),
                ("team/Review.md", "[p](../Plan.md)\n"),
                ("Other.md", "Plan\n"),
            ],
        );
        let workspace = Workspace::open(&sandbox.0).unwrap();
        assert_eq!(
            links_to_note(&workspace, &NoteId::from_relative("Plan")),
            (3, 2)
        );
        assert_eq!(
            links_to_note(&workspace, &NoteId::from_relative("team/Review")),
            (0, 0)
        );
    }

    /// TC-006-17: renaming back gives every file as it was.
    #[test]
    fn uc006_tc17_renaming_back() {
        let files = [
            ("Plan.md", "# Plan\n[i](sub/Ideas.md)\n"),
            ("sub/Ideas.md", "[Plan](../Plan.md)\n"),
        ];
        let sandbox = Sandbox::new("tc17", &files);
        sandbox.rename("Plan", "deep/er", "Road Map");
        sandbox.rename("deep/er/Road-Map", "", "Plan");
        for (path, content) in files {
            assert_eq!(sandbox.read(path), content, "{path}");
        }
    }
}
