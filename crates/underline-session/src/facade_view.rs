//! What the window's facade shows of an open note (UC-004 step 3, M-09,
//! ADR-021, ADR-024): the blocks of the facade engine
//! (`underline_facade::blocks`) as the window draws them, with the place in
//! the markup each shown character stands for.
//!
//! Offsets are web units, as in the source view (`underline_editor_core::utf16`):
//! UTF-16 code units of the markup with its line breaks normalised, so a
//! `\r\n` pair is one unit. The start and the line of a block are absolute;
//! everything else in a block is relative to its start, so a block after an
//! edit keeps its view and only moves (`BlocksView::shift`).

use serde::Serialize;

use crate::editor_view::SpanStyle;

/// A run of text in one style (M-09.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RunView {
    /// The text shown, with its line breaks normalised.
    pub text: String,
    /// Bold, in the colour "strong".
    pub bold: bool,
    /// Italic.
    pub italic: bool,
    /// Struck through.
    pub strike: bool,
    /// Inline code, on the code background.
    pub code: bool,
    /// The address of the link the text belongs to (M-07.4).
    pub link: Option<String>,
    /// Where the characters stand in the markup: pairs of a UTF-16 offset in
    /// `text` and the web offset, relative to the start of the block, of the
    /// markup character it shows. The characters between two pairs follow one
    /// another in the markup; the first pair is at 0 unless `text` is empty.
    pub offsets: Vec<(u32, u32)>,
    /// The web offset right after the run, relative to the start of the
    /// block.
    pub end: u32,
}

/// How the text of a table column is aligned (M-09.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum CellAlignName {
    /// Left.
    Left,
    /// Centred.
    Center,
    /// Right.
    Right,
}

/// A table (M-09.5): its rows of cells, each cell its styled runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TableView {
    /// The first row is the header: bold and shaded.
    pub header: bool,
    /// The alignment of each column.
    pub align: Vec<CellAlignName>,
    /// The rows, each a list of cells, each a list of runs.
    pub rows: Vec<Vec<Vec<RunView>>>,
    /// "Table n. {title}" above the table, for an AsciiDoc table with a
    /// title.
    pub caption: Option<String>,
}

/// What a block is (M-09.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BlockKindView {
    /// A heading of level 1 to 6.
    Heading {
        /// The level.
        level: u8,
    },
    /// A paragraph.
    Paragraph,
    /// A paragraph of a list item; its marker is the block's prefix.
    ListItem {
        /// The nesting depth, 0 for the outer list.
        depth: u32,
    },
    /// A paragraph in quotes: one bar per depth.
    Quote {
        /// How deep the quotes are, at least 1.
        depth: u32,
    },
    /// A code block; its one run is the code.
    Code {
        /// The language the block names, such as `rust` or `mermaid`.
        language: String,
    },
    /// A thematic break, drawn as a line.
    ThematicBreak,
    /// A paragraph set apart with a bar in the link colour; its label is in
    /// the block's prefix.
    Admonition {
        /// `NOTE`, `TIP`, `IMPORTANT`, `WARNING` or `CAUTION`.
        label: String,
    },
    /// A block shown as its markup.
    Markup,
    /// A table; its runs are its markup, shown below it while the cursor is
    /// in it (M-09.8).
    Table {
        /// The formatted table.
        table: TableView,
    },
    /// An image on its own; its runs are its markup.
    Image {
        /// Where the image is, as the markup says.
        target: String,
        /// Its alternative text.
        alt: String,
    },
}

/// The colours of a code block in its language (M-09.4, ADR-010).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CodeColors {
    /// `[start, end, style]`: UTF-16 offsets in the block's text and an
    /// index into `styles`. Text in the theme's plain colour has no span.
    pub spans: Vec<(u32, u32, u32)>,
    /// The looks the spans refer to.
    pub styles: Vec<SpanStyle>,
}

/// The picture of an image or a diagram block (M-09.6, M-09.7).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FigureView {
    /// The picture, as a path of the app's own protocol `ul-asset`
    /// (ADR-023), such as `diagram/0f…a1.svg`. While a changed diagram
    /// renders it is the last picture.
    pub picture: Option<String>,
    /// What stands instead of the picture: "Rendering the Mermaid
    /// diagram…", or why it cannot be shown.
    pub message: Option<String>,
    /// The message says why the picture cannot be shown.
    pub failed: bool,
    /// The markup is shown below the figure wherever the cursor is: a
    /// diagram that cannot be rendered shows its source (UC-004 A11).
    pub source_shown: bool,
    /// "Loading {target}…", shown while the window loads an image file.
    pub loading: Option<String>,
    /// What the window says when it cannot read the image file.
    pub unreadable: Option<String>,
    /// "Figure n. {caption}" below the picture.
    pub caption: Option<String>,
}

/// One block of the facade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BlockView {
    /// What the block is.
    pub kind: BlockKindView,
    /// Where the block's markup starts, in web units.
    pub start: u32,
    /// The line it starts on, counted from 0, for folding (M-08.8).
    pub line: u32,
    /// The length of the block's markup with its line endings, in web units.
    pub length: u32,
    /// How many lines of markup the block has, a hint for its height before
    /// it is drawn.
    pub lines: u32,
    /// What stands before the text in the weak colour, as egui lays it out:
    /// the indentation and marker of a list item ("•  ", "3.  ", "☐ "), the
    /// indentation of a quote, the label of an admonition.
    pub prefix: String,
    /// The text, run by run.
    pub runs: Vec<RunView>,
    /// The colours of a code block, once the theme's colours are ready.
    pub colors: Option<CodeColors>,
    /// The picture of an image or diagram block.
    pub figure: Option<FigureView>,
}

/// M-09, ADR-024: the facade of an open note as the window starts showing
/// it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FacadeState {
    /// The note.
    pub note: String,
    /// The revision of the text the blocks are of.
    pub rev: u32,
    /// Counts the changes of the blocks; each `ed-blocks` event names the
    /// version it applies to.
    pub version: u32,
    /// M-09.3: "Markdown" or "AsciiDoc", shown at the top right.
    pub format: String,
    /// The blocks.
    pub blocks: Vec<BlockView>,
    /// The other end of the selection, in web units.
    pub anchor: u32,
    /// The cursor, in web units.
    pub head: u32,
    /// UC-005: the place of the cursor for the status line.
    pub place: String,
}

/// ADR-024: the blocks changed after an input, a change from outside, a
/// picture that became ready or another theme: `removed` blocks from `from`
/// on make way for `blocks`, and every block after them moves by `shift`
/// web units and `line_shift` lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BlocksView {
    /// The note.
    pub note: String,
    /// The revision of the text the blocks are of now.
    pub rev: u32,
    /// The version of the blocks this change applies to; a window that has
    /// another asks for the whole facade again (`fa_open`).
    pub base: u32,
    /// The version after the change.
    pub version: u32,
    /// The first block replaced.
    pub from: u32,
    /// How many blocks are replaced.
    pub removed: u32,
    /// The new blocks.
    pub blocks: Vec<BlockView>,
    /// How far the blocks after them move, in web units.
    pub shift: i32,
    /// How many lines the blocks after them move.
    pub line_shift: i32,
}
