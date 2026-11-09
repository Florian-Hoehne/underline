//! What the window learns when it starts (M-01).

use serde::Serialize;

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
    /// The operating system, for the shortcut texts.
    pub platform: Platform,
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

    #[test]
    fn m01_start_info_uses_camel_case_fields() {
        let info = StartInfo {
            given_dir: Some("/notes".into()),
            platform: Platform::Linux,
        };

        assert_eq!(
            serde_json::to_value(info).unwrap(),
            serde_json::json!({ "givenDir": "/notes", "platform": "linux" })
        );
    }
}
