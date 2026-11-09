//! The eleven themes of Underline (FR-022, FR-038, ADR-010): one palette
//! each, for the window, the text, and the highlighting of code and markup.
//! Themes with an official palette take its colors; where one of them does
//! not reach the contrast of NFR-010, it is darkened or lightened just
//! enough, which a test checks for every theme.

use underline_core::Theme;

/// A color as red, green, blue.
pub type Rgb = [u8; 3];

/// The colors of one theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub theme: Theme,
    /// Whether the theme is dark, for diagrams and the window's base look.
    pub dark: bool,
    /// The background of notes, and of the panels around them.
    pub background: Rgb,
    pub panel: Rgb,
    /// Body text, and weaker text such as markers and hints.
    pub foreground: Rgb,
    pub weak: Rgb,
    pub link: Rgb,
    /// The selection, drawn half transparent over the text.
    pub selection: Rgb,
    /// The background of code and markup blocks.
    pub code_background: Rgb,
    // Highlighting (ADR-010).
    pub comment: Rgb,
    pub keyword: Rgb,
    pub string: Rgb,
    pub number: Rgb,
    pub kind: Rgb,
    pub function: Rgb,
    pub variable: Rgb,
    pub heading: Rgb,
}

impl Palette {
    /// UC-008 BR-008: the color of search hits: the first of the theme's
    /// number, string, keyword, heading, and function colors that stands
    /// apart from its links and from the fill of a selection
    /// ([`Palette::chosen`]), so a hit is not taken for either.
    /// Its fill behind the text must stand apart from the selection's too.
    pub fn found(&self) -> Rgb {
        let candidates = [
            self.number,
            self.string,
            self.keyword,
            self.heading,
            self.function,
        ];
        // How far a color is from links and selection, the fill scaled so
        // that 45 apart counts as 80.
        let apart = |c: Rgb| {
            distance(c, self.link)
                .min(distance(c, self.chosen()))
                .min(distance(self.fill_of(c), self.chosen()) * 80.0 / 45.0)
        };
        candidates
            .into_iter()
            .find(|&c| apart(c) >= 80.0)
            .unwrap_or_else(|| {
                // None far enough: the one farthest from both.
                candidates
                    .into_iter()
                    .max_by(|&a, &b| apart(a).total_cmp(&apart(b)))
                    .unwrap_or(self.number)
            })
    }

    /// UC-008 BR-008: the fill behind a search hit in the text: the note
    /// background tinted with [`Palette::found`], as far as the text on it
    /// stays readable (NFR-010).
    pub fn found_background(&self) -> Rgb {
        self.fill_of(self.found())
    }

    /// The note background tinted with `color` as far as the text on it
    /// stays readable, at most halfway.
    fn fill_of(&self, color: Rgb) -> Rgb {
        (0..=10_u8)
            .rev()
            .map(|step| mix(self.background, color, f32::from(step) / 20.0))
            .find(|&fill| contrast(self.foreground, fill) >= 4.5)
            .unwrap_or(self.background)
    }

    /// The fill of a box set into the panel, such as the cursor place in the
    /// editor's status line: the panel, made darker until it stands apart
    /// from it, but no further. A panel too dark for that is lightened
    /// towards the text instead.
    pub fn inset(&self) -> Rgb {
        let apart = |towards: Rgb| {
            (1..=20_u8)
                .map(move |step| mix(self.panel, towards, f32::from(step) / 40.0))
                .find(|&fill| contrast(fill, self.panel) >= 1.2)
        };
        apart([0; 3])
            .or_else(|| apart(self.foreground))
            .unwrap_or(self.code_background)
    }

    /// FR-026: the colors of the input modes in the editor's status line,
    /// as normal, insert, visual, and command: each the first of its
    /// colors of the theme that is far enough from the ones before, so the
    /// modes look apart in every theme.
    pub fn mode_colors(&self) -> [Rgb; 4] {
        let wishes = [
            [
                self.link,
                self.function,
                self.kind,
                self.heading,
                self.variable,
            ],
            [
                self.string,
                self.number,
                self.heading,
                self.kind,
                self.variable,
            ],
            [
                self.keyword,
                self.kind,
                self.heading,
                self.number,
                self.variable,
            ],
            [
                self.function,
                self.number,
                self.kind,
                self.heading,
                self.variable,
            ],
        ];
        let mut chosen = [[0; 3]; 4];
        for (at, wish) in wishes.into_iter().enumerate() {
            chosen[at] = wish
                .into_iter()
                .find(|&c| chosen[..at].iter().all(|&o| distance(c, o) >= MODES_APART))
                .unwrap_or(wish[0]);
        }
        chosen
    }

