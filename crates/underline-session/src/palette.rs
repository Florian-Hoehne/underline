//! The colours of a theme for the window (M-01.6, FR-022): every colour of
//! `underline_facade::themes::Palette` and the colours derived from it, as
//! CSS hex strings.

use serde::Serialize;
use underline_facade::themes::{Palette, Rgb};

/// The palette of one theme, as `#rrggbb` strings.
///
/// The fields are those of `underline_facade::themes::Palette`, one to one,
/// followed by the colours the egui app derives from them (`visuals()` and
/// the editor), so the window computes no colour of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PaletteView {
    /// The theme's name, such as "Solarized Dark".
    pub theme: String,
    /// Whether the theme is dark; sets the window's `color-scheme`.
    pub dark: bool,
    /// The background of notes and of text fields.
    pub background: String,
    /// The background of the panels: top bar, menus, status bar.
    pub panel: String,
    /// Body text.
    pub foreground: String,
    /// Weaker text such as markers and hints.
    pub weak: String,
    /// Links.
    pub link: String,
    /// The selection in the editor, drawn half transparent over the text.
    pub selection: String,
    /// The background of code and markup blocks.
    pub code_background: String,
    /// Highlighting (ADR-010): comments.
    pub comment: String,
    /// Highlighting: keywords.
    pub keyword: String,
    /// Highlighting: strings.
    pub string: String,
    /// Highlighting: numbers.
    pub number: String,
    /// Highlighting: types.
    pub kind: String,
    /// Highlighting: functions.
    pub function: String,
    /// Highlighting: variables.
    pub variable: String,
    /// Highlighting: headings.
    pub heading: String,
    /// The fill of a switched-on toggle or a chosen entry (`Palette::chosen`).
    pub chosen: String,
    /// The text on [`PaletteView::chosen`] (`Palette::chosen_text`).
    pub chosen_text: String,
    /// The fill of a button that is not switched on (`Palette::button`).
    pub button: String,
    /// Search hits (`Palette::found`, UC-008 BR-008).
    pub found: String,
    /// The fill behind a search hit in the text (`Palette::found_background`).
    pub found_background: String,
    /// The fill of a box set into the panel (`Palette::inset`).
    pub inset: String,
    /// The input modes in the editor's status line, as normal, insert,
    /// visual and command (`Palette::mode_colors`, FR-026).
    pub mode_colors: Vec<String>,
    /// The text on each of [`PaletteView::mode_colors`] (`Palette::text_on`).
    pub mode_text: Vec<String>,
}

impl PaletteView {
    /// The view of `palette`.
    #[must_use]
    pub fn of(palette: &Palette) -> Self {
        let modes = palette.mode_colors();
        Self {
            theme: palette.theme.label().to_owned(),
            dark: palette.dark,
            background: hex(palette.background),
            panel: hex(palette.panel),
            foreground: hex(palette.foreground),
            weak: hex(palette.weak),
            link: hex(palette.link),
            selection: hex(palette.selection),
            code_background: hex(palette.code_background),
            comment: hex(palette.comment),
            keyword: hex(palette.keyword),
            string: hex(palette.string),
            number: hex(palette.number),
            kind: hex(palette.kind),
            function: hex(palette.function),
            variable: hex(palette.variable),
            heading: hex(palette.heading),
            chosen: hex(palette.chosen()),
            chosen_text: hex(palette.chosen_text()),
            button: hex(palette.button()),
            found: hex(palette.found()),
            found_background: hex(palette.found_background()),
            inset: hex(palette.inset()),
            mode_colors: modes.iter().map(|&c| hex(c)).collect(),
            mode_text: modes.iter().map(|&c| hex(palette.text_on(c))).collect(),
        }
    }

    /// The palettes of all themes, in the order of the Theme menu.
    #[must_use]
    pub fn all() -> Vec<Self> {
        underline_core::Theme::ALL
            .iter()
            .map(|&theme| Self::of(underline_facade::themes::palette(theme)))
            .collect()
    }
}

/// `color` as a CSS hex colour, `#rrggbb`.
fn hex([r, g, b]: Rgb) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

#[cfg(test)]
mod tests {
    use underline_core::Theme;
    use underline_facade::themes::palette;

    use super::*;

    #[test]
    fn m01_6_hex_has_two_digits_per_channel() {
        assert_eq!(hex([0x0b, 0x57, 0xd0]), "#0b57d0");
        assert_eq!(hex([0, 0, 0]), "#000000");
    }

    #[test]
    fn m01_6_the_view_takes_the_palette_one_to_one() {
        let p = palette(Theme::Light);

        let view = PaletteView::of(p);

        assert_eq!(view.theme, "Light");
        assert!(!view.dark);
        assert_eq!(view.background, "#ffffff");
        assert_eq!(view.panel, "#f2f2f2");
        assert_eq!(view.foreground, "#1f1f1f");
        assert_eq!(view.chosen, hex(p.chosen()));
        assert_eq!(view.chosen_text, hex(p.chosen_text()));
        assert_eq!(view.button, hex(p.button()));
        assert_eq!(view.mode_colors.len(), 4);
        assert_eq!(view.mode_text[0], hex(p.text_on(p.mode_colors()[0])));
    }

    #[test]
    fn m01_6_all_eleven_themes() {
        let all = PaletteView::all();

        assert_eq!(all.len(), 11);
        assert_eq!(all[10].theme, "Gruvbox");
    }
}
