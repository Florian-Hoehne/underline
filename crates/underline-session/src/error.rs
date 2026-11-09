//! The error every command of the window returns.

use serde::Serialize;

/// What kind of failure a [`UiError`] reports, so the window can react to it
/// without parsing the message.
///
/// The kinds grow with the ported features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum UiErrorKind {
    /// Reading or writing a file or folder failed.
    Io,
    /// The window asked for something that does not fit the session's state,
    /// for example a tab that is no longer open.
    Invalid,
    /// A failure the user cannot act on.
    Internal,
}

/// A failure of a command, shown to the user.
///
/// `message` is the status text the egui app shows for the same failure, so
/// the messages stay the same after the migration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[error("{message}")]
pub struct UiError {
    /// What kind of failure this is.
    pub kind: UiErrorKind,
    /// The text shown to the user.
    pub message: String,
}

impl UiError {
    /// A failure of the given kind with the text shown to the user.
    pub fn new(kind: UiErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl From<std::io::Error> for UiError {
    fn from(error: std::io::Error) -> Self {
        Self::new(UiErrorKind::Io, error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_as_kind_and_message() {
        let error = UiError::new(UiErrorKind::Invalid, "The tab is closed");

        let json = serde_json::to_value(&error).unwrap();

        assert_eq!(
            json,
            serde_json::json!({ "kind": "invalid", "message": "The tab is closed" })
        );
    }

    #[test]
    fn io_errors_keep_their_text() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");

        let error = UiError::from(io);

        assert_eq!(error.kind, UiErrorKind::Io);
        assert_eq!(error.to_string(), "no such file");
    }
}
