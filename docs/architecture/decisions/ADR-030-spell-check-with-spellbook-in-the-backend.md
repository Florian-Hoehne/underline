# ADR-030: Check spelling with `spellbook` and Hunspell dictionaries in the backend, marks sent as events

**Status:** Accepted
**Date:** 2026-11-16
**Requirements:** NFR-002, NFR-013, NFR-014, NFR-015, C-006, C-008, FR-099, FR-100

## Context

Misspelled words are marked while the user types, in the source view and in the facade. The user gets up to five
suggestions and "Add to dictionary". The language is chosen per user and per workspace, or switched off (FR-099,
FR-100; UC-005 BR-023 and A28, UC-010 BR-001; OQ-206 to OQ-208). Some text is not checked: code, link and web
addresses, diagrams, formulas, the names of note attributes, tags, words holding digits, and words written only in
capitals.

The constraints are:

- Typing stays within 16 ms and a mark appears within 1 s (NFR-002).
- The check behaves the same on every platform (NFR-013).
- It works offline, with the dictionaries coming with Underline (NFR-014, NFR-015).
- Settings live in `underline.yaml` (C-006).
- The project is MIT (C-008).
- The webview loads nothing from the network (ADR-023).

The text of an open note lives in the webview; the backend keeps a mirror and already computes highlighting there,
sending it as events tagged with a revision (ADR-025).

The webviews' own spell checkers differ by platform:

- WebKitGTK uses enchant and the system's dictionaries.
- WebView2 uses the Windows checker and cannot be given a language.
- WKWebView uses NSSpellChecker.

None of them lets the app skip markup reliably, keep its own dictionary, or read the marks.

## Decision

### Checker

- `spellbook` 0.4.2 (MPL-2.0) is the checker: helix-editor's pure-Rust rewrite of Nuspell, reading Hunspell `.aff` and
  `.dic` files. It is `no_std`, and its only dependency, `hashbrown`, is already in the build.
- It sits behind a `SpellChecker` trait in a new library crate, `underline-spell`. That crate holds:
  - the tokenizer, which uses `unicode-segmentation` as ADR-027 does;
  - the skip rules of BR-023, taken from the blocks and inline runs of `underline-markup`;
  - a dictionary cache keyed by language;
  - the user's words.

### Where it runs

- The checker runs in the backend, on the mirror (`underline-session`).
- After an edit is confirmed, the changed blocks are checked after about 250 ms. On opening, a note is checked in
  chunks, the visible part first.
- The marks go to the webview as an event tagged with the revision. The webview maps them through the edits made since
  then, as it does for highlighting.
  - The source view shows them as CodeMirror mark decorations.
  - The facade renders them as spans with a wavy underline in the theme's colour.
- The webview's own spell check is switched off in both views (`spellcheck="false"`).
- Suggestions come from a command that runs when the context menu opens. The menu opens at once with "Add to
  dictionary", and the suggestions fill in when they arrive.
- Nothing runs on the key path.

### Dictionaries

- Dictionaries are shipped as Tauri resource files, one folder per language, each with its license text.
- They are never compiled into the binary and never converted during the build. Latin-1 files are decoded when they
  are loaded.
- A language is loaded on a background thread the first time it is needed.
- The first bundle holds only permissively licensed dictionaries:

  | Language | Source | License |
  |---|---|---|
  | en-US | SCOWL / wordlist 2020.12.07 | SCOWL permissive, BSD |
  | en-GB | SCOWL `en_GB-ise` | SCOWL permissive, BSD |
  | fr-FR | Grammalecte / dicollecte 7.7 | MPL-2.0 |
  | es-ES | RLA-ES | MPL-1.1, chosen from its triple license |

- German (de-DE, de-CH; igerman98 / frami) and Italian are licensed GPL-2.0 or GPL-3.0. Whether they ship is a
  decision for the user (open question).
  - The common reading is that unmodified, separate data files shipped next to an MIT program are "mere aggregation"
    (GPLv2 §2, GPLv3 §5). LibreOffice and VS Code's cSpell ship them this way.
  - The obligations would be to ship the GPL texts and to offer the corresponding source of the dictionaries.
  - The bundle as a whole would then be "MIT and others".

