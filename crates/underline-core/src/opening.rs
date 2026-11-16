//! What a followed link may open without asking (UC-004 A4, BR-007, Q-61,
//! Q-62): web addresses and mail open directly; an address of any other
//! scheme, a file that runs as a program, and a `file://` target are asked
//! about first, every time.

use std::path::Path;

/// UC-004 BR-007, Q-62: the schemes an address opens with, without asking.
const DIRECT_SCHEMES: [&str; 3] = ["http", "https", "mailto"];

/// UC-004 BR-007, Q-62: the scheme of `address` in lower case, such as
/// `https` of `https://example.org` or `tel` of `tel:+41…`; `None` when it
/// starts with no scheme.
///
/// ```
/// use underline_core::link_scheme;
/// assert_eq!(link_scheme("HTTPS://example.org").as_deref(), Some("https"));
/// assert_eq!(link_scheme("ms-msdt:/id").as_deref(), Some("ms-msdt"));
/// assert_eq!(link_scheme("docs/a.pdf"), None);
/// ```
pub fn link_scheme(address: &str) -> Option<String> {
    let (scheme, _) = address.trim().split_once(':')?;
    let mut chars = scheme.chars();
    let first = chars.next()?;
    let valid = first.is_ascii_alphabetic()
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid.then(|| scheme.to_ascii_lowercase())
}

/// UC-004 BR-007, Q-62: whether `address` opens without asking: only
/// `http`, `https` and `mailto` addresses do.
pub fn opens_without_asking(address: &str) -> bool {
    link_scheme(address).is_some_and(|scheme| DIRECT_SCHEMES.contains(&scheme.as_str()))
}

/// UC-004 BR-007, Q-62: the scheme of `address` as the question names it:
/// the text before the first `:`, every character that is not a visible
/// ASCII one written as `\u{…}` (a byte order mark, a zero-width space or
/// a Cyrillic `һ` before `ttp` so shows), and `?` when there is none.
///
/// ```
/// use underline_core::shown_scheme;
/// assert_eq!(shown_scheme("SSH://host"), "ssh");
/// assert_eq!(shown_scheme("\u{feff}http://x"), "\\u{feff}http");
/// assert_eq!(shown_scheme("://x"), "?");
/// ```
pub fn shown_scheme(address: &str) -> String {
    if let Some(scheme) = link_scheme(address) {
        return scheme;
    }
    let raw = address
        .split_once(':')
        .map_or(address, |(before, _)| before);
    let shown: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() {
                c.to_string()
            } else {
                format!("\\u{{{:x}}}", u32::from(c))
            }
        })
        .collect();
    if shown.is_empty() {
        "?".into()
    } else {
        shown
    }
}

/// UC-004 BR-007, Q-61, Q-62: `text` as a question shows it: control
/// characters, characters that turn the direction of the text, and
/// invisible ones are written as `\u{…}`, so a name cannot hide its
/// ending (`evil\u{202e}fdp.exe` would read `evilexe.pdf`).
pub fn shown_text(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() || is_hidden(c) {
                format!("\\u{{{:x}}}", u32::from(c))
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// Characters that change the direction of text or show nothing.
fn is_hidden(c: char) -> bool {
    matches!(
        c,
        '\u{061c}'
            | '\u{00ad}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{2069}'
            | '\u{feff}'
    )
}

/// UC-004 BR-007, Q-61: whether `place` names a file on another computer
/// or a device of Windows (`\\server\share`, `//server/share`, `\\?\…`,
/// `\\.\…`): it starts with two slashes of either kind. Decided on the
/// text alone: such a place must not be looked at (which may reach out to
/// the other computer) before the user agreed to open it.
///
/// ```
/// use underline_core::is_remote_place;
/// assert!(is_remote_place(r"\\server\share\a.pdf"));
/// assert!(is_remote_place("//server/share"));
/// assert!(!is_remote_place("/home/me/a.pdf"));
/// ```
pub fn is_remote_place(place: &str) -> bool {
    let mut chars = place.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some('/' | '\\'), Some('/' | '\\'))
    )
}