    /// The text on a badge filled with `fill`, such as the input mode in the
    /// status line: the note background or the body text, whichever stands
    /// out more, moved towards black or white only as far as NFR-010 needs.
    pub fn text_on(&self, fill: Rgb) -> Rgb {
        let base = if contrast(self.background, fill) >= contrast(self.foreground, fill) {
            self.background
        } else {
            self.foreground
        };
        let end = if contrast([0; 3], fill) >= contrast([255; 3], fill) {
            [0; 3]
        } else {
            [255; 3]
        };
        // Pure black or white stands out at least as much as any step.
        (0..=20_u8)
            .map(|step| mix(base, end, f32::from(step) / 20.0))
            .find(|&text| contrast(text, fill) >= 4.5)
            .unwrap_or(end)
    }

    /// The fill of a switched-on toggle or a chosen entry: the panel moved a
    /// quarter of the way to the link color, so it stands out from the panel.
    pub fn chosen(&self) -> Rgb {
        mix(self.panel, self.link, 0.25)
    }

    /// The text on a chosen fill: the body text, moved towards white in dark
    /// themes and black in light ones only as far as NFR-010 needs.
    pub fn chosen_text(&self) -> Rgb {
        let end = if self.dark { [255; 3] } else { [0; 3] };
        (0..=20)
            .map(|step| mix(self.foreground, end, step as f32 / 20.0))
            .find(|&text| contrast(text, self.chosen()) >= 4.5)
            .unwrap_or(end)
    }

    /// The fill of a button that is not switched on: the panel moved a little
    /// towards the text, no further than the text stays readable on it.
    pub fn button(&self) -> Rgb {
        (0..=8)
            .rev()
            .map(|step| mix(self.panel, self.foreground, step as f32 / 100.0))
            .find(|&fill| contrast(self.foreground, fill) >= 4.5)
            .unwrap_or(self.panel)
    }
}

/// `a` moved the fraction `t` of the way to `b`.
fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let channel =
        |i: usize| (f32::from(a[i]) + (f32::from(b[i]) - f32::from(a[i])) * t).round() as u8;
    [channel(0), channel(1), channel(2)]
}

const fn hex(value: u32) -> Rgb {
    [(value >> 16) as u8, (value >> 8) as u8, value as u8]
}

/// Builds a palette from hex colors, in the order of the fields.
macro_rules! palette {
    ($theme:ident, $dark:expr, $($color:expr),+ $(,)?) => {{
        let [background, panel, foreground, weak, link, selection, code_background, comment,
            keyword, string, number, kind, function, variable, heading] = [$(hex($color)),+];
        Palette {
            theme: Theme::$theme, dark: $dark, background, panel, foreground, weak, link,
            selection, code_background, comment, keyword, string, number, kind, function,
            variable, heading,
        }
    }};
}

