//! The system clipboard with its formats (ADR-014): plain text, HTML with a
//! plain-text alternative, and images, through `arboard`; behind a trait so
//! the views are tested with a clipboard in memory.

use std::sync::{Arc, Mutex};

/// What the clipboard holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClipContent {
    pub text: Option<String>,
    pub html: Option<String>,
    pub image: Option<ClipImage>,
}

/// An image on the clipboard: RGBA, not premultiplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Reads and writes the clipboard.
pub trait Clipboard: Send {
    fn read(&mut self) -> ClipContent;
    /// Puts `content` on the clipboard: HTML with its text, or text alone.
    fn write(&mut self, content: &ClipContent);
}

/// The clipboard shared by the tabs of the window.
pub type SharedClipboard = Arc<Mutex<dyn Clipboard>>;

/// The clipboard of the system.
pub struct SystemClipboard {
    inner: Option<arboard::Clipboard>,
}

impl Default for SystemClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemClipboard {
    /// Opens the clipboard; without one (no display), reads give nothing.
    pub fn new() -> Self {
        Self {
            inner: arboard::Clipboard::new().ok(),
        }
    }

    /// The clipboard, shared.
    pub fn shared() -> SharedClipboard {
        Arc::new(Mutex::new(Self::new()))
    }
}

impl Clipboard for SystemClipboard {
    fn read(&mut self) -> ClipContent {
        let Some(clipboard) = self.inner.as_mut() else {
            return ClipContent::default();
        };
        let text = clipboard
            .get_text()
            .ok()
            .map(|t| t.replace("\r\n", "\n"))
            .filter(|t| !t.is_empty());
        let html = clipboard.get().html().ok().filter(|h| !h.trim().is_empty());
        // An image is read only when there is no text, as a screenshot.
        let image = if text.is_none() && html.is_none() {
            clipboard.get_image().ok().map(|image| ClipImage {
                width: image.width as u32,
                height: image.height as u32,
                rgba: image.bytes.into_owned(),
            })
        } else {
            None
        };
        ClipContent { text, html, image }
    }

    fn write(&mut self, content: &ClipContent) {
        let Some(clipboard) = self.inner.as_mut() else {
            return;
        };
        let text = content.text.clone().unwrap_or_default();
        let _ = match &content.html {
            Some(html) => clipboard.set_html(html.clone(), Some(text)),
            None => clipboard.set_text(text),
        };
    }
}

/// A clipboard in memory, for tests.
#[derive(Debug, Default)]
pub struct MemoryClipboard {
    pub content: ClipContent,
}

impl MemoryClipboard {
    pub fn holding(text: &str) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            content: ClipContent {
                text: Some(text.to_owned()),
                ..ClipContent::default()
            },
        }))
    }
}

impl Clipboard for MemoryClipboard {
    fn read(&mut self) -> ClipContent {
        self.content.clone()
    }

    fn write(&mut self, content: &ClipContent) {
        self.content = content.clone();
    }
}