/// UC-004 BR-007, Q-61: endings of files the system runs, or hands to a
/// program that runs them, when it opens them, each with the kind the
/// question names. Compared without regard to case.
///
/// The list is deliberately not complete: a system may run more kinds of
/// files, and programs the user installs add their own. It holds what the
/// common systems run or launch from a double click; a file with an
/// execute permission counts as a program besides.
const EXECUTABLE_ENDINGS: [(&str, &str); 46] = [
    ("desktop", "a desktop launcher"),
    ("sh", "a shell script"),
    ("command", "a shell script"),
    ("run", "a program"),
    ("bin", "a program"),
    ("appimage", "a program"),
    ("py", "a Python script"),
    ("pl", "a Perl script"),
    ("rb", "a Ruby script"),
    ("bat", "a batch file"),
    ("cmd", "a batch file"),
    ("lnk", "a Windows shortcut"),
    ("url", "an internet shortcut"),
    ("webloc", "an internet shortcut"),
    ("inetloc", "an internet shortcut"),
    ("exe", "a Windows program"),
    ("com", "a Windows program"),
    ("pif", "a Windows program"),
    ("scf", "a Windows Explorer command"),
    ("msi", "a Windows installer"),
    ("msp", "a Windows installer"),
    ("inf", "a Windows setup file"),
    ("application", "a Windows program"),
    ("appref-ms", "a Windows program"),
    ("gadget", "a Windows gadget"),
    ("scr", "a screen saver program"),
    ("ps1", "a PowerShell script"),
    ("js", "a script"),
    ("jse", "a Windows script"),
    ("vbs", "a Windows script"),
    ("vbe", "a Windows script"),
    ("wsf", "a Windows script"),
    ("wsh", "a Windows script"),
    ("hta", "an HTML application"),
    ("chm", "a Windows help file"),
    ("msc", "a Windows console"),
    ("cpl", "a Control Panel item"),
    ("reg", "a registry file"),
    ("library-ms", "a Windows library"),
    ("search-ms", "a Windows search"),
    ("settingcontent-ms", "a Windows settings shortcut"),
    ("jar", "a Java program"),
    ("app", "a macOS app"),
    ("terminal", "a Terminal setting"),
    ("workflow", "an Automator workflow"),
    ("scpt", "an AppleScript"),
];

/// UC-004 BR-007, Q-61: what kind of program the file at `path` is, with
/// its article ("a shell script"), when opening it may run it: one with an
/// ending of [`EXECUTABLE_ENDINGS`] (`.desktop`, `.bat`, `.lnk`, `.exe`,
/// `.sh`, …), or on Unix a file with an execute permission. A symbolic
/// link counts as what it leads to. A folder is no program, whatever its
/// name, except a macOS app, which is a folder. `None` for anything else.
///
/// The answer holds for the moment it is asked; the file may change before
/// it is opened. Where the system does not keep execute permissions (a
/// Windows drive mounted in Linux or WSL shows every file as executable),
/// every file there counts as a program.
pub fn executable_kind(path: &Path) -> Option<&'static str> {
    let target = std::fs::canonicalize(path).ok();
    let folder = std::fs::metadata(path).is_ok_and(|meta| meta.is_dir());
    let named = executable_by_name(path).or_else(|| target.as_deref().and_then(executable_by_name));
    if folder {
        return named.filter(|kind| *kind == "a macOS app");
    }
    named.or_else(|| has_execute_permission(path).then_some("a program"))
}

/// UC-004 BR-007, Q-61: [`executable_kind`] by the name of `path` alone,
/// without looking at the file. Windows ignores dots and spaces at the end
/// of a name and a stream after a `:` (`a.exe.`, `a.exe::$DATA` run
/// `a.exe`), so they are left out before the ending is read, on every
/// system.
pub fn executable_by_name(path: &Path) -> Option<&'static str> {
    let name = path.file_name()?.to_string_lossy();
    let name = name.split(':').next().unwrap_or_default();
    let name = name.trim_end_matches(['.', ' ']);
    let (_, ending) = name.rsplit_once('.')?;
    EXECUTABLE_ENDINGS
        .iter()
        .find(|(known, _)| known.eq_ignore_ascii_case(ending))
        .map(|(_, kind)| *kind)
}