//                  background panel     text      weak      link      selection code bg   comment   keyword   string    number    type      function  variable  heading
const LIGHT: Palette = palette!(
    Light, false, 0xFFFFFF, 0xF2F2F2, 0x1F1F1F, 0x5C5C5C, 0x0B57D0, 0xB3D4FF, 0xF0F0F0, 0x6B6B6B,
    0x8A2BB8, 0x22863A, 0xB35900, 0x0E6B8C, 0x6F42C1, 0x1F1F1F, 0x0B57D0
);
const DARK: Palette = palette!(
    Dark, true, 0x1E1E1E, 0x252526, 0xD4D4D4, 0x9D9D9D, 0x4FC1FF, 0x264F78, 0x2D2D2D, 0x6A9955,
    0x569CD6, 0xCE9178, 0xB5CEA8, 0x4EC9B0, 0xDCDCAA, 0x9CDCFE, 0x569CD6
);
const GITHUB_LIGHT: Palette = palette!(
    GitHubLight,
    false,
    0xFFFFFF,
    0xF6F8FA,
    0x1F2328,
    0x59636E,
    0x0969DA,
    0xB6E3FF,
    0xF6F8FA,
    0x6E7781,
    0xCF222E,
    0x0A3069,
    0x0550AE,
    0x953800,
    0x8250DF,
    0x953800,
    0x0550AE
);
const GITHUB_DARK: Palette = palette!(
    GitHubDark, true, 0x0D1117, 0x161B22, 0xE6EDF3, 0x8D96A0, 0x58A6FF, 0x1F4A7A, 0x161B22,
    0x8B949E, 0xFF7B72, 0xA5D6FF, 0x79C0FF, 0xFFA657, 0xD2A8FF, 0xFFA657, 0x79C0FF
);
const SOLARIZED_LIGHT: Palette = palette!(
    SolarizedLight,
    false,
    0xFDF6E3,
    0xEEE8D5,
    0x4A5C63,
    0x5E7178,
    0x1A6A9E,
    0xE0D9C2,
    0xEEE8D5,
    0x93A1A1,
    0x7A8C00,
    0x26928A,
    0xD33682,
    0xA67E00,
    0x268BD2,
    0x268BD2,
    0xCB4B16
);
const SOLARIZED_DARK: Palette = palette!(
    SolarizedDark,
    true,
    0x002B36,
    0x073642,
    0x93A1A1,
    0x839496,
    0x4FAEE6,
    0x0E4A5A,
    0x073642,
    0x586E75,
    0x859900,
    0x2AA198,
    0xD54289,
    0xB58900,
    0x268BD2,
    0x268BD2,
    0xCE5523
);
const NIGHT_OWL: Palette = palette!(
    NightOwl, true, 0x011627, 0x0B2942, 0xD6DEEB, 0x8BA0B4, 0x82AAFF, 0x1D3B53, 0x0B2942, 0x637777,
    0xC792EA, 0xECC48D, 0xF78C6C, 0xFFCB8B, 0x82AAFF, 0xD6DEEB, 0x82AAFF
);
const UBUNTU: Palette = palette!(
    Ubuntu, true, 0x300A24, 0x2C001E, 0xEEEEEC, 0xB8A9B3, 0xFF8B5C, 0x5E2750, 0x3D1230, 0x9A8A96,
    0xF0844F, 0x87D75F, 0xFCE94F, 0xC9A0C4, 0x8FB8E8, 0xEEEEEC, 0xFF8B5C
);
const NORD: Palette = palette!(
    Nord, true, 0x2E3440, 0x3B4252, 0xD8DEE9, 0xA3ABB9, 0x88C0D0, 0x434C5E, 0x3B4252, 0x7B88A1,
    0x81A1C1, 0xA3BE8C, 0xB48EAD, 0x8FBCBB, 0x88C0D0, 0xD8DEE9, 0x88C0D0
);
const DRACULA: Palette = palette!(
    Dracula, true, 0x282A36, 0x21222C, 0xF8F8F2, 0xA9ADC9, 0x8BE9FD, 0x44475A, 0x343746, 0x6272A4,
    0xFF79C6, 0xF1FA8C, 0xBD93F9, 0x8BE9FD, 0x50FA7B, 0xF8F8F2, 0xBD93F9
);
const GRUVBOX: Palette = palette!(
    Gruvbox, true, 0x282828, 0x32302F, 0xEBDBB2, 0xA89984, 0x83A598, 0x504945, 0x3C3836, 0x928374,
    0xFB4934, 0xB8BB26, 0xD3869B, 0xFABD2F, 0x8EC07C, 0xEBDBB2, 0xFABD2F
);

const PALETTES: [Palette; 11] = [
    LIGHT,
    DARK,
    GITHUB_LIGHT,
    GITHUB_DARK,
    SOLARIZED_LIGHT,
    SOLARIZED_DARK,
    NIGHT_OWL,
    UBUNTU,
    NORD,
    DRACULA,
    GRUVBOX,
];

