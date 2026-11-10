//! The system clipboard of the window (ADR-014, M-08.6, M-09.14, M-10.6,
//! M-10.7), through `arboard`, as the egui app uses it
//! (`crates/underline-editor/src/clipboard.rs`): plain text, HTML with a
//! plain-text alternative, and images. The commands read and write it
//! without holding the session.
//!
//! Copy on select (FR-031, BR-011) keeps what the clipboard held before a
//! selection was copied, one per note, so pasting over that selection
//! brings it back; the editor core in the window knows when the selection
//! is the copied one, and says when it is gone (`forget`); a closed tab
//! lets go too. An image is kept that way only up to [`KEPT_IMAGE_AT_MOST`].
//!
//! `arboard` cannot tell whether the clipboard holds an image without
//! reading it whole, so a paste reads it once (`read_for_paste`) and keeps
//! it for storing it right after (`take_pasted_image`).

use std::collections::HashMap;

/// The largest image (RGBA bytes, 4096 × 1024) kept for pasting over a
/// copied selection; a larger one is not kept (its text and HTML are).
pub const KEPT_IMAGE_AT_MOST: usize = 16 * 1024 * 1024;

/// An image on the clipboard: RGBA, not premultiplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// What the clipboard holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClipContent {
    /// The plain text, with `\n` line endings; `None` without text.
    pub text: Option<String>,
    /// HTML of other programs; `None` without.
    pub html: Option<String>,
    /// An image, read only when there is neither text nor HTML.
    pub image: Option<ClipImage>,
}

/// The clipboard of the system, kept open while the app runs: on Linux the
/// text Underline copied stays available only as long as it is open.
pub struct SystemClipboard {
    inner: Option<arboard::Clipboard>,
    /// Without a system clipboard (tests, no display): the content in
    /// memory.
    memory: ClipContent,
    /// BR-011: per note, what the clipboard held before copy on select
    /// copied its selection.
    before: HashMap<String, ClipContent>,
    /// The image the last paste read, for storing it (M-10.6).
    pasted_image: Option<ClipImage>,
}

/// What a paste finds on the clipboard, the image itself kept aside.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PasteRead {
    pub text: Option<String>,
    pub html: Option<String>,
    pub image: bool,
}

impl SystemClipboard {
    /// Opens the clipboard; without one (no display), reads give nothing and
    /// writes stay in memory, as in the egui app.
    pub fn new() -> Self {
        Self {
            inner: arboard::Clipboard::new().ok(),
            memory: ClipContent::default(),
            before: HashMap::new(),
            pasted_image: None,
        }
    }

