//! The outline, code index, and figure index of a note for the tool panel
//! (UC-004 step 4, FR-041 to FR-043): built from the blocks of the facade,
//! each entry with the source offset to jump to.

use crate::blocks::{Block, BlockKind};
use crate::diagram::DiagramLanguage;

/// One entry of a list in the tool panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// What the entry shows.
    pub label: String,
    /// The nesting of a heading, 0 for the highest heading level of the
    /// note; 0 for other entries.
    pub depth: usize,
    /// Where the entry is in the source, to jump to.
    pub offset: usize,
}

/// The three lists of the tool panel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Indexes {
    pub outline: Vec<Entry>,
    pub code: Vec<Entry>,
    pub figures: Vec<Entry>,
}

/// Longest label, in characters.
const LABEL_LENGTH: usize = 60;

/// FR-041 to FR-043: the outline, code index, and figure index of `blocks`.
pub fn indexes(blocks: &[Block]) -> Indexes {
    let top = blocks
        .iter()
        .filter_map(|b| match b.kind {
            BlockKind::Heading(level) => Some(level),
            _ => None,
        })
        .min()
        .unwrap_or(1);
    let mut indexes = Indexes::default();
    for block in blocks {
        let offset = block.span.start;
        match &block.kind {
            BlockKind::Heading(level) => indexes.outline.push(Entry {
                label: shorten(&block.text()),
                depth: usize::from(level - top),
                offset,
            }),
            BlockKind::Code { language } => {
                let text = block.text();
                match DiagramLanguage::from_code_language(language) {
                    Some(diagram) => indexes.figures.push(Entry {
                        label: format!("{} diagram: {}", diagram.name(), first_line(&text, true)),
                        depth: 0,
                        offset,
                    }),
                    None => {
                        let first = first_line(&text, false);
                        let label = if language.is_empty() {
                            first
                        } else {
                            format!("{language}: {first}")
                        };
                        indexes.code.push(Entry {
                            label,
                            depth: 0,
                            offset,
                        });
                    }
                }
            }
            BlockKind::Image {
                target,
                alt,
                caption,
            } => {
                let label = caption
                    .as_deref()
                    .or((!alt.is_empty()).then_some(alt.as_str()))
                    .unwrap_or(target);
                indexes.figures.push(Entry {
                    label: shorten(label),
                    depth: 0,
                    offset,
                });
            }
            _ => {}
        }
    }
    indexes
}

/// The first line that says something, shortened; for a diagram the lines
/// that only start or end it (`@startuml`) are skipped.
fn first_line(text: &str, diagram: bool) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| {
            !line.is_empty() && !(diagram && (line.starts_with('@') || line.starts_with("%%")))
        })
        .unwrap_or("(empty)");
    shorten(line)
}

fn shorten(text: &str) -> String {
    let text = text.trim();
    if text.chars().count() <= LABEL_LENGTH {
        return text.to_owned();
    }
    let mut short: String = text.chars().take(LABEL_LENGTH - 1).collect();
    short.push('…');
    short
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::parse_blocks;
    use underline_core::NoteFormat;

    /// TC-004-08: the outline lists the headings with their nesting, the
    /// code index the code blocks, and the figure index the images and
    /// diagrams, each with where it is.
    #[test]
    fn uc004_tc08_outline_code_and_figure_index() {
        let note = "## Intro\n\n```rust\nfn main() {}\n```\n\n## Setup\n\n### Linux\n\n![Shot](shot.png)\n\n```\nmake\n```\n\n```plantuml\n@startuml\nAlice -> Bob\n@enduml\n```\n\n## Usage\n\n```sh\nunderline .\n```\n\n![](img/plan.png)\n";
        let blocks = parse_blocks(note, NoteFormat::Markdown);
        let indexes = indexes(&blocks);
        let outline: Vec<_> = indexes
            .outline
            .iter()
            .map(|e| (e.label.as_str(), e.depth))
            .collect();
        assert_eq!(
            outline,
            [("Intro", 0), ("Setup", 0), ("Linux", 1), ("Usage", 0)]
        );
        let code: Vec<_> = indexes.code.iter().map(|e| e.label.as_str()).collect();
        assert_eq!(code, ["rust: fn main() {}", "make", "sh: underline ."]);
        let figures: Vec<_> = indexes.figures.iter().map(|e| e.label.as_str()).collect();
        assert_eq!(
            figures,
            ["Shot", "PlantUML diagram: Alice -> Bob", "img/plan.png"]
        );
        // Each entry jumps to its block.
        let usage = &indexes.outline[3];
        assert!(note[usage.offset..].starts_with("## Usage"));
        assert!(note[indexes.code[2].offset..].starts_with("```sh"));
    }

    #[test]
    fn asciidoc_captions_name_figures() {
        let note = "= Guide\n\n== Parts\n\n.The parts\nimage::parts.png[Parts]\n\n[mermaid]\n----\ngraph TD\nA-->B\n----\n";
        let indexes = indexes(&parse_blocks(note, NoteFormat::AsciiDoc));
        let outline: Vec<_> = indexes
            .outline
            .iter()
            .map(|e| (e.label.as_str(), e.depth))
            .collect();
        assert_eq!(outline, [("Guide", 0), ("Parts", 1)]);
        let figures: Vec<_> = indexes.figures.iter().map(|e| e.label.as_str()).collect();
        assert_eq!(figures, ["The parts", "Mermaid diagram: graph TD"]);
    }

    #[test]
    fn long_labels_are_shortened() {
        let long = "x".repeat(100);
        assert_eq!(shorten(&long).chars().count(), LABEL_LENGTH);
        assert!(shorten(&long).ends_with('…'));
    }
}