/// The palette of `theme`.
pub fn palette(theme: Theme) -> &'static Palette {
    PALETTES
        .iter()
        .find(|p| p.theme == theme)
        .expect("every theme has a palette")
}

/// How far apart two mode colors are at least, as [`distance`].
const MODES_APART: f32 = 60.0;

/// The distance of two colors in RGB.
fn distance(a: Rgb, b: Rgb) -> f32 {
    a.iter()
        .zip(b)
        .map(|(&x, y)| (f32::from(x) - f32::from(y)).powi(2))
        .sum::<f32>()
        .sqrt()
}

/// The WCAG 2.1 contrast ratio of two colors, from 1 to 21 (NFR-010).
pub fn contrast(a: Rgb, b: Rgb) -> f32 {
    fn luminance([r, g, b]: Rgb) -> f32 {
        let channel = |c: u8| {
            let c = f32::from(c) / 255.0;
            if c <= 0.039_28 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
    }
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TC-010-17, NFR-010: body text and links reach 4.5:1 on the note and
    /// the panels, weaker text and highlighted code 3:1, in every theme.
    #[test]
    fn uc010_tc17_every_theme_reaches_its_contrast() {
        let mut failures = Vec::new();
        let mut too_close = Vec::new();
        for theme in Theme::ALL {
            let p = palette(*theme);
            let mut check = |name: &str, color: Rgb, on: Rgb, least: f32| {
                let ratio = contrast(color, on);
                if ratio < least {
                    failures.push(format!("{}: {name} {ratio:.2} < {least}", theme.label()));
                }
            };
            check("text", p.foreground, p.background, 4.5);
            check("text on panel", p.foreground, p.panel, 4.5);
            check("text on code", p.foreground, p.code_background, 4.5);
            check("text on a chosen toggle", p.chosen_text(), p.chosen(), 4.5);
            check("text on a button", p.foreground, p.button(), 4.5);
            check(
                "chosen toggle against a button",
                p.chosen(),
                p.button(),
                1.2,
            );
            check("link", p.link, p.background, 4.5);
            check("weak", p.weak, p.background, 3.0);
            check("weak on panel", p.weak, p.panel, 3.0);
            check("text on an inset", p.foreground, p.inset(), 4.5);
            // UC-008 BR-008: hits stand out behind the text and in the
            // minimap, and the text on them stays readable.
            check("text on a hit", p.foreground, p.found_background(), 4.5);
            check(
                "hit fill against the note",
                p.found_background(),
                p.background,
                1.15,
            );
            check("hit in the minimap", p.found(), p.background, 2.5);
            // The hit's line under the text (its fill may come close to a
            // selection's in muted dark themes, the line does not).
            if distance(p.found(), p.chosen()) < 80.0 || distance(p.found(), p.link) < 60.0 {
                too_close.push(format!("{}: hit and selection or link", theme.label()));
            }
            check("inset against the panel", p.inset(), p.panel, 1.2);
            // FR-026: the badges and command keys of the input modes in the
            // editor's status line, apart from each other.
            let modes = p.mode_colors();
            let names = ["normal", "insert", "visual", "command"];
            for (at, (name, color)) in names.into_iter().zip(modes).enumerate() {
                check(
                    &format!("text on a {name} badge"),
                    p.text_on(color),
                    color,
                    4.5,
                );
                check(&format!("{name} keys on panel"), color, p.panel, 3.0);
                if modes[..at]
                    .iter()
                    .any(|&o| distance(color, o) < MODES_APART)
                {
                    too_close.push(format!("{}: {name} mode", theme.label()));
                }
            }
            for (name, color) in [
                ("keyword", p.keyword),
                ("string", p.string),
                ("number", p.number),
                ("type", p.kind),
                ("function", p.function),
                ("heading", p.heading),
            ] {
                check(name, color, p.code_background, 3.0);
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
        assert!(
            too_close.is_empty(),
            "too close to another mode: {too_close:#?}"
        );
    }

    #[test]
    fn every_theme_has_its_own_palette() {
        for theme in Theme::ALL {
            assert_eq!(palette(*theme).theme, *theme);
        }
        assert!(contrast([0, 0, 0], [255, 255, 255]) > 20.9);
    }
}
