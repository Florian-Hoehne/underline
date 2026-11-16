//! FR-098, UC-004 BR-019, A21, ADR-029: formulas typeset as SVG by RaTeX,
//! a port of KaTeX to Rust. One engine typesets the facade, the HTML export
//! and the PDF export, so a formula looks the same in all three; its glyphs
//! are paths, so the SVG needs no fonts and looks alike on every platform
//! (NFR-013), and nothing is loaded from the network (NFR-014).
//!
//! The renderer sits behind [`MathRenderer`], as the diagrams sit behind
//! their renderer (ADR-009), so another engine can take its place.

use std::fmt::Write as _;

use ratex_layout::{layout, to_display_list, validate_layout, LayoutOptions};
use ratex_svg::{render_to_svg, SvgOptions};
use ratex_types::color::Color;
use ratex_types::math_style::MathStyle;

use crate::themes::Rgb;

/// The SVG's user units per em; the facade and the exports size it in em.
const UNITS_PER_EM: f64 = 40.0;
/// Space around the formula, in em, so no glyph that reaches beyond its box
/// is cut off.
const PADDING_EM: f64 = 0.05;
/// The longest source typeset; a longer one is [`MathError::TooLarge`].
pub const MAX_SOURCE_BYTES: usize = 16 * 1024;
/// The largest SVG of one formula, so the cache of the facade keeps many.
pub const MAX_SVG_BYTES: usize = 2 * 1024 * 1024;
/// The widest and highest formula, in em.
const MAX_EM: f64 = 2000.0;

/// Why a formula cannot be typeset (UC-004 A21).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MathError {
    /// The notation has an error.
    #[error("{message}{}", .at.map(|at| format!(" (at character {at})")).unwrap_or_default())]
    Syntax {
        /// The parser's reason.
        message: String,
        /// The character the error was found at, counted from 1 in the
        /// formula's source.
        at: Option<usize>,
    },
    /// The formula is too large to typeset.
    #[error("the formula is too large to typeset: {0}")]
    TooLarge(String),
    /// The engine failed.
    #[error("the formula could not be typeset: {0}")]
    Engine(String),
}

/// A typeset formula: its SVG and its size in em, the padding included.
#[derive(Debug, Clone, PartialEq)]
pub struct Typeset {
    /// A standalone SVG document, its glyphs as paths.
    pub svg: String,
    /// The width of the picture, in em.
    pub width: f64,
    /// The height of the picture above and below the baseline, in em.
    pub height: f64,
    /// How far the picture reaches below the baseline, in em; an inline
    /// formula sits at `vertical-align: -depth`.
    pub depth: f64,
}

impl Typeset {
    /// FR-098, UC-009: the SVG to stand inline in an HTML page, sized in
    /// em so it follows the text around it, with `role="img"` and
    /// `label` (the formula's source) for screen readers.
    #[must_use]
    pub fn inline_svg(&self, label: &str) -> String {
        let Some(rest) = self.svg.strip_prefix("<svg ") else {
            return self.svg.clone();
        };
        // The root's own size (in pt) gives way to one in em.
        let rest = strip_attribute(&strip_attribute(rest, "width"), "height");
        let mut out = String::with_capacity(self.svg.len() + 160);
        let _ = write!(
            out,
            "<svg role=\"img\" aria-label=\"{}\" width=\"{:.3}em\" height=\"{:.3}em\" \
             style=\"vertical-align:-{:.3}em\" ",
            escape_attribute(label),
            self.width,
            self.height,
            self.depth
        );
        out.push_str(&rest);
        out
    }
}

/// `tag_rest` (the attributes of a start tag and what follows) without its
/// first attribute `name="…"`.
fn strip_attribute(tag_rest: &str, name: &str) -> String {
    let end = tag_rest.find('>').unwrap_or(tag_rest.len());
    let pattern = format!(" {name}=\"");
    let found = tag_rest[..end]
        .find(&pattern)
        .map(|at| (at, at + pattern.len()))
        .or_else(|| {
            tag_rest[..end]
                .strip_prefix(&pattern[1..])
                .map(|_| (0, pattern.len() - 1))
        });
    let Some((start, value)) = found else {
        return tag_rest.to_owned();
    };
    let Some(close) = tag_rest[value..end].find('"') else {
        return tag_rest.to_owned();
    };
    format!("{}{}", &tag_rest[..start], &tag_rest[value + close + 1..])
}

