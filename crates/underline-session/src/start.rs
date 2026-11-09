//! What the window learns when it starts (M-01).

use serde::Serialize;

use crate::SettingsView;

/// The operating system the app runs on.
///
/// The window needs it to name the command key in shortcut texts ("Cmd" on
/// macOS, "Ctrl" elsewhere).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    /// macOS.
    MacOs,
    /// Windows.
    Windows,
    /// Linux and every other system, which use the same shortcut texts.
    Linux,
}

impl Platform {
    /// The platform this program was built for.
    #[must_use]
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

/// What the window needs to know when it starts (M-01).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StartInfo {
    /// The workspace folder given on the command line (M-01.1), as a display
    /// string only. The backend opens the workspace from its own stored path,
    /// never from this string sent back.
    pub given_dir: Option<String>,
    /// The workspace opened most recently (M-22.1), as a display string.
    pub last_workspace: Option<String>,
    /// The known workspaces, the most recently opened first (M-22.1), as
    /// display strings.
    pub recent_workspaces: Vec<String>,
    /// The operating system, for the shortcut texts.
    pub platform: Platform,
    /// The settings in effect with the palette of the theme (M-01.6).
    pub settings: SettingsView,
    /// The names of all themes, in the order of the Theme menu (M-03.6).
    pub themes: Vec<String>,
    /// The status message at start, such as a note about the state file
    /// (M-01.3).
    pub status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m01_platform_serializes_lowercase() {
        assert_eq!(
            serde_json::to_value(Platform::MacOs).unwrap(),
            serde_json::json!("macos")
        );
        assert_eq!(
            serde_json::to_value(Platform::Windows).unwrap(),
            serde_json::json!("windows")
        );
        assert_eq!(
            serde_json::to_value(Platform::Linux).unwrap(),
            serde_json::json!("linux")
        );
    }
}