    /// A clipboard in memory only, for tests.
    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self {
            inner: None,
            memory: ClipContent::default(),
            before: HashMap::new(),
            pasted_image: None,
        }
    }

    /// Sets what the clipboard holds, for tests.
    #[cfg(test)]
    pub fn hold(&mut self, content: ClipContent) {
        self.memory = content;
    }

    /// The plain text on the clipboard, with `\n` line endings; `None` when
    /// it holds no text (Vim's `"+`).
    pub fn read_text(&mut self) -> Option<String> {
        let text = match self.inner.as_mut() {
            Some(clipboard) => clipboard.get_text().ok(),
            None => self.memory.text.clone(),
        };
        text.map(|text| text.replace("\r\n", "\n"))
            .filter(|text| !text.is_empty())
    }

    /// Puts `text` on the clipboard (Vim's `"+`).
    pub fn write_text(&mut self, text: &str) {
        self.write(text, None);
    }

    /// What the clipboard holds: text, HTML, and an image only when there
    /// is neither, as a screenshot (`SystemClipboard::read` of egui).
    pub fn read(&mut self) -> ClipContent {
        let Some(clipboard) = self.inner.as_mut() else {
            return self.memory.clone();
        };
        let text = clipboard
            .get_text()
            .ok()
            .map(|t| t.replace("\r\n", "\n"))
            .filter(|t| !t.is_empty());
        let html = clipboard.get().html().ok().filter(|h| !h.trim().is_empty());
        let image = if text.is_none() && html.is_none() {
            clipboard.get_image().ok().map(|image| ClipImage {
                width: u32::try_from(image.width).unwrap_or(0),
                height: u32::try_from(image.height).unwrap_or(0),
                rgba: image.bytes.into_owned(),
            })
        } else {
            None
        };
        ClipContent { text, html, image }
    }

    /// Puts `text` on the clipboard, with `html` for other programs when
    /// given (ADR-014, BR-011).
    pub fn write(&mut self, text: &str, html: Option<&str>) {
        let Some(clipboard) = self.inner.as_mut() else {
            self.memory = ClipContent {
                text: Some(text.to_owned()).filter(|t| !t.is_empty()),
                html: html.map(str::to_owned),
                image: None,
            };
            return;
        };
        let written = match html {
            Some(html) => clipboard.set_html(html, Some(text)),
            None => clipboard.set_text(text),
        };
        if let Err(error) = written {
            eprintln!("The clipboard could not be written: {error}");
        }
    }

    /// FR-031, BR-011: copy on select copies a selection of `note` for the
    /// first time; what the clipboard holds now is kept for pasting over it.
    pub fn keep_before(&mut self, note: &str) {
        let mut before = self.read();
        if before
            .image
            .as_ref()
            .is_some_and(|image| image.rgba.len() > KEPT_IMAGE_AT_MOST)
        {
            before.image = None;
        }
        self.before.insert(note.to_owned(), before);
    }

    /// The copied selection of `note` is gone, or its tab closed: what the
    /// clipboard held before is let go.
    pub fn forget(&mut self, note: &str) {
        self.before.remove(note);
    }

    /// UC-006 step 7: the note `from` is `to` now; what was kept for its
    /// copied selection goes with it.
    pub fn rename(&mut self, from: &str, to: &str) {
        if let Some(before) = self.before.remove(from) {
            self.before.insert(to.to_owned(), before);
        }
    }

    /// What a paste in `note` finds (see [`Self::content`]); an image is
    /// kept for [`Self::take_pasted_image`] instead of being returned.
    pub fn read_for_paste(&mut self, note: &str, over_copied: bool) -> PasteRead {
        let content = self.content(note, over_copied);
        let image = content.image.is_some();
        self.pasted_image = content.image;
        PasteRead {
            text: content.text,
            html: content.html,
            image,
        }
    }

    /// M-10.6: the image of the paste just read, or else read now.
    pub fn take_pasted_image(&mut self, note: &str, over_copied: bool) -> Option<ClipImage> {
        self.pasted_image
            .take()
            .or_else(|| self.content(note, over_copied).image)
    }

    /// What a paste in `note` takes: over the selection copy on select
    /// copied (`over_copied`), what the clipboard held before, if kept;
    /// otherwise what it holds now.
    pub fn content(&mut self, note: &str, over_copied: bool) -> ClipContent {
        if over_copied {
            if let Some(before) = self.before.get(note) {
                return before.clone();
            }
        }
        self.read()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// M-10.7, BR-011 (TC-005-17): the content before a copy on select is
    /// kept per note and taken only over the copied selection.
    #[test]
    fn m10_7_the_content_before_copy_on_select_is_kept_per_note() {
        let mut clipboard = SystemClipboard::in_memory();
        clipboard.write("former", Some("<b>former</b>"));
        clipboard.keep_before("Home");
        clipboard.write("selected", None);
        let before = clipboard.content("Home", true);
        assert_eq!(before.text.as_deref(), Some("former"));
        assert_eq!(before.html.as_deref(), Some("<b>former</b>"));
        assert_eq!(
            clipboard.content("Home", false).text.as_deref(),
            Some("selected")
        );
        assert_eq!(
            clipboard.content("Plan", true).text.as_deref(),
            Some("selected"),
            "nothing kept for another note"
        );
        assert_eq!(clipboard.read_text().as_deref(), Some("selected"));
        clipboard.forget("Home");
        assert_eq!(
            clipboard.content("Home", true).text.as_deref(),
            Some("selected"),
            "let go"
        );
    }

    /// Review F6: a large image is not kept for pasting over a selection;
    /// a paste reads an image once and stores that one.
    #[test]
    fn m10_6_large_images_are_not_kept_and_a_paste_reads_once() {
        let mut clipboard = SystemClipboard::in_memory();
        let big = ClipImage {
            width: 4096,
            height: 1025,
            rgba: vec![0; 4096 * 1025 * 4],
        };
        clipboard.hold(ClipContent {
            image: Some(big.clone()),
            ..ClipContent::default()
        });
        clipboard.keep_before("Home");
        clipboard.write("sel", None);
        assert_eq!(clipboard.content("Home", true).image, None);

        clipboard.hold(ClipContent {
            image: Some(big.clone()),
            ..ClipContent::default()
        });
        let read = clipboard.read_for_paste("Home", false);
        assert!(read.image);
        clipboard.write("changed", None);
        assert_eq!(clipboard.take_pasted_image("Home", false), Some(big));
        assert_eq!(clipboard.take_pasted_image("Home", false), None);
    }
}