/// `text` for an attribute value in double quotes.
fn escape_attribute(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// Turns the source of a formula into SVG (ADR-029).
pub trait MathRenderer {
    /// Typesets `source` as a display formula (`display`) or in the line,
    /// in the colour `color`.
    ///
    /// # Errors
    ///
    /// [`MathError`] when the source has an error or cannot be typeset.
    fn render(&mut self, source: &str, display: bool, color: Rgb) -> Result<Typeset, MathError>;
}

/// The formulas of Underline, typeset by RaTeX (ADR-029).
#[derive(Debug, Clone, Copy, Default)]
pub struct Ratex;

impl MathRenderer for Ratex {
    fn render(&mut self, source: &str, display: bool, color: Rgb) -> Result<Typeset, MathError> {
        typeset(source, display, color)
    }
}

/// FR-098: whether `source` is a formula RaTeX reads, without typesetting
/// it (the exports to the other note format decide its notation by it).
#[must_use]
pub fn parses(source: &str) -> bool {
    source.len() <= MAX_SOURCE_BYTES && ratex_parser::parser::parse(source).is_ok()
}

/// FR-098: `source` typeset by RaTeX, as a display formula or in the line,
/// in `color`.
///
/// # Errors
///
/// [`MathError::Syntax`] with the parser's reason and place (UC-004 A21),
/// [`MathError::TooLarge`] beyond the engine's limits.
pub fn typeset(source: &str, display: bool, color: Rgb) -> Result<Typeset, MathError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(MathError::TooLarge(format!(
            "its source is longer than {} KiB",
            MAX_SOURCE_BYTES / 1024
        )));
    }
    let nodes = ratex_parser::parser::parse(source).map_err(|error| MathError::Syntax {
        at: error.loc.as_ref().map(|loc| {
            source
                .get(..loc.start)
                .map_or(loc.start, |s| s.chars().count())
                + 1
        }),
        message: error.message,
    })?;
    let [r, g, b] = color;
    let options = LayoutOptions::default()
        .with_style(if display {
            MathStyle::Display
        } else {
            MathStyle::Text
        })
        .with_color(Color {
            r: f32::from(r) / 255.0,
            g: f32::from(g) / 255.0,
            b: f32::from(b) / 255.0,
            a: 1.0,
        });
    let laid = layout(&nodes, &options);
    validate_layout(&laid).map_err(MathError::TooLarge)?;
    let list = to_display_list(&laid);
    let svg = render_to_svg(
        &list,
        &SvgOptions {
            font_size: UNITS_PER_EM,
            padding: PADDING_EM * UNITS_PER_EM,
            embed_glyphs: true,
            ..SvgOptions::default()
        },
    );
    let typeset = Typeset {
        svg: well_formed(&svg),
        width: list.width + 2.0 * PADDING_EM,
        height: list.height + list.depth + 2.0 * PADDING_EM,
        depth: list.depth + PADDING_EM,
    };
    let sizes = [typeset.width, typeset.height, typeset.depth];
    if sizes.iter().any(|size| !size.is_finite() || *size > MAX_EM) {
        return Err(MathError::TooLarge(format!(
            "it is larger than {MAX_EM} em"
        )));
    }
    if typeset.svg.len() > MAX_SVG_BYTES {
        return Err(MathError::TooLarge(format!(
            "its picture is larger than {} MiB",
            MAX_SVG_BYTES / (1024 * 1024)
        )));
    }
    Ok(typeset)
}

