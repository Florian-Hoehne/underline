//! UC-005 A17, ADR-014: what the clipboard holds, as the window's editor
//! core takes it for a paste (feature F6). The system clipboard itself is
//! the app's (`arboard`); this is the part with rules: HTML of other
//! programs becomes markup of the note's format (`html_to_markup`).

use underline_facade::clip::html_to_markup;

use crate::editor_view::ClipContentView;
use crate::workspace_view::FormatName;

/// HTML longer than this is pasted as its plain text: `html_to_markup`
/// parses all of it, and no web page selection worth keeping as markup is
/// this long.
pub const HTML_AT_MOST: usize = 2 * 1024 * 1024;

/// HTML nested deeper than this is pasted as its plain text:
/// `html_to_markup` walks the elements recursively (shared with the egui
/// app, unchanged), and thousands of nested elements would overflow the
/// stack.
pub const HTML_DEPTH_AT_MOST: usize = 64;

/// Elements that have no end tag.
const VOID: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// The deepest nesting of elements in `html`, counted from its tags; it
/// stops counting once it passes `limit`. Comments, doctypes and void or
/// self-closed elements do not nest; an end tag closes one level. Rough on
/// purpose (no parsing): it only has to see that HTML is not too deep.
fn nesting_beyond(html: &str, limit: usize) -> bool {
    let mut depth = 0usize;
    let mut rest = html;
    while let Some(open) = rest.find('<') {
        rest = &rest[open + 1..];
        let end = rest.find('>').unwrap_or(rest.len());
        let tag = &rest[..end];
        rest = &rest[end..];
        if tag.starts_with('/') {
            depth = depth.saturating_sub(1);
            continue;
        }
        if tag.starts_with('!') || tag.starts_with('?') || tag.ends_with('/') {
            continue;
        }
        let name: String = tag
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        if name.is_empty() || VOID.contains(&name.as_str()) {
            continue;
        }
        depth += 1;
        if depth > limit {
            return true;
        }
    }
    false
}

/// Whether `html` is safe to hand to `html_to_markup`: not too long and
/// not too deeply nested (otherwise its plain text is pasted).
fn convertible(html: &str) -> bool {
    html.len() <= HTML_AT_MOST && !nesting_beyond(html, HTML_DEPTH_AT_MOST)
}

/// What the clipboard holds (`text`, `html`, an `image`) for a paste: the
/// HTML as markup of `rich`, the note's format, when the facade asks for
/// it (`SourceView::paste` of egui converts it only then); the text only
/// when it has any. HTML longer than [`HTML_AT_MOST`] or nested deeper than
/// [`HTML_DEPTH_AT_MOST`] is left out, so its plain text is pasted.
#[must_use]
pub fn clipboard_view(
    text: Option<String>,
    html: Option<&str>,
    image: bool,
    rich: Option<FormatName>,
) -> ClipContentView {
    let markup = rich
        .zip(html.filter(|html| convertible(html)))
        .map(|(format, html)| html_to_markup(html, format.into()));
    ClipContentView {
        text: text.filter(|text| !text.is_empty()),
        markup,
        image,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UC-005 A17 (TC-005-26): the facade gets HTML as markup of the note's
    /// format; the source view only the text.
    #[test]
    fn uc005_a17_html_becomes_markup_for_the_facade_only() {
        let html = Some("<p>Use <b>bold</b></p>");
        let rich = clipboard_view(
            Some("Use bold".into()),
            html,
            false,
            Some(FormatName::Markdown),
        );
        assert_eq!(rich.markup.as_deref(), Some("Use **bold**"));
        assert_eq!(rich.text.as_deref(), Some("Use bold"));
        let adoc = clipboard_view(None, html, false, Some(FormatName::AsciiDoc));
        assert_eq!(adoc.markup.as_deref(), Some("Use *bold*"));
        let plain = clipboard_view(Some("Use bold".into()), html, false, None);
        assert_eq!(plain.markup, None);
        let image = clipboard_view(Some(String::new()), None, true, None);
        assert_eq!((image.text, image.image), (None, true));
    }

    /// Review F6: HTML nested 100'000 deep (or longer than 2 MB) is pasted as
    /// its plain text instead of overflowing the stack in `html_to_markup`.
    #[test]
    fn hostile_deep_or_long_html_is_pasted_as_plain_text() {
        let n = 100_000;
        let deep = format!("{}x{}", "<div>".repeat(n), "</div>".repeat(n));
        let view = clipboard_view(
            Some("x".into()),
            Some(&deep),
            false,
            Some(FormatName::Markdown),
        );
        assert_eq!(view.markup, None);
        assert_eq!(view.text.as_deref(), Some("x"));
        let unclosed = "<b>".repeat(n);
        assert_eq!(
            clipboard_view(None, Some(&unclosed), false, Some(FormatName::Markdown)).markup,
            None
        );
        let long = format!("<p>{}</p>", "a".repeat(HTML_AT_MOST));
        assert_eq!(
            clipboard_view(None, Some(&long), false, Some(FormatName::Markdown)).markup,
            None
        );
    }

    /// Ordinary HTML stays convertible: closed tags, void elements, long
    /// lists, comments.
    #[test]
    fn ordinary_html_is_not_too_deep() {
        let list = format!("<ul>{}</ul>", "<li>item <b>b</b><br></li>".repeat(3000));
        assert!(convertible(&list));
        assert!(convertible(
            "<!DOCTYPE html><meta charset=utf-8><!-- c --><p>a<img src=x><hr/></p>"
        ));
        let at_limit = format!(
            "{}x{}",
            "<div>".repeat(HTML_DEPTH_AT_MOST),
            "</div>".repeat(HTML_DEPTH_AT_MOST)
        );
        assert!(convertible(&at_limit));
        let beyond = format!("{}x", "<div>".repeat(HTML_DEPTH_AT_MOST + 1));
        assert!(!convertible(&beyond));
    }
}
