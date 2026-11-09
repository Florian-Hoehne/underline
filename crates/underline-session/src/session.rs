//! The session of one window.

use std::path::PathBuf;

use crate::{Platform, StartInfo};

/// The state of one window: for now only what was given on the command line.
///
/// This is a placeholder. The workspace, the tabs, the background services
/// and `tick` are ported here from the egui app feature by feature.
#[derive(Debug, Default)]
pub struct Session {
    /// The authority for the folder. The window only gets a display string of
    /// it (`StartInfo::given_dir`) and never sends a path back.
    given_dir: Option<PathBuf>,
}

impl Session {
    /// A session for a window started with the optional workspace folder from
    /// the command line (M-01.1).
    #[must_use]
    pub fn new(given_dir: Option<PathBuf>) -> Self {
        Self { given_dir }
    }

    /// What the window needs to know when it starts (M-01).
    #[must_use]
    pub fn start_info(&self) -> StartInfo {
        StartInfo {
            given_dir: self
                .given_dir
                .as_ref()
                .map(|dir| dir.to_string_lossy().into_owned()),
            platform: Platform::current(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m01_1_start_info_passes_the_given_dir_along() {
        let session = Session::new(Some(PathBuf::from("notes")));

        let info = session.start_info();

        assert_eq!(info.given_dir.as_deref(), Some("notes"));
        assert_eq!(info.platform, Platform::current());
    }

    #[test]
    fn m01_1_start_info_without_a_dir() {
        assert_eq!(Session::default().start_info().given_dir, None);
    }
}