/// `svg` as well-formed XML: a character XML does not allow (a control
/// character of `\verb`, a noncharacter of `\text`) becomes U+FFFD.
fn well_formed(svg: &str) -> String {
    let allowed = |c: char| {
        matches!(c, '\t' | '\n' | '\r')
            || ('\u{20}'..='\u{d7ff}').contains(&c)
            || ('\u{e000}'..='\u{fffd}').contains(&c)
            || c >= '\u{10000}'
    };
    if svg.chars().all(allowed) {
        return svg.to_owned();
    }
    svg.chars()
        .map(|c| if allowed(c) { c } else { '\u{fffd}' })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    const WHITE: Rgb = [0xff, 0xff, 0xff];

    /// TC-004-38, FR-098, NFR-013: a formula is a standalone SVG of paths
    /// in the theme's text colour, sized in em, an inline one reaching
    /// below the baseline.
    #[test]
    fn tc004_38_formulas_typeset_as_svg_in_the_theme_colour() {
        let inline = typeset("E=mc^2", false, WHITE).unwrap();
        assert!(inline.svg.starts_with("<svg "), "{}", &inline.svg[..80]);
        assert!(inline.svg.contains("<path"), "glyphs are paths");
        assert!(!inline.svg.contains("<text"), "no fonts needed");
        assert!(inline.svg.contains("255,255,255"), "the text colour");
        assert!(inline.width > 2.0 && inline.height > 0.5);
        assert!(inline.depth > 0.0 && inline.depth < inline.height);
        let sum = typeset("\\sum_{i=1}^n i", true, WHITE).unwrap();
        let small = typeset("\\sum_{i=1}^n i", false, WHITE).unwrap();
        assert!(sum.height > small.height, "display style is larger");
    }

    /// OQ-222, ADR-029: every spelling of AsciiMath (and each Unicode form
    /// it reads), written as LaTeX, is typeset by RaTeX: alone, with
    /// operands after it, and between two.
    #[test]
    fn oq222_every_asciimath_spelling_typesets() {
        let mut failed = Vec::new();
        for (spelling, _) in asciimath_parser::asciimath_spellings() {
            for source in [
                spelling.to_owned(),
                format!("{spelling} x y"),
                format!("a {spelling} b"),
            ] {
                let latex = underline_markup::asciimath::to_latex(&source);
                if let Err(error) = typeset(&latex, false, WHITE) {
                    failed.push(format!("{source:?} -> {latex:?}: {error}"));
                }
            }
        }
        assert!(failed.is_empty(), "{failed:#?}");
    }

    /// OQ-222: the forms of the AsciiMath spec typeset.
    #[test]
    fn oq222_asciimath_forms_typeset() {
        for source in [
            "sum_(i=1)^n i^3=((n(n+1))/2)^2",
            "int_0^1 f(x) dx",
            "[(a,b),(c,d)]",
            "[(a, |, b), (c, |, d)]",
            "{: (x, y), (a, b) :}",
            "root(3)(x) + sqrt(y)",
            "color(red)(x) + text(if) y + \"50% & #1\"",
            "bbsfit x + bbcc C + overarc AB + cancel x + lim_(n->oo) a_n",
            "|__ x __| + |~ y ~| + (: a :) + norm(v) + ||x|| + |x|",
            // LaTeX's special characters, written as AsciiMath.
            "a \\ b ^ c ~ d",
            "\\",
            "x^",
            "~",
            "root(]a)(x) + color(#)(y) + color(#0a0)(z)",
        ] {
            let latex = underline_markup::asciimath::to_latex(source);
            assert!(
                typeset(&latex, true, WHITE).is_ok(),
                "{source:?} -> {latex:?}: {:?}",
                typeset(&latex, true, WHITE).err()
            );
        }
    }

    /// TC-004-38, UC-004 A21: a formula with an error says why and where.
    #[test]
    fn uc004_a21_an_error_gives_its_reason() {
        let error = typeset("\\frac{1}{", true, WHITE).unwrap_err();
        assert!(matches!(error, MathError::Syntax { .. }), "{error:?}");
        assert!(!error.to_string().is_empty());
        let unknown = typeset("a \\notacommand b", false, WHITE).unwrap_err();
        assert!(unknown.to_string().contains("\\notacommand"), "{unknown}");
    }

    /// UC-009 BR-003: the inline SVG of the HTML export is sized in em and
    /// labelled with its source.
    #[test]
    fn uc009_inline_svg_is_labelled_and_sized_in_em() {
        let formula = typeset("a<b", false, WHITE).unwrap();
        let svg = formula.inline_svg("a<b \"x\"");
        assert!(svg.starts_with("<svg role=\"img\" aria-label=\"a&lt;b &quot;x&quot;\""));
        assert!(svg.contains("em\" style=\"vertical-align:-"));
        assert!(!svg.contains("pt\""), "{}", &svg[..200]);
        assert_eq!(svg.matches(" width=").count(), 1);
    }

    /// NFR-002, ADR-029: how long a formula takes (printed; the first one
    /// loads the fonts).
    #[test]
    fn nfr002_render_time_per_formula() {
        let formulas = [
            "E=mc^2",
            "\\sum_{i=1}^n i = \\frac{n(n+1)}{2}",
            "\\int_0^1 x\\,dx",
            "e^{i\\pi}+1=0",
            "\\sqrt{2}",
            "\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}",
        ];
        let start = Instant::now();
        typeset(formulas[0], false, WHITE).unwrap();
        let first = start.elapsed();
        let start = Instant::now();
        let rounds = 20;
        let mut bytes = 0;
        for _ in 0..rounds {
            for formula in formulas {
                bytes += typeset(formula, true, WHITE).unwrap().svg.len();
            }
        }
        let each = start.elapsed() / (rounds * formulas.len() as u32);
        println!(
            "ADR-029: first formula {first:?}, then {each:?} per formula, {} bytes of SVG each",
            bytes / (rounds as usize * formulas.len())
        );
    }

    /// FR-098, NFR-002 (found by the test monkey): the SVG of a formula stays
    /// bounded. 19 KB of source (`\overbrace{abc}^{d}` 1000 times) gave 17 MB
    /// of SVG, 95 KB gave 90 MB, which the window loads as one picture and
    /// the session's cache of 32 MiB keeps whole. A formula beyond the bound
    /// is `TooLarge`.
    #[test]
    fn fr098_the_svg_of_a_formula_is_bounded() {
        const BOUND: usize = 4 * 1024 * 1024;
        for source in [
            "\\overbrace{abc}^{d}".repeat(1000),
            "\\frac{1}{2}".repeat(2000),
            format!(
                "\\begin{{array}}{{{}}}{}\\end{{array}}",
                "c".repeat(2000),
                vec!["a"; 2000].join("&")
            ),
        ] {
            match typeset(&source, true, WHITE) {
                Ok(formula) => assert!(
                    formula.svg.len() <= BOUND,
                    "{} bytes of source gave {} bytes of SVG",
                    source.len(),
                    formula.svg.len()
                ),
                Err(error) => assert!(matches!(error, MathError::TooLarge(_)), "{error:?}"),
            }
        }
    }

    /// FR-098, UC-004 A21 (found by the test monkey): the SVG is an XML
    /// document, whatever the source holds. A control character of `\verb`
    /// (U+0001) or a noncharacter (U+FFFE in `\text`) stood raw in a `<text>`
    /// element, which makes the picture an XML error: the window shows a
    /// broken image and the PDF export fails as a whole.
    #[test]
    fn fr098_the_svg_is_well_formed_xml_for_control_characters() {
        for source in ["\\verb|\u{1}\u{8}|", "\\text{\u{fffe}}", "\\verb|\u{b}|"] {
            let Ok(formula) = typeset(source, false, WHITE) else {
                continue;
            };
            let parsed =
                resvg::usvg::Tree::from_str(&formula.svg, &resvg::usvg::Options::default());
            assert!(parsed.is_ok(), "{source:?}: {:?}", parsed.err());
        }
    }

    /// FR-098, UC-009 BR-003 (property, test monkey): hostile sources never
    /// panic or hang (they typeset or say why), and the picture holds only
    /// the elements of a drawing, never a script, link or foreign object,
    /// with nothing from the source but in the quoted label.
    #[test]
    fn fr098_hostile_sources_give_only_a_drawing() {
        let long = "x".repeat(100_000);
        let nested = format!("{}x{}", "{".repeat(5000), "}".repeat(5000));
        let sources = [
            long.as_str(),
            nested.as_str(),
            "\\def\\a{\\a\\a}\\a",
            "\\text{<script>alert(1)</script>}",
            "\\text{\"onload=\"alert(1)}",
            "\\text{]]>}",
            "\\verb|<svg onload=x>|",
            "\\href{javascript:alert(1)}{x}",
            "\\url{http://e/\"onload=x}",
            "\\htmlClass{x onload=alert(1)}{b}",
            "\\htmlStyle{background:url(http://evil)}{b}",
            "\\color{red\"onload=x}x",
            "\\char\"D800 \\char\"110000",
            "</svg><script>alert(1)</script>",
        ];
        for source in sources {
            let started = Instant::now();
            let result = std::panic::catch_unwind(|| typeset(source, false, WHITE));
            assert!(started.elapsed().as_secs() < 5, "too slow: {:.40}", source);
            let Ok(Ok(formula)) = result else {
                assert!(result.is_ok(), "panicked: {:.40}", source);
                continue;
            };
            let html = formula.inline_svg(source);
            // Without the label there is no source text in the picture.
            let label_start = html.find("aria-label=\"").unwrap() + 12;
            let label_end = label_start + html[label_start..].find('"').unwrap();
            let drawing = format!("{}{}", &html[..label_start], &html[label_end..]);
            for needle in ["<script", "<a ", "<foreignObject", "<style", "javascript:"] {
                assert!(!drawing.contains(needle), "{needle} in {:.40}", source);
            }
            assert!(!html[label_start..label_end].contains(['<', '>']));
            for tag in drawing.split('<').skip(1) {
                let name: String = tag
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '/')
                    .collect();
                assert!(
                    ["svg", "/svg", "path", "rect", "text", "/text", "image", "g", "/g"]
                        .contains(&name.as_str()),
                    "element {name} in {:.40}",
                    source
                );
            }
        }
    }

    /// OQ-222, ADR-029 (property, test monkey): hostile AsciiMath (text holding
    /// `\`, braces, `$`, `%`, `#`, `&`, `\def`, `\href`, `]]`, `++++`, NUL and
    /// right-to-left text, unbalanced brackets, deep nesting) gives LaTeX that
    /// is brace-balanced, holds only control words the writer itself spells,
    /// and typesets or says why, within a few seconds, without a panic.
    #[test]
    fn oq222_hostile_asciimath_never_injects_latex_and_typesets_or_fails_clean() {
        let writer = include_str!("../../underline-markup/src/asciimath.rs");
        let mut known: std::collections::HashSet<String> = std::collections::HashSet::new();
        let bytes = writer.as_bytes();
        let mut at = 0;
        while at < bytes.len() {
            if bytes[at] == b'\\' {
                let mut end = at + 1;
                while end < bytes.len() && bytes[end].is_ascii_alphabetic() {
                    end += 1;
                }
                known.insert(writer[at + 1..end].to_owned());
                at = end.max(at + 1);
            } else {
                at += 1;
            }
        }
        // Words of AsciiMath's symbol table are written as `\word` by
        // `format!("\\{spelling}")`: the Greek letters.
        for name in [
            "alpha", "beta", "chi", "delta", "Delta", "eta", "gamma", "Gamma", "iota", "kappa",
            "varkappa", "lambda", "Lambda", "mu", "nu", "omega", "Omega", "Phi", "pi", "Pi",
            "varpi", "psi", "Psi", "rho", "varrho", "sigma", "Sigma", "tau", "theta", "vartheta",
            "Theta", "upsilon", "Upsilon", "xi", "Xi", "zeta",
        ] {
            known.insert(name.to_owned());
        }
        for name in [
            "sin", "cos", "tan", "sinh", "cosh", "tanh", "cot", "sec", "csc", "arcsin", "arccos",
            "arctan", "coth", "exp", "log", "ln", "det", "gcd",
        ] {
            known.insert(name.to_owned());
        }
        const PIECES: &[&str] = &[
            "text(",
            "\"",
            ")",
            "(",
            "[",
            "]",
            "{",
            "}",
            "{:",
            ":}",
            "\\",
            "\\def",
            "\\href",
            "\\input",
            "\\write18",
            "\\catcode",
            "\\newcommand",
            "\\url",
            "$",
            "%",
            "#",
            "&",
            "_",
            "^",
            "~",
            "]]",
            "++++",
            "\u{0}",
            "\u{5d0}\u{5d1}",
            "\u{202e}",
            "\n",
            "color(",
            "red",
            "#f00",
            "}\\input{x}",
            "mbox(",
            "sqrt",
            "frac",
            "root",
            "abs",
            "norm",
            "ubrace",
            "sum",
            "int",
            "lim",
            "oo",
            "x",
            "1",
            " ",
            "/",
            "|",
            "||",
            "\u{1F600}",
            "e\u{301}",
            "'",
            "!",
            "sin",
            "f",
            "bb",
            "cc",
            "hat",
            "stackrel",
            "id",
            "class",
            "text",
            "a b",
            "[(a,b),(c,d)]",
            "[(a,|,b)]",
            "<<",
            ">>",
            "lfloor",
            "|__",
            "__|",
            "|~",
            "~|",
            "obrace",
            "underset",
            "overset",
        ];
        let mut state = 0x1357_9bdf_u64;
        eprintln!("seed {state}");
        let started = std::time::Instant::now();
        let mut failures = Vec::new();
        let mut slowest = (std::time::Duration::ZERO, String::new());
        for round in 0..2500 {
            let mut source = String::new();
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let n = (state >> 33) % 24;
            for _ in 0..n {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                source.push_str(
                    PIECES[usize::try_from((state >> 33) % PIECES.len() as u64).unwrap()],
                );
            }
            let latex = underline_markup::asciimath::to_latex(&source);
            let mut depth = 0_i64;
            let mut escaped = false;
            let mut ok = true;
            let mut chars = latex.chars().peekable();
            while let Some(c) = chars.next() {
                if escaped {
                    escaped = false;
                    continue;
                }
                match c {
                    '\\' => {
                        escaped = true;
                        let mut word = String::new();
                        while let Some(&l) = chars.peek() {
                            if l.is_ascii_alphabetic() {
                                word.push(l);
                                chars.next();
                                escaped = false;
                            } else {
                                break;
                            }
                        }
                        if !word.is_empty() && !known.contains(&word) {
                            failures.push(format!("round {round}: unknown control word \\{word}: {source:?} -> {latex:?}"));
                            ok = false;
                        }
                    }
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    _ => {}
                }
                if depth < 0 {
                    ok = false;
                }
            }
            if depth != 0 || !ok && failures.is_empty() {
                failures.push(format!(
                    "round {round}: unbalanced ({depth}): {source:?} -> {latex:?}"
                ));
            }
            let t = std::time::Instant::now();
            let latex2 = latex.clone();
            let result = std::panic::catch_unwind(|| typeset(&latex2, false, WHITE));
            let took = t.elapsed();
            if took > slowest.0 {
                slowest = (took, format!("{source:?}"));
            }
            match result {
                Err(_) => failures.push(format!(
                    "round {round}: typeset panicked: {source:?} -> {latex:?}"
                )),
                Ok(Err(MathError::Engine(e))) => failures.push(format!(
                    "round {round}: engine failure {e}: {source:?} -> {latex:?}"
                )),
                Ok(_) => {}
            }
            if failures.len() > 15 {
                break;
            }
        }
        eprintln!("slowest {:?}, total {:?}", slowest, started.elapsed());
        assert!(failures.is_empty(), "{:#?}", failures);
    }
}
