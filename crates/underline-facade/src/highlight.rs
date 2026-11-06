//! Highlighting of markup and code (ADR-010): per-line parse state, so an
//! edit recolors only from the changed line on.

use std::sync::Arc;

use syntect::highlighting::{
    Color, FontStyle, HighlightIterator, HighlightState, Highlighter as SyntectHighlighter, Theme,
    ThemeSet,
};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};

use underline_core::NoteFormat;

/// One colored piece of a line: a byte range of the line and its color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    /// Red, green, blue, alpha.
    pub color: [u8; 4],
    pub bold: bool,
    pub italic: bool,
}

/// The grammars and the theme colors, shared by all documents.
pub struct HighlightSet {
    syntaxes: SyntaxSet,
    theme: Theme,
}

impl HighlightSet {
    /// The embedded grammars with a light or dark theme.
    ///
    /// The palettes of the eleven Underline themes replace these themes when
    /// themes are built (ADR-010).
    pub fn new(dark: bool) -> Self {
        let themes = ThemeSet::load_defaults();
        let name = if dark {
            "base16-ocean.dark"
        } else {
            "InspiredGitHub"
        };
        Self {
            syntaxes: SyntaxSet::load_defaults_newlines(),
            theme: themes.themes[name].clone(),
        }
    }

    fn syntax_for(&self, format: NoteFormat) -> &SyntaxReference {
        let found = match format {
            NoteFormat::Markdown => self.syntaxes.find_syntax_by_extension("md"),
            // The AsciiDoc grammar is vendored later (ADR-010).
            NoteFormat::AsciiDoc => self.syntaxes.find_syntax_by_extension("adoc"),
        };
        found.unwrap_or_else(|| self.syntaxes.find_syntax_plain_text())
    }
}

/// The highlighting state of one document: for every line the parse state at
/// its start and, once computed, its spans.
pub struct DocumentHighlight {
    set: Arc<HighlightSet>,
    format: NoteFormat,
    /// `starts[i]` is the state at the start of line `i`, valid for `i < valid`.
    starts: Vec<LineState>,
    spans: Vec<Option<Arc<Vec<Span>>>>,
    valid: usize,
    /// First line changed by an edit whose effect on later lines is unknown.
    dirty: Option<usize>,
}

impl DocumentHighlight {
    pub fn new(set: Arc<HighlightSet>, format: NoteFormat) -> Self {
        let syntax = set.syntax_for(format);
        let highlighter = SyntectHighlighter::new(&set.theme);
        let first = LineState::Markup(
            ParseState::new(syntax),
            HighlightState::new(&highlighter, ScopeStack::new()),
        );
        Self {
            set,
            format,
            starts: vec![first],
            spans: Vec::new(),
            valid: 1,
            dirty: None,
        }
    }

    pub fn format(&self) -> NoteFormat {
        self.format
    }

    /// Takes note of an edit that changed lines from `line` on and added
    /// `added` lines (negative when lines were removed); `lines` is the
    /// number of lines the document has now.
    ///
    /// The colors and states of the lines after the edit are kept: when the
    /// state at the end of a recolored line equals the one remembered, the
    /// lines after it keep their colors (ADR-010).
    pub fn edited(&mut self, line: usize, added: isize, lines: usize) {
        let shift = |v: usize| -> usize {
            if v > line {
                (v as isize + added).max(line as isize + 1) as usize
            } else {
                v
            }
        };
        if added > 0 {
            let at = (line + 1).min(self.spans.len());
            self.spans
                .splice(at..at, std::iter::repeat(None).take(added as usize));
            let at = (line + 1).min(self.starts.len());
            let filler = self.starts.get(at.saturating_sub(1)).cloned();
            if let Some(filler) = filler {
                self.starts
                    .splice(at..at, std::iter::repeat(filler).take(added as usize));
            }
        } else if added < 0 {
            let removed = (-added) as usize;
            let from = (line + 1).min(self.spans.len());
            let to = (from + removed).min(self.spans.len());
            self.spans.drain(from..to);
            let from = (line + 1).min(self.starts.len());
            let to = (from + removed).min(self.starts.len());
            self.starts.drain(from..to);
        }
        self.valid = shift(self.valid).min(self.starts.len());
        self.dirty = Some(self.dirty.map_or(line, |d| d.min(line)));
        // The changed lines themselves need new colors.
        let last_changed = (line as isize + added.max(0)) as usize;
        for spans in self.spans.iter_mut().take(last_changed + 1).skip(line) {
            *spans = None;
        }
        self.spans.resize(lines, None);
        self.starts.truncate(lines.max(1));
        self.valid = self.valid.min(self.starts.len());
    }