### User's words

- The user's words are a plain UTF-8 file next to the user's settings, one word per line, sorted.
- It is written atomically (ADR-005) and followed by the watcher (ADR-011).
- A workspace dictionary (OQ-208) would be the same kind of file in the workspace, committed with it. Adding it changes
  FR-099 and A28 and is left to the user.

### Setting

- `spell_check_language` takes `off` or a language code, in both settings layers (ADR-004).

### Update 2026-11-16

The user decided OQ-299, OQ-208 and OQ-207 (decisions of 2026-11-16):

- **German and Italian ship (OQ-299).** `de-DE` and `de-CH` (igerman98 / frami, GPL-2.0 or GPL-3.0) and `it-IT`
  (LibreItalia, GPL-3.0) come from the LibreOffice dictionaries repository at a pinned commit, each in a resource folder
  of its own, unmodified (only the `.aff` and `.dic` are renamed to the language code).
  - *License reasoning:* the dictionaries are data files the program reads at run time, never compiled in, linked or
    converted. Shipped unmodified and separately next to the MIT program, they are "mere aggregation" (GPLv2 §2,
    GPLv3 §5), as LibreOffice and VS Code's cSpell ship them; Underline's code stays MIT.
  - *Obligations:* each folder holds the GPL texts and the README of its dictionary; `SOURCES.md` records version,
    commit and sha256 and offers the corresponding source (the LibreOffice repository at the commit, the igerman98
    tarball); `THIRD-PARTY-NOTICES.md` at the root of the repository names every dictionary with its license and is
    bundled as a resource; the installers' license reads "MIT and others". Changing a GPL dictionary would oblige
    Underline to publish the change under the GPL, so they are never changed.
  - The system-language default maps `de_CH` (and `de_LI`) to German (Switzerland), other German to German (Germany),
    and Italian of any region to Italian.
- **Workspace dictionary (OQ-208).** `underline-words.txt` in the root of the workspace, the same format as the user's
  file, committed with the workspace. It is followed like the user's file (time and size polled every 500 ms, read on
  the check thread), written atomically after reading it first, so hand edits stay, and never written over when it
  cannot be read. The words in effect are the dictionary's, the user's and the workspace's. The spell menu gains
  "Add to workspace dictionary" (command `spell_add_workspace_word`).
  - The workspace file is untrusted input (a cloned repository): no part of its path below the workspace root may be
    a symbolic link (`underline_core::link_below`, shared with the snapshots), only a regular file is read, and every
    words file, the user's too, is read through `take(2 MiB + 1)`, so a pipe, a device or a `/proc` file never blocks
    or fills the check thread. A linked workspace file is neither read nor written; adding a word then refuses and
    says why.
  - Adding a word re-reads the file under a lock file next to it (`underline-words.txt.lock`, `create_new`, waiting
    up to 3 s, a lock older than 10 s taken over) and writes the union, so two windows adding at once lose no word.
  - A workspace in the folder of the user settings file has one file for both, read once, as the user's.
- **A note's language (OQ-207).** The note attribute `lang` (front matter `lang: de-CH`, AsciiDoc `:lang: de-CH`; `-`
  or `_`, any case) overrides the language of the settings for that note. The check thread reads it from the head of
  the text it checks (at most 64 KiB, with `underline-markup::attributes`), so the marks follow a change of it; the
  suggestions command takes the note to use the same language. A language not offered marks nothing and the status
  bar names the note ("{title}: no dictionary for …") while that note is active. The value of `lang` is not checked,
  and the setting `off` wins over the note. `lang` is read as `SpellCheckLanguage::of_tag` (also behind the system
  default): region and variant subtags (`de-CH-1996`), `de` and `it` alone, `gsw`; no value, `null`, `~` and `off`
  leave the settings; of AsciiDoc entries set twice the last wins; a language not offered is shown cut to 35
  characters on one line.
