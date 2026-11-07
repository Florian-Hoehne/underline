//! The properties of a note shown in the tool panel (UC-004 step 4, BR-010,
//! FR-019): its place in the workspace with its file name, its note format,
//! its size, and when it was last saved.

use std::io;
use std::path::Path;
use std::time::SystemTime;

use crate::embed::relative;
use crate::format::NoteFormat;

/// The properties of a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteProperties {
    /// The place in the workspace, with `/`, such as `guide/Install.adoc`.
    pub place: String,
    pub format: NoteFormat,
    /// Size of the file in bytes.
    pub size: u64,
    /// When the file was last saved.
    pub changed: SystemTime,
}

impl NoteProperties {
    /// UC-004 BR-010: the properties of the note in `file`, a file of the
    /// workspace at `root`.
    pub fn read(root: &Path, file: &Path, format: NoteFormat) -> io::Result<Self> {
        let meta = std::fs::metadata(file)?;
        let root = std::path::absolute(root).unwrap_or_else(|_| root.to_owned());
        let file = std::path::absolute(file).unwrap_or_else(|_| file.to_owned());
        Ok(Self {
            place: relative(&root, &file),
            format,
            size: meta.len(),
            changed: meta.modified()?,
        })
    }

    /// The size as people read it: `512 bytes`, `2 KB`, `1.4 MB`.
    pub fn size_text(&self) -> String {
        format_size(self.size)
    }

    /// The time of the last change in the local time zone, to the minute:
    /// `2026-11-01 09:30`.
    pub fn changed_text(&self) -> String {
        format_local_time(self.changed)
    }
}

/// A size in bytes as people read it.
pub fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if bytes < 1024 {
        format!("{bytes} bytes")
    } else if b < KB * KB {
        let kb = b / KB;
        if kb < 10.0 && kb.fract() >= 0.05 {
            format!("{kb:.1} KB")
        } else {
            format!("{:.0} KB", kb)
        }
    } else if b < KB * KB * KB {
        format!("{:.1} MB", b / (KB * KB))
    } else {
        format!("{:.1} GB", b / (KB * KB * KB))
    }
}

/// `time` in the local time zone, to the minute.
pub fn format_local_time(time: SystemTime) -> String {
    let zone = jiff::tz::TimeZone::system();
    match jiff::Timestamp::try_from(time) {
        Ok(timestamp) => timestamp
            .to_zoned(zone)
            .strftime("%Y-%m-%d %H:%M")
            .to_string(),
        Err(_) => String::from("unknown"),
    }
}

/// `time` in the local time zone as part of a file name: `2026-11-06-1430`.
pub(crate) fn local_stamp(time: SystemTime) -> String {
    let zone = jiff::tz::TimeZone::system();
    match jiff::Timestamp::try_from(time) {
        Ok(timestamp) => timestamp
            .to_zoned(zone)
            .strftime("%Y-%m-%d-%H%M")
            .to_string(),
        Err(_) => String::from("pasted"),
    }
}

/// `time` in the time zone `zone` (an IANA name), to the minute; for tests.
#[cfg(test)]
fn format_time_in(time: SystemTime, zone: &str) -> String {
    let zone = jiff::tz::TimeZone::get(zone).expect("time zone");
    jiff::Timestamp::try_from(time)
        .expect("timestamp")
        .to_zoned(zone)
        .strftime("%Y-%m-%d %H:%M")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    /// TC-004-07: place, format, size and last change of a note.
    #[test]
    fn uc004_tc07_note_properties() {
        let root = std::env::temp_dir().join(format!("underline-props-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("guide")).unwrap();
        let file = root.join("guide/Install.adoc");
        std::fs::write(&file, vec![b'x'; 2048]).unwrap();
        let properties = NoteProperties::read(&root, &file, NoteFormat::AsciiDoc).unwrap();
        assert_eq!(properties.place, "guide/Install.adoc");
        assert_eq!(properties.format, NoteFormat::AsciiDoc);
        assert_eq!(properties.size_text(), "2 KB");
        assert_eq!(properties.changed_text().len(), "2026-11-01 09:30".len());
        std::fs::remove_dir_all(&root).unwrap();

        // 2026-11-01 08:30 UTC is 09:30 in Berlin.
        let saved = UNIX_EPOCH + Duration::from_secs(1_793_521_800);
        assert_eq!(format_time_in(saved, "UTC"), "2026-11-01 08:30");
        assert_eq!(format_time_in(saved, "Europe/Berlin"), "2026-11-01 09:30");
    }

    #[test]
    fn sizes_as_people_read_them() {
        assert_eq!(format_size(512), "512 bytes");
        assert_eq!(format_size(2048), "2 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(300 * 1024), "300 KB");
        assert_eq!(format_size(1_468_006), "1.4 MB");
    }
}