    /// Forgets all colors from `line` on; `lines` is the number of lines the
    /// document has now.
    pub fn invalidate_from(&mut self, line: usize, lines: usize) {
        self.valid = self.valid.min(line + 1).max(1);
        self.starts.truncate(self.valid);
        self.spans.truncate(line.min(self.spans.len()));
        self.spans.resize(lines, None);
        self.dirty = None;
    }

    /// The spans of `line`, computing the lines before it as needed; `text`
    /// gives the text of a line, with its line ending.
    pub fn spans(&mut self, line: usize, text: impl Fn(usize) -> String) -> Arc<Vec<Span>> {
        if self.spans.len() <= line {
            self.spans.resize(line + 1, None);
        }
        // Recolor from an edit until the state matches the remembered one.
        if let Some(dirty) = self.dirty {
            if dirty <= line {
                self.recolor_from(dirty, line, &text);
            }
        }
        if let Some(spans) = &self.spans[line] {
            return Arc::clone(spans);
        }
        let from = self.valid.min(line + 1) - 1;
        self.compute(from, line, &text, false);
        Arc::clone(self.spans[line].as_ref().expect("just computed"))
    }

    /// Recolors from the edited line `from`; stops early when the state at
    /// the end of a line equals the remembered start state of the next line,
    /// otherwise forgets everything after `upto`.
    fn recolor_from(&mut self, from: usize, upto: usize, text: &impl Fn(usize) -> String) {
        self.dirty = None;
        if from >= self.starts.len() {
            return;
        }
        if self.compute(from, upto, text, true).is_none() {
            // Not settled by `upto`: the states after it are unknown now.
            let keep = (upto + 1).min(self.spans.len());
            self.invalidate_from(keep, self.spans.len());
        }
    }

    /// Computes lines `from..=to` from the state at the start of `from`. With
    /// `until_settled`, goes on past `to` while the lines after it still have
    /// remembered states, and stops at the first line whose new end state
    /// equals the remembered one; returns that line.
    fn compute(
        &mut self,
        from: usize,
        to: usize,
        text: &impl Fn(usize) -> String,
        until_settled: bool,
    ) -> Option<usize> {
        let highlighter = SyntectHighlighter::new(&self.set.theme);
        let lines = self.spans.len();
        let mut current = from;
        while current < lines && (current <= to || until_settled) {
            let line_text = text(current);
            let (spans, state) =
                self.highlight_line(&self.starts[current], &line_text, &highlighter);
            self.spans[current] = Some(Arc::new(spans));
            let next = current + 1;
            if next < self.starts.len() {
                let settled = next < self.valid && self.starts[next] == state;
                self.starts[next] = state;
                if until_settled && settled && self.spans.get(next).is_some_and(Option::is_some) {
                    return Some(current);
                }
            } else {
                self.starts.push(state);
            }
            let remembered = self.valid;
            self.valid = self.valid.max(next + 1).min(self.starts.len());
            current += 1;
            if current > to && (!until_settled || next >= remembered) {
                break;
            }
        }
        None
    }

