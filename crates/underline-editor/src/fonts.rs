//! The fonts of the facade: DejaVu Sans in its regular, bold, oblique, and
//! bold oblique cuts, so bold and italic text look as on GitHub (C-003). The
//! fonts egui brings stay behind them for characters DejaVu lacks.

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

/// Names of the facade's font families.
pub const BODY: &str = "underline-body";
pub const BOLD: &str = "underline-bold";
pub const ITALIC: &str = "underline-italic";
pub const BOLD_ITALIC: &str = "underline-bold-italic";

/// Adds the facade's fonts to `ctx`; call once when the window starts.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(definitions());
}

/// The default fonts of egui with the facade's families added.
pub fn definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (family, data) in [
        (BODY, dejavu::sans::regular()),
        (BOLD, dejavu::sans::bold()),
        (ITALIC, dejavu::sans::oblique()),
        (BOLD_ITALIC, dejavu::sans::bold_oblique()),
    ] {
        fonts
            .font_data
            .insert(family.to_owned(), FontData::from_static(data));
        let mut list = vec![family.to_owned()];
        list.extend(fallback.iter().cloned());
        fonts.families.insert(FontFamily::Name(family.into()), list);
    }
    fonts
}

/// The family for text that is bold and/or italic.
pub fn family(bold: bool, italic: bool) -> FontFamily {
    FontFamily::Name(
        match (bold, italic) {
            (false, false) => BODY,
            (true, false) => BOLD,
            (false, true) => ITALIC,
            (true, true) => BOLD_ITALIC,
        }
        .into(),
    )
}
