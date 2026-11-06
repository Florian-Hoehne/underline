//! Turning the SVG of a diagram into pixels for the facade (ADR-009), with
//! the font the renderers measured their text with.

use std::sync::OnceLock;

use resvg::{tiny_skia, usvg};

use super::DiagramError;

/// The pixels of a rendered diagram.
#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    /// Size in pixels.
    pub width: u32,
    pub height: u32,
    /// RGBA, premultiplied by alpha, row by row.
    pub pixels: Vec<u8>,
    /// The scale the picture was rasterized at: its size in points is its
    /// size in pixels divided by it.
    pub scale: f32,
}

impl Picture {
    /// The size of the picture in points.
    pub fn size(&self) -> [f32; 2] {
        [
            self.width as f32 / self.scale,
            self.height as f32 / self.scale,
        ]
    }
}

/// Largest picture side in pixels; a larger diagram is shown smaller.
const MAX_SIDE: f32 = 8192.0;

fn options() -> &'static usvg::Options<'static> {
    static OPTIONS: OnceLock<usvg::Options<'static>> = OnceLock::new();
    OPTIONS.get_or_init(|| {
        let mut options = usvg::Options::default();
        let fonts = options.fontdb_mut();
        for font in [
            dejavu::sans::regular(),
            dejavu::sans::bold(),
            dejavu::sans::oblique(),
            dejavu::sans::bold_oblique(),
            dejavu::sans_mono::regular(),
        ] {
            fonts.load_font_data(font.to_vec());
        }
        fonts.set_sans_serif_family("DejaVu Sans");
        fonts.set_serif_family("DejaVu Sans");
        fonts.set_monospace_family("DejaVu Sans Mono");
        options.font_family = "DejaVu Sans".into();
        options
    })
}

/// Rasterizes `svg` at `scale` pixels per point.
pub fn rasterize(svg: &str, scale: f32) -> Result<Picture, DiagramError> {
    let tree = usvg::Tree::from_str(svg, options())
        .map_err(|e| DiagramError::Engine(format!("the picture cannot be read: {e}")))?;
    let size = tree.size();
    let largest = size.width().max(size.height()) * scale;
    let scale = if largest > MAX_SIDE {
        scale * MAX_SIDE / largest
    } else {
        scale
    };
    let width = (size.width() * scale).ceil().max(1.0) as u32;
    let height = (size.height() * scale).ceil().max(1.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or(DiagramError::TooLarge)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(Picture {
        width,
        height,
        pixels: pixmap.take(),
        scale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rasterizes_at_the_display_scale() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="#ff0000"/><text x="2" y="15">Hi</text></svg>"##;
        let picture = rasterize(svg, 2.0).unwrap();
        assert_eq!((picture.width, picture.height), (80, 40));
        assert_eq!(picture.size(), [40.0, 20.0]);
        assert_eq!(&picture.pixels[..4], &[255, 0, 0, 255]);
        assert!(rasterize("not svg", 1.0).is_err());
    }
}