    /// Colors one line from the state at its start; returns its spans and
    /// the state at its end. Code inside a fence is colored in the language
    /// the fence names (ADR-010).
    fn highlight_line(
        &self,
        state: &LineState,
        line: &str,
        highlighter: &SyntectHighlighter,
    ) -> (Vec<Span>, LineState) {
        let syntaxes = &self.set.syntaxes;
        match state {
            LineState::Markup(parse, highlight) => {
                let (spans, parse, highlight) =
                    color_line(parse, highlight, line, syntaxes, highlighter);
                let next = match open_fence(line) {
                    Some((fence, language)) => {
                        let syntax = syntaxes
                            .find_syntax_by_token(&language)
                            .unwrap_or_else(|| syntaxes.find_syntax_plain_text());
                        LineState::Code {
                            markup: Box::new((parse, highlight)),
                            fence,
                            parse: ParseState::new(syntax),
                            highlight: HighlightState::new(highlighter, ScopeStack::new()),
                        }
                    }
                    None => LineState::Markup(parse, highlight),
                };
                (spans, next)
            }
            LineState::Code {
                markup,
                fence,
                parse,
                highlight,
            } => {
                if closes_fence(line, fence) {
                    let (spans, parse, highlight) =
                        color_line(&markup.0, &markup.1, line, syntaxes, highlighter);
                    (spans, LineState::Markup(parse, highlight))
                } else {
                    let (spans, parse, highlight) =
                        color_line(parse, highlight, line, syntaxes, highlighter);
                    (
                        spans,
                        LineState::Code {
                            markup: markup.clone(),
                            fence: fence.clone(),
                            parse,
                            highlight,
                        },
                    )
                }
            }
        }
    }

    /// The foreground and background colors of the theme.
    pub fn colors(&self) -> ([u8; 4], [u8; 4]) {
        let settings = &self.set.theme.settings;
        (
            rgba(settings.foreground.unwrap_or(Color::BLACK)),
            rgba(settings.background.unwrap_or(Color::WHITE)),
        )
    }
}

/// The state at the start of a line: in the markup, or inside a code fence
/// with the markup state to go back to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum LineState {
    Markup(ParseState, HighlightState),
    Code {
        markup: Box<(ParseState, HighlightState)>,
        fence: String,
        parse: ParseState,
        highlight: HighlightState,
    },
}

/// Colors `line` with a grammar state; returns the spans and the end state.
fn color_line(
    parse: &ParseState,
    highlight: &HighlightState,
    line: &str,
    syntaxes: &SyntaxSet,
    highlighter: &SyntectHighlighter,
) -> (Vec<Span>, ParseState, HighlightState) {
    let (mut parse, mut highlight) = (parse.clone(), highlight.clone());
    let ops = parse.parse_line(line, syntaxes).unwrap_or_default();
    let visible = line.trim_end_matches(['\n', '\r']).len();
    let mut spans = Vec::new();
    let mut offset = 0;
    for (style, piece) in HighlightIterator::new(&mut highlight, &ops, line, highlighter) {
        let end = offset + piece.len();
        if end.min(visible) > offset {
            spans.push(Span {
                start: offset,
                end: end.min(visible),
                color: rgba(style.foreground),
                bold: style.font_style.contains(FontStyle::BOLD),
                italic: style.font_style.contains(FontStyle::ITALIC),
            });
        }
        offset = end;
    }
    (spans, parse, highlight)
}

/// A line that opens a code fence (three or more backticks or tildes, at most
/// three spaces in): the fence and the language it names.
fn open_fence(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim_end_matches(['\n', '\r']);
    let indent = trimmed.len() - trimmed.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &trimmed[indent..];
    let marker = rest.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let fence: String = rest.chars().take_while(|c| *c == marker).collect();
    if fence.len() < 3 {
        return None;
    }
    let info = rest[fence.len()..].trim();
    if marker == '`' && info.contains('`') {
        return None;
    }
    let language = info
        .split(|c: char| c.is_whitespace() || c == '{' || c == ',')
        .next()
        .unwrap_or("")
        .to_lowercase();
    Some((fence, language))
}