#[cfg(unix)]
fn has_execute_permission(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn has_execute_permission(_path: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UC-004 BR-007, Q-62: only http, https and mailto open directly.
    #[test]
    fn uc004_br007_q62_only_http_https_and_mailto_open_directly() {
        for address in [
            "https://example.org",
            "HTTP://example.org/a?b",
            "mailto:me@example.org",
            "  MailTo:me@example.org",
        ] {
            assert!(opens_without_asking(address), "{address}");
        }
        for address in [
            "ms-msdt:/id PCWDiagnostic",
            "smb://server/share",
            "ssh://host",
            "ftp://example.org",
            "tel:+41123",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https",
            "docs/a.pdf",
            "",
        ] {
            assert!(!opens_without_asking(address), "{address}");
        }
    }

    /// UC-004 BR-007, Q-62: the scheme the question names.
    #[test]
    fn uc004_br007_q62_the_scheme_of_an_address() {
        assert_eq!(link_scheme("SSH://host").as_deref(), Some("ssh"));
        assert_eq!(link_scheme("ms-msdt:/x").as_deref(), Some("ms-msdt"));
        assert_eq!(link_scheme("urn:isbn:1").as_deref(), Some("urn"));
        assert_eq!(link_scheme("1ab://x"), None);
        assert_eq!(link_scheme(":x"), None);
        assert_eq!(link_scheme("a b:c"), None);
    }

    /// UC-004 BR-007, Q-61: files the system would run, by their ending,
    /// whatever its case.
    #[test]
    fn uc004_br007_q61_executable_endings() {
        let cases = [
            ("start.desktop", "a desktop launcher"),
            ("run.BAT", "a batch file"),
            ("run.cmd", "a batch file"),
            ("Mail.lnk", "a Windows shortcut"),
            ("setup.exe", "a Windows program"),
            ("setup.msi", "a Windows installer"),
            ("old.com", "a Windows program"),
            ("saver.scr", "a screen saver program"),
            ("do.ps1", "a PowerShell script"),
            ("do.vbs", "a Windows script"),
            ("tool.jar", "a Java program"),
            ("Tool.app", "a macOS app"),
            ("deploy.sh", "a shell script"),
        ];
        for (name, kind) in cases {
            assert_eq!(executable_kind(Path::new(name)), Some(kind), "{name}");
        }
        for name in [
            "report.pdf",
            "notes.md",
            "README",
            "archive.tar.gz",
            "x.shx",
        ] {
            assert_eq!(executable_kind(Path::new(name)), None, "{name}");
        }
    }

    /// UC-004 BR-007, Q-61: on Unix a file with an execute permission is a
    /// program; a folder (whose permission lets it be entered) is not, and
    /// a link counts as what it leads to.
    #[cfg(unix)]
    #[test]
    fn uc004_br007_q61_execute_permission_makes_a_program() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("underline-opening-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let program = dir.join("tool");
        fs::write(&program, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
        let plain = dir.join("data");
        fs::write(&plain, "x").unwrap();
        fs::set_permissions(&plain, fs::Permissions::from_mode(0o644)).unwrap();
        let disguised = dir.join("photo.png");
        std::os::unix::fs::symlink(&program, &disguised).unwrap();
        let launcher = dir.join("launcher.desktop");
        fs::write(&launcher, "[Desktop Entry]\n").unwrap();
        let innocent = dir.join("readme.txt");
        std::os::unix::fs::symlink(&launcher, &innocent).unwrap();

        assert_eq!(executable_kind(&program), Some("a program"));
        assert_eq!(executable_kind(&plain), None);
        assert_eq!(executable_kind(&dir), None);
        assert_eq!(executable_kind(&disguised), Some("a program"));
        assert_eq!(executable_kind(&innocent), Some("a desktop launcher"));
        fs::remove_dir_all(&dir).unwrap();
    }

    /// UC-004 BR-007, Q-61 (monkey finding): endings the system runs or
    /// launches, which the list lacks: Windows Script Host files, internet
    /// shortcuts, saved consoles, ClickOnce and macOS launchers.
    #[test]
    fn uc004_br007_q61_monkey_more_launchable_endings() {
        let missing: Vec<_> = [
            "x.js",
            "x.jse",
            "x.vbe",
            "x.wsh",
            "x.url",
            "x.msc",
            "x.application",
            "x.msp",
            "x.inf",
            "x.scf",
            "x.terminal",
            "x.workflow",
        ]
        .into_iter()
        .filter(|name| executable_kind(Path::new(name)).is_none())
        .collect();
        assert!(missing.is_empty(), "opened without asking: {missing:?}");
    }

    /// UC-004 BR-007, Q-61 (monkey finding): Windows ignores trailing dots
    /// and spaces and a `:stream` after a name, so `a.exe.`, `a.exe ` (a
    /// link `a.exe%20` decodes to it) and `a.exe::$DATA` run `a.exe`.
    #[test]
    fn uc004_br007_q61_monkey_windows_name_tricks_do_not_hide_a_program() {
        for name in ["a.exe.", "a.exe ", "a.exe. .", "a.exe::$DATA", "a.exe:x"] {
            assert!(
                executable_kind(Path::new(name)).is_some(),
                "{name:?} is no program"
            );
        }
    }

    /// UC-004 BR-007, Q-61 (monkey finding): the doc says `None` "also for
    /// a folder", but a folder named like a program ending is a program:
    /// `example.com`, `release.sh`, `x.exe` ask "Open program".
    #[cfg(unix)]
    #[test]
    fn uc004_br007_q61_monkey_a_folder_is_no_program_whatever_its_name() {
        use std::fs;
        let dir = std::env::temp_dir().join(format!("underline-monkey9-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for name in ["example.com", "release.sh", "x.exe"] {
            fs::create_dir_all(dir.join(name)).unwrap();
            assert_eq!(executable_kind(&dir.join(name)), None, "{name}");
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    /// UC-004 BR-007, Q-61: places on another computer or a device are
    /// told by their text alone, before anything looks at them.
    #[test]
    fn uc004_br007_q61_remote_places_by_their_text() {
        for place in [
            r"\\server\share\a.pdf",
            "//server/share/a.pdf",
            r"\\?\C:\a.exe",
            r"\\.\PhysicalDrive0",
            r"/\server\x",
        ] {
            assert!(is_remote_place(place), "{place}");
        }
        for place in ["/home/me/a.pdf", r"C:\x", "docs/a.pdf", "", "/"] {
            assert!(!is_remote_place(place), "{place}");
        }
        assert_eq!(
            executable_by_name(Path::new(r"\\server\share\setup.exe")),
            Some("a Windows program")
        );
    }

    /// UC-004 BR-007, Q-62: an unreadable or empty scheme is still named,
    /// with what is invisible or not ASCII written out.
    #[test]
    fn uc004_br007_q62_the_scheme_as_shown() {
        assert_eq!(shown_scheme("ftp://x"), "ftp");
        assert_eq!(shown_scheme("\u{200b}http://x"), r"\u{200b}http");
        assert_eq!(shown_scheme("һttp://x"), r"\u{4bb}ttp");
        assert_eq!(shown_scheme("://x"), "?");
    }

    /// UC-004 BR-007, Q-61: a name cannot hide its ending behind a change
    /// of direction or a control character.
    #[test]
    fn uc004_br007_q61_hidden_characters_are_written_out() {
        assert_eq!(shown_text("evil\u{202e}fdp.exe"), r"evil\u{202e}fdp.exe");
        assert_eq!(shown_text("a\nb\u{feff}"), r"a\u{a}b\u{feff}");
        assert_eq!(shown_text("Bericht für Zoë.pdf"), "Bericht für Zoë.pdf");
    }

    /// UC-004 BR-007, Q-61: a macOS app is a folder, and still asks.
    #[cfg(unix)]
    #[test]
    fn uc004_br007_q61_a_macos_app_folder_asks() {
        let dir = std::env::temp_dir().join(format!("underline-app-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Tool.app")).unwrap();
        assert_eq!(executable_kind(&dir.join("Tool.app")), Some("a macOS app"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