- **Typographic apostrophe.** A word with `’` the dictionary does not know is checked again with `'`, in every
  language; its corrections keep the `’`.
- **Memory of dictionaries.** A tab keeps only the speller of the language it is checked in now. Every 60 s the
  dictionaries neither the setting nor an open note uses are let go (`Dictionaries::retain`), and so are loads that
  failed, which are tried again when needed.

## Consequences

- **Same on every platform:** the marks, the suggestions and the user's words behave the same everywhere. The skip rules
  are tested once, natively.
- **Measured** (native, release, LibreOffice dictionaries):

  | Language | Load | Heap | Check per word | Suggestions, mean | Suggestions, max |
  |---|---|---|---|---|---|
  | en-US | 10–27 ms | 2.1 MiB | about 0.4 µs | 15 ms | 22 ms |
  | fr-FR | 18 ms | 4.8 MiB | 3.3 µs | 100 ms | 160 ms |
  | de-DE (if shipped) | 60 ms | 17.8 MiB | 11 µs | 130 ms | 465 ms |

  German suggestions are why suggestions are asynchronous.
- **Size:** the backend grows by about 0.2–0.3 MB of code. The installers grow by the dictionaries: about 1.2 MB gzip
  for the four permissive languages, and about 2.5 MB more with German and Italian.
- **spellbook is young** (alpha, mostly one author). The trait and golden tests per language keep a switch open:
  `hunspell-rs` in the backend is the fallback.
- **Dictionaries are old:** SCOWL is from 2020, and the user's words absorb what is missing.
- **Large notes:** a check parses and copies the whole note on the check thread. For notes over 1 MB the pause before a
  check is 1 s instead of 250 ms, and a check comes at the latest after 3 s instead of 700 ms.
- **Faults:** each check runs under `catch_unwind`; a lost check thread is started again, and after three restarts the
  spell check stops and the status bar says so.
- **The user's words:** a file that cannot be read (not UTF-8, no permission, over 2 MB) is never written over. Two
  windows adding a word at the same moment both write atomically; the file is never corrupted, but one window's word
  may be missing until it is added again.
- **German depends on a license decision.** Without it, German, which is the user's language, is not offered.

### Update 2026-11-16 (consequences)

- German and Italian are offered. The installers grow by about 2.8 MB compressed (the three `.dic` files are 4.4, 4.4
  and 1.3 MB, about 1.2, 1.2 and 0.35 MB gzip); the source tree by about 10 MB.
- German loads in about 60 ms and takes about 18 MiB while in use, as measured above; only the languages in use are
  loaded.
- Every release must keep the GPL texts, the READMEs, `SOURCES.md` and `THIRD-PARTY-NOTICES.md` in the bundle, and a
  new dictionary version means a new pinned commit and new checksums in `SOURCES.md`.
- A team shares its project words through git; a word one member adds shows as a change of `underline-words.txt`.

### Alternatives considered

- **The webviews' own spell check:** it differs per platform and cannot skip markup, keep the app's words, or take a
  language. This is against NFR-013 and BR-023.
- **The checker in the WASM editor core or in a Web Worker:**
  - The dictionaries (up to 20 MiB for German) would sit in the UI process.
  - German suggestions would block for up to 0.7 s.
  - The skip rules would be duplicated outside the mirror.
- **`zspell`:** its license is non-standard, it has no compounding (so German fails), and its suggestions are unfinished.
- **`hunspell-rs`:** it needs the C++ Hunspell built on all three platforms. It is the fallback only.
- **JavaScript checkers** (nspell, Typo.js, cspell):
  - They put domain logic into the frontend.
  - nspell and Typo.js are slow with German compounds.
  - cspell's German dictionary is GPL as well.
- **`harper-core`, `nlprule`, `symspell`:** they are grammar checkers or suggestion accelerators. None covers German
  compounding.
- **The system's Hunspell dictionaries:** they are not available on Windows or macOS, and on Linux they depend on the
  distribution.
