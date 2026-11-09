//! HTML of the facade's blocks: one element per block, whose text content is
//! exactly the block's facade text (markers come from CSS), so a DOM offset
//! in UTF-16 code units maps to a facade index without a lookup table.
//!
//! The elements carry no source offsets: those of every later block move on
//! each keystroke, so they stay in the owner's block model (per run), and the
//! view addresses a block by its position among the root's children.

use crate::blocks::{Block, BlockKind, ListMarker, Run};

fn escape(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
}

fn runs(runs: &[Run], out: &mut String) {
    for run in runs {
        let s = &run.style;
        let mut classes = String::new();
        for (on, class) in [
            (s.bold, "b"),
            (s.italic, "i"),
            (s.strike, "s"),
            (s.code, "c"),
            (s.link.is_some(), "l"),
        ] {
            if on {
                classes.push_str(class);
                classes.push(' ');
            }
        }
        if classes.is_empty() {
            escape(&run.text, out);
        } else {
            out.push_str("<span class=\"");
            out.push_str(classes.trim_end());
            out.push_str("\">");
            escape(&run.text, out);
            out.push_str("</span>");
        }
    }
}

/// The HTML of block `index`.
pub fn block_html(index: usize, block: &Block) -> String {
    let mut out =
        String::with_capacity(64 + block.runs.iter().map(|r| r.text.len()).sum::<usize>());
    let start = |out: &mut String, tag: &str, class: &str, extra: &str| {
        out.push_str(&format!(
            "<{tag} data-block=\"{index}\" class=\"blk {class}\"{extra}>"
        ));
    };
    let (tag, close_br) = match &block.kind {
        BlockKind::Heading(level) => {
            let tag = format!("h{}", (*level).clamp(1, 6));
            start(&mut out, &tag, "", "");
            (tag, true)
        }
        BlockKind::Paragraph => {
            start(&mut out, "p", "", "");
            ("p".to_owned(), true)
        }
        BlockKind::ListItem {
            depth,
            marker,
            checked,
        } => {
            let marker = match (marker, checked) {
                (Some(_), Some(true)) => "☑".to_owned(),
                (Some(_), Some(false)) => "☐".to_owned(),
                (Some(ListMarker::Bullet), None) => "•".to_owned(),
                (Some(ListMarker::Number(n)), None) => format!("{n}."),
                (None, _) => String::new(),
            };
            start(
                &mut out,
                "div",
                "li",
                &format!(" style=\"--depth:{depth}\" data-marker=\"{marker}\""),
            );
            ("div".to_owned(), true)
        }
        BlockKind::Quote(depth) => {
            start(
                &mut out,
                "blockquote",
                "",
                &format!(" style=\"--depth:{depth}\""),
            );
            ("blockquote".to_owned(), true)
        }
        BlockKind::Code { language } => {
            start(
                &mut out,
                "pre",
                "code",
                &format!(" data-lang=\"{language}\""),
            );
            ("pre".to_owned(), true)
        }
        BlockKind::ThematicBreak => {
            start(&mut out, "div", "hr", " contenteditable=\"false\"");
            ("div".to_owned(), false)
        }
        BlockKind::Admonition(label) => {
            start(&mut out, "div", "adm", &format!(" data-label=\"{label}\""));
            ("div".to_owned(), true)
        }
        BlockKind::Markup => {
            start(&mut out, "pre", "markup", "");
            ("pre".to_owned(), true)
        }
        BlockKind::Table(_) => {
            start(&mut out, "pre", "markup table", "");
            ("pre".to_owned(), true)
        }
        BlockKind::Image { .. } => {
            start(&mut out, "pre", "markup image", "");
            ("pre".to_owned(), true)
        }
    };
    runs(&block.runs, &mut out);
    // An empty block still needs a line to hold the cursor.
    if close_br && block.runs.iter().all(|r| r.text.is_empty()) {
        out.push_str("<br>");
    }
    out.push_str("</");
    out.push_str(&tag);
    out.push('>');
    out
}

/// The HTML of every block, in order.
pub fn all_html(blocks: &[Block]) -> String {
    let mut out = String::new();
    for (i, block) in blocks.iter().enumerate() {
        out.push_str(&block_html(i, block));
    }
    out
}