/// Whether `line` closes a fence opened with `fence`.
fn closes_fence(line: &str, fence: &str) -> bool {
    let trimmed = line.trim();
    let marker = fence.chars().next().unwrap_or('`');
    trimmed.len() >= fence.len() && trimmed.chars().all(|c| c == marker)
}

fn rgba(color: Color) -> [u8; 4] {
    [color.r, color.g, color.b, color.a]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc<'a>(lines: &'a [&'a str]) -> impl Fn(usize) -> String + 'a {
        move |i| format!("{}\n", lines.get(i).copied().unwrap_or(""))
    }

    /// ADR-010: splitting a line keeps the colors after it.
    #[test]
    fn a_new_line_keeps_the_colors_after_it() {
        let set = Arc::new(HighlightSet::new(true));
        let mut lines: Vec<String> = (0..1000)
            .map(|i| {
                if i % 10 == 0 {
                    "```rust".to_owned()
                } else if i % 10 == 9 {
                    "```".to_owned()
                } else {
                    format!("let x{i} = {i};")
                }
            })
            .collect();
        let mut highlight = DocumentHighlight::new(set, NoteFormat::Markdown);
        highlight.invalidate_from(0, lines.len());
        let text = |l: &Vec<String>| {
            let l = l.clone();
            move |i: usize| format!("{}\n", l[i])
        };
        highlight.spans(999, text(&lines));
        let tail = lines[504].split_off(4);
        lines.insert(505, tail);
        highlight.edited(504, 1, lines.len());
        highlight.spans(505, text(&lines));
        assert!(
            highlight.spans[900].is_some(),
            "colors after the edit were kept"
        );
        assert!(
            highlight.valid > 900,
            "states after the edit were kept: valid {}",
            highlight.valid
        );
    }

    #[test]
    fn colors_markdown_and_code_in_fences() {
        let set = Arc::new(HighlightSet::new(false));
        let lines = ["# Title", "```rust", "fn main() {}", "```", "plain"];
        let mut highlight = DocumentHighlight::new(set, NoteFormat::Markdown);
        highlight.invalidate_from(0, lines.len());
        let heading = highlight.spans(0, doc(&lines));
        let code = highlight.spans(2, doc(&lines));
        assert!(!heading.is_empty());
        let distinct: std::collections::HashSet<_> = code.iter().map(|s| s.color).collect();
        assert!(distinct.len() > 1, "code in the fence is colored: {code:?}");
        assert_eq!(code.last().unwrap().end, "fn main() {}".len());
    }

    #[test]
    fn an_edit_recolors_only_from_its_line() {
        let set = Arc::new(HighlightSet::new(true));
        let mut lines: Vec<String> = (0..100)
            .map(|i| format!("def f{i}(): return {i}"))
            .collect();
        let mut highlight = DocumentHighlight::new(set, NoteFormat::Markdown);
        highlight.invalidate_from(0, lines.len());
        let text = |l: &Vec<String>| {
            let l = l.clone();
            move |i: usize| format!("{}\n", l[i])
        };
        highlight.spans(99, text(&lines));
        // A change inside a line that does not change the state keeps the
        // colors of the lines after it.
        lines[50] = "def fifty(): return 50".to_owned();
        highlight.edited(50, 0, lines.len());
        assert!(highlight.spans[51].is_some());
        highlight.spans(50, text(&lines));
        assert!(highlight.spans[51].is_some());
        // Opening a code fence changes the state: later lines get new colors.
        let before = highlight.spans(60, text(&lines));
        lines[50] = "```python".to_owned();
        highlight.edited(50, 0, lines.len());
        let after = highlight.spans(60, text(&lines));
        assert_ne!(before, after);
        // Inserted lines shift the remembered colors.
        lines.insert(10, "inserted".to_owned());
        highlight.edited(9, 1, lines.len());
        assert_eq!(highlight.spans(61, text(&lines)), after);
    }
}
