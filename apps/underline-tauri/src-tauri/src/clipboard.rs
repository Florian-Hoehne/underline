//! The system clipboard of the window (ADR-014, M-08.6), through `arboard`,
//! as the egui app uses it. The commands read and write it without holding
//! the session. Feature F3 needs plain text; HTML and images come with
//! feature F6.

/// The clipboard of the system, kept open while the app runs: on Linux the
/// text Underline copied stays available only as long as it is open.
pub struct SystemClipboard {
    inner: Option<arboard::Clipboard>,
    /// Without a system clipboard (tests, no display): the text in memory.
    memory: Option<String>,
}

impl SystemClipboard {
    /// Opens the clipboard; without one (no display), reads give nothing and
    /// writes are dropped, as in the egui app.
    pub fn new() -> Self {
        Self {
            inner: arboard::Clipboard::new().ok(),
            memory: None,
        }
    }

    /// A clipboard in memory only, for tests.
    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self {
            inner: None,
            memory: None,
        }
    }

    /// The plain text on the clipboard, with `\n` line endings; `None` when
    /// it holds no text.
    pub fn read_text(&mut self) -> Option<String> {
        let text = match self.inner.as_mut() {
            Some(clipboard) => clipboard.get_text().ok(),
            None => self.memory.clone(),
        };
        text.map(|text| text.replace("\r\n", "\n"))
            .filter(|text| !text.is_empty())
    }

    /// Puts `text` on the clipboard.
    pub fn write_text(&mut self, text: &str) {
        match self.inner.as_mut() {
            Some(clipboard) => {
                if let Err(error) = clipboard.set_text(text) {
                    eprintln!("The clipboard could not be written: {error}");
                }
            }
            None => self.memory = Some(text.to_owned()),
        }
    }
}
