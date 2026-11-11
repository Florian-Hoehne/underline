# Feature-Inventar: egui-App → Tauri 2

Checkliste für die Feature-Parität der Migration. Grundlage ist der Code auf `main` bei Commit `af807f6`
(`apps/underline-desktop`, `crates/underline-editor`), abgeglichen mit den Use Cases `docs/use_cases/UC-001` bis
`UC-021`. Jeder Punkt beschreibt das **heutige** Verhalten. Die Tauri-App muss es übernehmen. Abweichungen kommen nach
`docs/migration/paritaet.md`.

- Status: `[ ]` offen, `[x]` migriert und von app-tester geprüft, `[~]` migriert mit dokumentierter Abweichung.
- IDs `M-xx.y` bleiben stabil. Commits und Tests nennen die ID zusätzlich zur UC-ID.
- Code-Verweise: `app` = `apps/underline-desktop/src/app.rs`, `sv` = `crates/underline-editor/src/source_view.rs`,
  `fv` = `crates/underline-editor/src/facade_view.rs`, `keys` = `crates/underline-editor/src/keys.rs`.
- UI-Texte stehen wörtlich (Englisch) da und werden 1:1 übernommen.

Allgemeine Konventionen der heutigen UI:

- Alle Dialoge sind nicht-modale Fenster. Mehrere können gleichzeitig offen sein, keiner blockiert die App.
- Es gibt **eine** Statusmeldung (`status`). Angebote wie Undo, Open oder Show sind nur sichtbar, solange die Meldung
  unverändert ist, zu der sie gehören.
- Shortcut-Beschriftungen zeigen auf macOS „Cmd“, sonst „Ctrl“.

---

## M-01 Start und Kommandozeile

- [x] **M-01.1** `underline [DIR]` öffnet das Fenster mit DIR. Ohne DIR öffnet es den zuletzt offenen Workspace,
  sonst den leeren Bildschirm (M-02). Ausgewertet wird nur das erste Positionsargument, es gibt kein `--help`
  (`main.rs:18-56`).
- [x] **M-01.2** `underline mcp [DIR]` startet den MCP-Server über stdio, ohne Fenster. Ohne DIR nimmt er
  `~/Underline` ($HOME, sonst %USERPROFILE%, sonst `.`). Der Server folgt dem gespeicherten Agent-Zugriff
  (UC-017 BR-007). **Wichtig:** MCP-Install trägt den Pfad der laufenden Binary (`current_exe`) in die
  Agent-Konfigurationen ein, also muss auch die Tauri-Binary `mcp` verstehen.
- [x] **M-01.3** App-State: `<data_dir>/underline/state.yaml`. Ist kein Pfad verfügbar, liegt der State nur im
  Speicher. Ein Lade-Hinweis erscheint als Status „Note: {notice}“.
- [x] **M-01.4** Fenstertitel ist immer „Underline“. Grösse ist Standard, ohne Mindestgrösse und ohne Icon.
  Fenstergeometrie wird nicht gespeichert.
- [x] **M-01.5** Schriften: DejaVu Sans (Facade, 15 pt) und Monospace 14 pt (Quelle, Code) sind eingebettet
  (`underline_editor::fonts`).
- [x] **M-01.6** Theme: die `Palette` des Themes bestimmt alle UI-Farben (`app:4934-4952`): Panel, Hintergrund,
  Code-Hintergrund, Link, Text, Auswahl (`chosen`/`chosen_text`) und Buttons. Es gibt 11 Themes (`Theme::ALL`).

## M-02 Leerer Bildschirm (UC-002 A2)

- [x] **M-02.1** Bei einem Drittel der Höhe, zentriert: Überschrift „No workspace is open“, darunter die Buttons
  „Open workspace…“ und „New workspace…“. Top-Bar und Statusleiste bleiben sichtbar.

## M-03 Top-Bar und Menü

- [x] **M-03.1** Burger-Button (a11y „Menu“, Hover „Hide the menu“/„Show the menu“) schaltet die Menüleiste um.
  Die Einstellung `show_menu_bar` wird sofort gespeichert.
- [~] **M-03.2** Lupe (a11y „Search“, Hover „Search notes  Ctrl+Shift+F“) schaltet das Such-Overlay um. Öffnen
  fokussiert das Feld, Schliessen leert den Text.
- [x] **M-03.3** Ist die Menüleiste ausgeblendet, zeigt die Top-Bar den Ordnernamen des Workspace fett und
  gekürzt, ohne Workspace „No workspace open“.
- [x] **M-03.4** Menü **File**: „New note…“ (ohne Workspace deaktiviert), „Open workspace…“ (schliesst den
  Create-Dialog), „New workspace…“ (schliesst den Open-Dialog).
- [x] **M-03.5** Menü **Workspace**: Feld „Wiki project URL“ mit „Check“ oder Enter. Danach Status „Wiki repository:
  {clone_url}“ oder der Fehler, das Menü schliesst. *Stub, nirgends angebunden und nicht gespeichert.*
- [x] **M-03.6** Menü **Theme**: Radio-Liste aller Themes. Die Wahl wird sofort gespeichert, das Menü schliesst.
- [~] **M-03.7** Menü **Settings**: „Settings…“ mit Shortcut-Text „Ctrl+,“ schaltet das Einstellungsfenster um.

## M-04 Statusleiste (FR-020, FR-021)

Reihenfolge von links nach rechts:

- [ ] **M-04.1** Branch-Button (nur wenn die Änderungen lesbar sind): „Branch {b}“ oder „Branch {b} · N change(s)“.
  `{b}` ist der Name, „{name} (no commit yet)“ oder „not on a branch: {commit}“. Hover „Branches  Ctrl+Shift+B“,
  Klick öffnet das Branch-Menü. Bei einem Fehler steht schwach „Changes cannot be shown: {e}“.
- [ ] **M-04.2** Konflikte: orange „1 conflict“/„N conflicts“ und ein kleiner Button „Abort the sync“.
- [ ] **M-04.3** Sync-Zustand (nur mit Syncer):
  - „Syncing…“
  - „Not synced yet“
  - „Synced just now / N min ago / N h ago“
  - „Offline, will retry (synced …)“
  - „Sync failed“ (Warnung)
  - „Sync failed: signing in“ (Warnung)
  - „Sync stopped: conflict“ (Warnung)
  - „Sync stopped“ (Warnung)
  - „Push refused“ (Warnung)
  - „No sync“

  Die Begründung bzw. die Dateien stehen im Hover. Danach folgen „N to push“, „N to pull“ und der Button „Sync now“,
  deaktiviert während eines Syncs oder bei offenen Konflikten.
- [ ] **M-04.4** Agents-Menü (M-17.1).
- [ ] **M-04.5** Rot „1 todo overdue“/„N todos overdue“ (Hover „Open the todo list“) öffnet den Todos-Tab mit Filter
  Open.
- [ ] **M-04.6** Undo für einen abgehakten Todo („Todo {name} is as before“) und für einen gelöschten Todo
  („Todo {name} is back“). Beide stehen *vor* dem Text.
- [x] **M-04.7** Text: Workspace-Pfad und Status, getrennt durch „  |  “.
- [~] **M-04.8** Nach einem Export: „Open“ (Systemprogramm) und „Show“ (Fenster „Approximated blocks“).
- [ ] **M-04.9** Nach Agent-Änderungen: bis zu 3 Buttons „Open“/„Open {title}“, die neueste zuerst.
- [ ] **M-04.10** Undo nach dem Löschen einer Notiz. Das Angebot verschwindet, sobald sich der Status ändert.

## M-05 Notizübersicht links (UC-002, FR-017)

- [x] **M-05.1** Panel in der Breite veränderbar (100–1000, Start aus `overview_width`). Beim Loslassen nach mehr
  als 1 px Änderung wird die Breite sofort gespeichert. Kopf: „Notes“ und Button „New note…“.
- [x] **M-05.2** Liste aus `note_overview`: zuletzt geöffnete zuerst, dann der Rest nach Titel A–Z (BR-008). Ein
  Klick öffnet die Notiz, die dabei nach oben wandert.
- [~] **M-05.3** Git-Buchstabe hinter dem Titel: N grün, D rot, R blau, C orange, sonst M amber. Tabs mit
  ungespeicherten Änderungen zeigen M, aber nur in Git-Workspaces.
- [~] **M-05.4** Kontextmenü: „Rename…“, „Delete…“ und Untermenü „Export“ mit „{kind}…“ für jede Exportart. Das
  Untermenü überspringt die Auswahl der Art.
- [~] **M-05.5** Die Taste Delete löscht die zuletzt angeklickte Notiz (UC-007 BR-007). Ein Klick oder Fokus
  ausserhalb hebt die Wahl auf. *Keine Pfeiltasten-Navigation und kein Leertext.*
- [~] **M-05.6** „Deleted notes (N)“ ist standardmässig zugeklappt. Je Eintrag: Titel (Hover „{folder} · deleted
  {local time}“), Zeit, „Restore“ und „Remove“ (mit Rückfrage). Dazu „Empty the deleted notes“. Beim Öffnen des
  Workspace wird alles gelöscht, was älter als 30 Tage ist.

## M-06 Tabs (FR-018)

- [x] **M-06.1** Tab-Leiste mit horizontalem Scroll. Label: Titel, dazu „ ⚠“ bei Speicherfehler, sonst „ •“ wenn
  ungespeichert, und „ (deleted)“ wenn ausserhalb gelöscht. Hover zeigt den Pfad.
- [x] **M-06.2** Ein Klick aktiviert den Tab, merkt „zuletzt geöffnet“ vor, speichert die Tabs und sortiert die
  Übersicht neu.
- [x] **M-06.3** Mittelklick oder „×“ schliesst den Tab. Vorher wird gespeichert; scheitert das, fragt M-21.2.
- [ ] **M-06.4** Kontextmenü: „Rename…  F2“, „Delete…“, „Export…  Ctrl+Shift+E“.
- [ ] **M-06.5** Todos-Tab „Todos (N)“, wenn die Liste offen ist. Schliessen mit „×“ oder Mittelklick. Das Öffnen
  einer Notiz deaktiviert ihn, lässt ihn aber offen.
- [ ] **M-06.6** Höchstens 100 Tabs (`MAX_TABS`). Ein weiterer schliesst den am längsten unbenutzten Tab, Status
  „Closed the tab {t}, used longest ago“. *Speicherfehler werden dabei heute ignoriert (Defekt D-07).*
- [x] **M-06.7** Tabs, aktiver Tab und Ansicht je Tab werden beim Öffnen des Workspace wiederhergestellt. Ohne
  gemerkten Tab wird `first_note` geöffnet.

## M-07 Notiz lesen (UC-004)

- [ ] **M-07.1** Ohne Tab steht „Select or create a note.“.
- [x] **M-07.2** Die Werkzeugleiste über dem Editor hat den Umschalter „Facade“/„Source“ (FR-010). Der Umschalter
  speichert die Tabs und fokussiert den Editor. Cursor und Auswahl bleiben beim Wechsel erhalten (UC-005 A1).
- [~] **M-07.3** Links folgen (FR-040, `app:2463-2523`) mit Ctrl/Cmd+Klick, in beiden Ansichten. Ziel:
  - Überschrift → Sprung, sonst „The heading X was not found“.
  - Notiz → im Tab öffnen und zur Überschrift springen.
  - Fehlende Notiz → Dialog „New note“ vorbefüllt mit „{title} does not exist yet. Create it?“.
  - Web → einmal pro Sitzung fragen („Open X in your browser?“ / „Underline asks this once until it is closed.“,
    „Open“/„Cancel“), dann im Systembrowser öffnen.
  - Datei → öffnen, falls vorhanden, sonst „X does not exist“.
  - Ausserhalb → „{link} leads outside the workspace and is not followed“.
- [x] **M-07.4** In der Facade zeigt Hover über einem Link die Adresse und „Ctrl+click to follow“.
- [x] **M-07.5** Änderungen von aussen (ADR-011):
  - Ist der Inhalt gleich der Fassung auf Platte oder dem gerade Gespeicherten → ignorieren.
  - Tab ungespeichert oder Speichern läuft → die fremde Fassung als Kopie ablegen (einmal je Inhalt):
    „{t} was changed outside while you were editing; your text stays, the other version is kept as {name}“.
  - Sonst neu laden: „{t} was changed outside“.
  - Gelöscht → Tab „(deleted)“ und „{t} was deleted outside“. Beim nächsten Speichern wird die Datei wieder
    angelegt: „Stored {t} again at its place“.
  - Viele Änderungen auf einmal → Workspace neu einlesen. Watcher im Poll-Modus → einmalig „Changes from outside may
    show up later in the note overview; this system delivers no change events“.

## M-08 Quellansicht (FR-008, ADR-012)

- [ ] **M-08.1** Monospace 14 pt mit Soft-Wrap. Nur sichtbare Zeilen werden gelayoutet (10'000 Zeilen, NFR-002).
- [ ] **M-08.2** Syntax-Hervorhebung von Markdown/AsciiDoc mit syntect (ADR-010). Code in Fences wird in seiner
  Sprache eingefärbt. Noch nicht gefärbte Zeilen erscheinen schlicht und werden nachgefärbt (Budget 4 ms je Frame).
  Die Palette kommt aus dem Theme.
- [x] **M-08.3** Maus: Klick setzt den Cursor, Ziehen oder Shift+Klick erweitert die Auswahl. *Kein Doppel- oder
  Dreifachklick.*
- [x] **M-08.4** Tasten (Normal):
  - Bearbeiten: Backspace, Delete, Enter, Tab (4 Leerzeichen), Ctrl+A.
  - Bewegen: Pfeile (Up/Down überspringen gefaltete Zeilen und halten die Spalte), PageUp/PageDown, Home/End,
    Ctrl+Home/End. Shift erweitert die Auswahl.
  - *Keine Wort-Bewegung und kein wortweises Löschen.*
- [ ] **M-08.5** IME: es wird nur der fertige Text eingefügt, die Komposition wird nicht angezeigt.
- [ ] **M-08.6** Kopieren und Ausschneiden legen das Markup ab. Einfügen fügt immer reinen Text ein. Ein Bild in der
  Zwischenablage wird als PNG neben der Notiz gespeichert und als Bild eingefügt (M-10.6).
- [x] **M-08.7** Zeilennummern (FR-028/029): ein/aus, links oder rechts. Die Breite folgt der Stellenzahl, die Farbe
  ist 45 % der Textfarbe. Nur in der Quellansicht.
- [x] **M-08.8** Falten (FR-027):
  - Der Pfeil im Gutter erscheint bei Hover oder wenn gefaltet. Eine gefaltete Überschrift endet auf „…“.
  - Ctrl/Cmd+Shift+[ faltet, Ctrl/Cmd+Shift+] klappt auf.
  - Steht der Cursor beim Falten in der Sektion, springt er zur Überschrift. Landet der Cursor in einem gefalteten
    Bereich, klappt dieser auf.
  - Der Faltzustand gilt für beide Ansichten.
- [ ] **M-08.9** Cursor blinkt im Takt von egui. Nach jeder Bewegung ist er sofort sichtbar. Er wird nur mit Fokus
  gezeichnet.
- [x] **M-08.10** Tab, Pfeile und (in Vim/Nano) Escape verlassen den Editor nicht über die Fokus-Navigation.

## M-09 Facade (FR-009, FR-011, FR-012, NFR-007)

- [x] **M-09.1** Blockmodell aus `underline-facade` (comrak bzw. eigener AsciiDoc-Parser). Blockarten: Überschrift,
  Absatz, Listenpunkt (Tiefe, Marker, Checkbox), Zitat (Tiefe), Code, Trennlinie, Admonition, Markup, Tabelle, Bild.
- [x] **M-09.2** Darstellung:
  - Schrift: DejaVu Sans 15 pt. Überschriften 26/22/19/17/16 pt fett.
  - Code, Markup und Tabellen-Markup in Monospace 14 pt auf Code-Hintergrund. Inline-Code hat einen Hintergrund.
  - Links unterstrichen, Durchstreichen, fett in der Farbe „strong“.
  - Listenmarker „•“, „n.“, „☑/☐“, eingerückt je Tiefe.
  - Zitat: ein Balken je Tiefe. Admonition: Balken in Linkfarbe mit Label. Trennlinie als Linie.
- [x] **M-09.3** Format-Anzeige „Markdown“/„AsciiDoc“ oben rechts (FR-011).
- [x] **M-09.4** Code-Blöcke werden in ihrer Sprache und in Theme-Farben hervorgehoben (FR-035, FR-038).
- [x] **M-09.5** Tabellen:
  - Spaltenbreiten nach Inhalt, proportional geschrumpft, wenn zu breit.
  - Ausrichtung je Spalte, Kopfzeile fett und schattiert.
  - Beschriftung „Table n.“
- [ ] **M-09.6** Bilder:
  - Geladen im Hintergrund, mit Beschriftung „Figure n.“
  - Meldungen für „lädt“, „nicht gefunden“ und „ausserhalb des Workspace“.
  - Web-Bilder nur nach Zustimmung (ADR-013, M-09.15).
- [ ] **M-09.7** Diagramme (`plantuml`, `puml`, `mermaid`):
  - Offline gerendert (ADR-009), mit Cache.
  - Während eine geänderte Quelle rendert, bleibt das letzte Bild stehen.
  - Bei einem Fehler wird die Quelle mit der Meldung gezeigt.
  - Farben: heute nur hell/dunkel, nicht die Theme-Palette (Defekt D-10).
- [x] **M-09.8** Abbildungen (Diagramm, Bild, Tabelle): Steht der Cursor im Block, erscheint unter der Abbildung ein
  Rahmen mit dem Markup. Dort wird bearbeitet.
- [x] **M-09.9** Vergrössern (FR-039):
  - Öffnen per Doppelklick auf das Bild oder per Lupen-Button bei Hover.
  - Zoomen mit Rad, Pinch, `+`, `=` und `-` um den Zeiger. Ziehen verschiebt, `0` passt ein.
  - Schliessen mit Escape, Klick daneben oder „×“.
  - Beim Hineinzoomen wird schärfer nachgerendert.
- [x] **M-09.10** Bearbeiten ändert die Bytes an Ort und Stelle; kein Block wird neu serialisiert (NFR-007,
  `fv:1561-1600`). Beim Tippen escapt Markdown `\ * _ \` [ ] < > ~`, am Blockanfang auch `# - + |`. Code-, Markup-,
  Bild- und Tabellen-Blöcke nehmen rohen Text.
- [~] **M-09.11** Enter:
  - Code und Markup: `\n`.
  - Liste: `\n` plus der nächste Marker (gleiche Einrückung, nächste Nummer, offene Checkbox).
  - Markdown-Zitat: `\n\n> `.
  - Sonst: `\n\n`. Danach steht der Cursor in einer leeren Lücke: Enter fügt eine Zeile hinzu, Backspace nimmt eine
    weg.
- [~] **M-09.12** Backspace/Delete:
  - Löschen ein sichtbares Zeichen samt Escape-Backslash.
  - Am Blockrand wird mit dem Nachbarblock verbunden.
  - Eine Trennlinie wird als Ganzes entfernt.
- [~] **M-09.13** Tasten:
  - Left/Right gehen je sichtbares Zeichen, auch über Blockgrenzen.
  - Up/Down gehen je Zeile, dann in den Nachbarblock. Home/End gehen an Zeilenanfang/-ende.
  - *Kein PageUp/PageDown, kein Ctrl+Home/End, Tab tut nichts.*
- [~] **M-09.14** Einfügen:
  - HTML wird zu Markup (`html_to_markup`). Ctrl+Shift+V fügt reinen Text ein, escaped.
  - Underlines eigene Kopien kommen als Markup zurück.
  - Mehrzeiliges Markup bekommt Leerzeilen davor und danach.
  - Kopieren legt Markup als Text plus HTML ab.
- [x] **M-09.15** Web-Bilder sind der einzige Netzzugriff des Editors (ADR-013), mit Zustimmung. Die Webview darf
  keine Fremd-URLs selbst laden.
- [ ] **M-09.16** `reveal(offset)` scrollt einen Block nach oben, ohne den Cursor zu bewegen (Sprünge aus dem
  Werkzeugpanel).

## M-10 Bearbeiten, beide Ansichten (UC-005)

- [x] **M-10.1** Undo/Redo (BR-004):
  - Tasten: Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z, Ctrl/Cmd+Y.
  - Zusammenfassen: Zeichen eines Wortes werden ein Schritt. Ein Leerzeichen nach einem Wort beginnt einen neuen.
    Backspaces in Folge sind ein Schritt. Alles mit `\n` ist ein eigener Schritt.
  - Agent-Änderungen sind ein eigener, versiegelter Schritt. Neu laden löscht die History.
  - Nach Undo steht der Cursor am Ende des wiederhergestellten Texts.
  - Die History gilt über den Ansichtswechsel hinweg.
- [ ] **M-10.2** Formatierungs-Toolbar (FR-023), ein Button je Tool mit Hover-Hinweis:
  - Bold Ctrl+B, Italic Ctrl+I, Code Ctrl+E, Link Ctrl+K.
  - Heading Ctrl+Shift+H, List Ctrl+Shift+L, Quote Ctrl+Shift+Q, Image Ctrl+Shift+I, Table Ctrl+Shift+T.
  - Inline-Formate schalten um (ein zweites Mal entfernt das Format). Zeilenformate setzen Präfixe, je Format die
    passenden Marker.
- [~] **M-10.3** Dialog Link:
  - Feld „Web address:“ (fokussiert) und Filter „Or a note of the workspace:“ mit bis zu 50 Treffern
    (Gross-/Kleinschreibung egal, max. 160 px hoch).
  - Eine gewählte Notiz wird als relativer Link mit ihrem Titel eingefügt. „Insert“/Enter fügt die Adresse ohne
    Label ein.
  - *Kein Escape.*
- [~] **M-10.4** Dialog Bild:
  - Pfad eintippen unter „Image file:“, dann „Insert“/Enter.
  - Dateien über 10 MB → Rückfrage „{file} has N MB. Copy it into the workspace, or only link to where it is (it is
    then not part of the workspace and not pushed with git)?“ mit „Copy“, „Link to its place“, „Cancel“.
  - Ist der Name belegt → „Copied as {place}, since the name was taken“.
  - Andere Dateien als Bilder werden ein Link mit dem Dateinamen.
  - *Kein Dateidialog.*
- [~] **M-10.5** Drag & Drop von Dateien (FR-033):
  - Während des Ziehens: Status „Drop to insert at the place under the pointer“.
  - Der Cursor geht an die Zeigerstelle. Die Datei wird kopiert (Ordner der Notiz) oder, wenn sie schon im Workspace
    liegt, verlinkt.
  - Mehrere Elemente werden durch Leerzeichen getrennt. Im Code-Block wird nur der Pfad eingefügt.
  - Ohne Pfad → „This cannot be dropped into a note“.
  - *Links aus dem Browser fallen heute nicht darunter (FR-034 offen, Q-07).*
- [~] **M-10.6** Ein eingefügtes Bild wird als PNG neben der Notiz gespeichert: „Pasted image stored as {name}“.
- [~] **M-10.7** Copy on select (FR-031/032):
  - Nach dem Loslassen der Maus wird die Auswahl kopiert; der alte Inhalt der Zwischenablage wird gemerkt.
  - Einfügen über genau dieser Auswahl fügt den alten Inhalt ein.
  - Abschaltbar.
- [x] **M-10.8** Autosave (FR-044, NFR-005):
  - Gespeichert wird 1 s nach der letzten Änderung, spätestens nach 2 s. Neuer Versuch nach 10 s.
  - Atomar (ADR-005), im Hintergrund.
  - Fehler → rot „Not stored: {err}. Underline keeps trying.“ über dem Editor und „ ⚠“ im Tab.
- [x] **M-10.9** Befehle aus den Eingabemodi: `Store` (Vim `:w`, Nano Ctrl+O/Ctrl+S) speichert sofort, mit „Stored
  {t}“ oder „… could not be stored: …“. `Close` schliesst den Tab.

## M-11 Eingabemodi (FR-024/025/026)

- [x] **M-11.1** Normal: siehe M-08.4 und M-09.13.
- [~] **M-11.2** Vim (`keys:175-1060`), heute in beiden Ansichten gleich (in der Facade ist eine „Zeile“ ein
  Block). *Laut Entscheidung F-2 übernimmt `codemirror-vim` den Vim-Modus der Quellansicht. Das ist eine gewollte
  Abweichung: Dort ist die Liste unten eine Untergrenze, in der Facade gilt sie genau.*
  - Modi: NORMAL, INSERT, VISUAL, VISUAL LINE, COMMAND.
  - Zähler für Bewegungen, Operatoren, `x`, `p`, `u`, `gg` und `G`.
  - Bewegungen: `h l Space j k w W b B e E 0 ^ $ gg G nG`, Pfeile.
  - Operatoren `d c y` mit Bewegung; `dd cc yy`; `iw aw`; `D C Y x X`. `cw` wirkt wie `ce`.
  - Einfügen: `i a I A o O`. Escape geht ein Zeichen nach links.
  - Weitere: `r<c>`, `p P` zeichen- oder zeilenweise, Register intern oder Systemzwischenablage mit `"+`/`"*`.
  - Visuell: `d c y x`.
  - Undo `u`, Redo Ctrl+R.
  - Suche: `/pat` mit Umlauf, `n`/`N`. Die Markierung endet mit Escape oder `:noh`/`:nohl`/`:nohlsearch`.
  - Befehle: `:w`, `:q`/`:q!`, `:wq`/`:x`.
  - Ctrl+C wird durchgereicht.
  - *Bewusst fehlend: `. f t % ? * J ~`, Marks, Makros, `:s`.*
- [x] **M-11.3** Nano (`keys:1064-1232`):
  - Ctrl+K schneidet die Zeile aus (aufeinanderfolgende sammeln sich), Ctrl+U fügt ein.
  - Ctrl+W öffnet den Such-Prompt: Enter sucht vorwärts mit Umlauf, Escape oder Ctrl+C bricht ab.
  - Undo Alt+U, Redo Alt+E.
  - Ctrl+O und Ctrl+S speichern, Ctrl+X schliesst.
  - Ctrl+A und Ctrl+E gehen an Zeilenanfang/-ende.
  - Alles andere wirkt wie Normal. Auf macOS gilt Cmd wie Ctrl.
- [x] **M-11.4** Statuszeile des Editors (FR-026, `app:1136-1305`):
  - Links:
    - Modus-Badge in Theme-Farbe: „{keymap} · {mode}“, unter 450 px nur der Modus.
    - Wartende Tasten.
    - Hinweis-Paare; sie fallen von hinten weg, wenn sie nicht passen.
  - Rechts:
    - Umschalter „Copy on select“/„Copy“ und „Minimap“/„Map“.
    - Auswahl „Keys: {mode}“.
    - Positionsbox „Ln 3, Col 5 · 4 chars“ bzw. „· 3 lines“.
    - Unter 720 px kurze Labels.
  - Jeder Umschalter wird sofort gespeichert.

## M-12 Minimap (FR-030)

- [x] **M-12.1** Streifen von 80 px mit 4 px Abstand, unter 240 px Breite ausgeblendet. Massstab höchstens 0,2.
- [x] **M-12.2** In der Quellansicht ein Balken je Zeile, Überschriften kräftiger. In der Facade ein Balken je
  Layoutzeile, Abbildungen als Fläche. Neuaufbau höchstens alle 150 ms.
- [x] **M-12.3** Der sichtbare Bereich ist getönt. Klick oder Ziehen zentriert die Ansicht auf diese Stelle.
- [~] **M-12.4** Sieb (UC-008): Gibt es Treffer, wird der Streifen abgedunkelt (schwarz, Alpha 150). Jeder Treffer
  leuchtet an Zeile und Spalte. Treffer blinken 0,6 s an, 0,4 s aus; das Blinken ist per Einstellung abschaltbar.

## M-13 Suche (UC-008)

- [~] **M-13.1** Such-Overlay: Ctrl+Shift+F oder die Lupe. Volltextsuche im Hintergrund (`Searcher`), mit
  „Searching…“. Ein Ergebnis gilt nur, wenn es zum aktuellen Text passt. Neu gesucht wird, wenn sich Text,
  Übersicht oder Tab-Revisionen ändern.
- [~] **M-13.2** Treffer werden in allen offenen Tabs markiert (Hintergrund `found_background` plus 2 px
  Unterstrich `found`), auch in der Minimap (M-12.4).
- [~] **M-13.3** Ein Klick schliesst das Overlay (ausser auf der Lupe) und öffnet die Notiz an der Fundstelle.
  *Feinverhalten des Overlays ist im Commit `af807f6` neu; vor F7 gegen `app` und UC-008 nachprüfen.*
- [~] **M-13.4** Suchleiste in der Notiz:
  - Öffnen mit Ctrl+F, nur im Modus Normal mit aktivem Tab. Vorbefüllt mit einer einzeiligen Auswahl, sonst mit
    dem letzten Text.
  - Aufbau: „Find in note:“, Feld 200 px, „{i} of {n}“/„{n} found“, „Previous“, „Next“, „×“.
  - Tasten: Enter/Shift+Enter im Feld, F3/Shift+F3 überall, Escape schliesst.
  - Bei geändertem Text springt sie zum ersten Treffer ab dem Cursor (mit Umlauf). Schliessen gibt den Fokus an den
    Editor zurück.

## M-14 Notizen anlegen, umbenennen, löschen (UC-003, UC-006, UC-007)

- [~] **M-14.1** Dialog New note:
  - Vorbefüllt mit dem Ordner der aktiven Notiz und dem Standardformat.
  - Felder: Titel (Autofokus), „Folder (empty for the top folder, new folders are created):“, Format-Radio.
  - Live „File: {name}“.
  - „Create“/Enter, „Cancel“. *Kein Escape.*
  - Erfolg: Tab offen, Cursor am Ende, „Created the note {t}“.
  - Existiert die Notiz: „The note {t} exists. Change the title or the folder, or open it.“ mit „Open the existing
    note“.
  - E/A-Fehler: der Dialog schliesst, Status. Andere Probleme stehen inline.
- [~] **M-14.2** Dialog Rename:
  - Öffnen mit F2, Tab- oder Übersichtsmenü.
  - Bei Konflikt verweigert: „{t} is in conflict; resolve it before renaming it“.
  - Titel ganz markiert, Ordner, live „File: …“. Bei der Home-Notiz die Wiki-Warnung.
  - Rename/Enter, Cancel/Escape.
  - Vor dem Umbenennen wird der Tab gespeichert. Danach werden die Links angepasst, auch in offenen Tabs, die im
    Speicher geändert und still gespeichert werden.
  - Meldung „Renamed A to B; updated N link(s) in M note(s)“ und gegebenenfalls „; these notes could not be updated
    …“.
  - Existiert der Zielname: „A note named {t} exists in that folder. Change the title or the folder.“
- [~] **M-14.3** Dialog Delete:
  - Bei Konflikt verweigert.
  - Text „Delete the note {t}?“, dazu die Link-Zählung, die Home-Warnung und „It is kept among the deleted notes for
    30 days.“
  - Delete/Enter, Cancel/Escape. *Beide wirken heute überall (Defekt D-01).*
  - Lässt sich die Notiz nicht aufbewahren, bietet der Dialog „Delete for good“ an.
  - Erfolg: Tab zu, „Deleted {t}“ plus Undo (öffnet den Tab wieder).
- [~] **M-14.4** Restore-Dialog, wenn der Platz belegt ist (Titel und Ordner neu, „Restored {t}“). Purge-Dialog
  „Remove N deleted note(s) for good? …“.

## M-15 Export (UC-009)

- [~] **M-15.1** Auslöser:
  - Ctrl+Shift+E oder das Tab-Menü → Auswahl der Art („Export {t} as:“, Up/Down/Enter, Escape).
  - Das Übersichtsmenü gibt die Art direkt vor.
  - Bei Konflikt verweigert.
- [~] **M-15.2** Speicherdialog des Systems „Export as {label}“:
  - Start im letzten Export-Ordner (nur für die Sitzung), sonst Home. Dateiname = Notizname plus Endung, ein Filter.
  - Ohne Dialog ein Pfadfeld als Rückfall: „The file exists. Export again to replace it.“ → „Replace“.
- [~] **M-15.3** Die Notiz selbst und Dateien offener Tabs dürfen nicht überschrieben werden: „A note cannot be
  replaced by an export; choose another place“.
- [~] **M-15.4** Inhalt ist der Tab-Text samt ungespeicherter Änderungen. Papier richtet sich nach dem Locale
  (`LC_ALL`/`LC_PAPER`/`LANG`).
- [~] **M-15.5** Der Export läuft im Hintergrund: „Exporting {t}…“, dann „Exported {t} to {p}“ samt „; N block(s)
  only approximated“ und „; missing: …“.
- [~] **M-15.6** Fenster „Approximated blocks“ mit einer Liste „Line N“.

## M-16 Werkzeugpanel rechts (FR-019, FR-041–043)

- [ ] **M-16.1** Breite veränderbar (100–1000) und wird gespeichert, aber nur mit aktivem Tab.
- [x] **M-16.2** Properties: Place, Format, Size, Last change, sonst „Not saved yet“.
- [ ] **M-16.3** Abschnitt Todos (M-18.1).
- [~] **M-16.4** Commit / Changes / Diff (M-19), nur in Git-Workspaces.
- [~] **M-16.5** Outline / Code / Figures (N):
  - Aus den Blöcken, gecacht je Revision.
  - Einrückung 14 px je Tiefe. Leere Liste „None“.
  - Ein Klick zeigt die Stelle (Facade: `reveal`, Quelle: Sprung und Fokus).

## M-17 Agenten / MCP (UC-016 bis UC-019)

- [ ] **M-17.1** Agents-Menü in der Statusleiste:
  - Text „Agents: {status}“, „ in this workspace“ und „ · Agent(s) connected: A, B“ bzw. „ · A asked for access“.
  - Radio-Auswahl der Stufen bis zur Obergrenze des Workspace. Die Wahl ändert die **Benutzer**-Einstellung.
  - „Settings of this workspace…“.
- [ ] **M-17.2** Abschnitt Agents in den Einstellungen:
  - Ohne Workspace: „Open a workspace to connect agents to it.“
  - Raster je Tool mit dem Zustand: not found, not installed, installed as X, installed for another place, cannot be
    read. Dazu Install bzw. Remove.
  - Zustände werden beim Öffnen und nach jeder Aktion neu geprüft.
- [ ] **M-17.3** Install bei Zugriff Off fragt nach: „Allow reading and install“, „Allow reading and writing and
  install“, „Install only“, „Cancel“. Bei ForAnotherPlace: „Replace“ oder „Cancel“.
- [ ] **M-17.4** Meldungen:
  - „Installed … restart {tool} if it does not show it yet.“ (gegebenenfalls mit dem Hinweis, dass der Zugriff aus
    ist)
  - „Removed …“
  - „{tool} was not changed: …“
- [ ] **M-17.5** „Copy configuration“ kopiert das Snippet: „Copied the configuration of {ws}. Its command: …“. Der
  Befehl ist `current_exe` plus Workspace.
- [ ] **M-17.6** Agent-Cursor (FR-057/058):
  - Markierte Zeilen bzw. Blöcke werden blau getönt, mit einem 3-px-Balken links und dem Agent-Namen in 80 % Grösse.
  - Sichtbar 10 s nach dem letzten Lesen/Schreiben, gefiltert nach Zugriff.
  - Die Farben sind fest, nicht je Theme.
- [ ] **M-17.7** Agent-Schreiben durch das Fenster (ADR-018):
  - Präsenzdatei `window` wird jede Sekunde erneuert. Requests werden alle 100 ms gelesen, solange ein Agent
    verbunden ist.
  - Ohne Schreibrecht verweigert: „Underline allows agents only to read the notes of this workspace“.
  - Nicht offene Notiz → `apply_to_file`, Status „{agent} created A and changed B“; Meldungen desselben Agenten
    werden zusammengefasst.
  - Offene Notiz → Antwort `Busy`, solange gespeichert wird. Sonst `plan_for_tab` → versiegelte Änderung → sofort
    speichern → Antwort.
  - Ein Todo, den ein Agent ändert, gibt den Status „{agent} {change}“.
- [ ] **M-17.8** Beim Beenden wird die Fenster-Präsenz entfernt. *Beim Workspace-Wechsel heute nicht (Defekt D-05).*

## M-18 Todos (UC-020, UC-021)

- [ ] **M-18.1** Abschnitt „Todos (N open)“:
  - Oben: Todo-Konflikte, „Open list“ und das Feld „New todo“ (Enter legt an und behält den Fokus,
    „Created todo {name}“). Fehler NoTitle/LongTitle inline. Danach eine Zeile je unlesbarer Datei.
  - Offene Todos sortiert nach (ohne Frist zuletzt, Frist, Nummer), leer „No open todos“. Gruppe „Done (N)“
    zugeklappt.
- [ ] **M-18.2** Zeile:
  - Checkbox.
  - Titel in Dringlichkeitsfarbe: Low schwach, Normal Text, High orange, Critical rot. Erledigte sind
    durchgestrichen.
  - „due {d}“.
  - „not stored“ mit dem Grund im Hover.
- [ ] **M-18.3** Detail-Editor (Klick auf den Titel):
  - Titel, Due und Scheduled (YYYY-MM-DD); gespeichert wird beim Verlassen des Feldes, Parsefehler stehen inline.
  - „Pick“ öffnet den Kalender „Choose a day“: ◀/▶, Montag zuerst, heute fett, „No day“. Die Wahl speichert sofort.
  - Urgency-Combo.
  - Warnung „The scheduled day is after the due day.“
- [ ] **M-18.4** Notiz-Links:
  - Ein vorhandener Link öffnet die Notiz. Ein fehlender zeigt „(missing)“ und „The note X no longer exists; remove
    the link with ×“.
  - „×“ entfernt den Link, „Link {active}“ verlinkt die aktive Notiz.
  - „Link a note…“ zeigt bis zu 8 Treffer, sonst „No note matches“.
- [ ] **M-18.5** „Delete“ ohne Rückfrage: „Deleted todo {name}“ plus Undo. Angezeigt werden „created/done
  YYYY-MM-DD HH:MM“.
- [ ] **M-18.6** Todos-Tab:
  - Filter (`TodoFilter::ALL`), Sortierung „Due date“/„Urgency“ und Feld „New todo“ (320 px).
  - Leertexte: „No open todos. Type a title above to create one.“, „No done todos.“, „No todos. Type a title above to
    create one.“
- [ ] **M-18.7** Tasten im Todos-Tab (nur ohne fokussiertes Textfeld):
  - Up/Down wählen.
  - Space hakt ab bzw. öffnet wieder: „Marked todo X done/open“ plus Undo.
  - Enter öffnet die erste verlinkte Notiz.
- [ ] **M-18.8** Spalten des Todos-Tabs:
  - Checkbox, „#N“, Titel.
  - Due: der Kopf sortiert; überfällig rot „{due}  N day(s) overdue“.
  - Urgency: der Kopf sortiert.
  - Scheduled.
  - Notes: bis zu 2, dann „+N“ mit Hover.
  - Done.

  Sortierung und Filter werden je Workspace gespeichert.
- [ ] **M-18.9** Hintergrund (jede Sekunde): ungespeicherte Todos erneut versuchen, Änderungen per Fingerprint
  erkennen und neu laden, Konflikte nach einem Sync prüfen.

## M-19 Git (UC-011 bis UC-015)

- [~] **M-19.1** Commit-Box:
  - Während eines Merge steht stattdessen eine Warnung.
  - Die Nachricht folgt `proposed_message`, bis der Benutzer tippt. Ctrl/Cmd+Enter committet.
  - Hinweis bei einer ersten Zeile über 72 Zeichen.
  - „Amend the last commit“ übernimmt die letzte Nachricht.
  - „Commit“/„Amend“ (deaktiviert ohne Auswahl), „All“, „None“, „Nothing is chosen.“
- [~] **M-19.2** Liste „Changes (N)“:
  - Zugeklappt; Ctrl+Shift+G oder „Show changes“ öffnet sie und fokussiert die Nachricht.
  - Leer: „Nothing changed since the last commit.“ Ein Trenner „Other files“.
  - Zeile: Checkbox, farbiger Buchstabe, „Title · folder“ bzw. „New ← Old“.
  - Ein Klick öffnet die Notiz bzw. wählt sie für den Diff.
  - Aktionen: „Keep here“/„Keep remote“, „Keep“/„Delete“, „Discard…“.
- [~] **M-19.3** Diff (FR-049):
  - Live-Tabtext gegen `committed`, 3 Kontextzeilen, höchstens alle 700 ms neu.
  - Zeile „{old:>4} {new:>4} ±“ mit Wort-Hervorhebung, „(only spaces differ)“ und „(only the line ending
    differs)“.
  - Ein Klick springt im Tab zur Zeile.
  - Leertexte: „Open a note to see its changes.“, „This file is not text; …“, „The note did not change since the
    last commit.“
- [~] **M-19.4** Commit:
  - Verweigert mit „A commit needs a message.“ oder „Nothing is chosen.“
  - Vorher wird alles gespeichert, sonst „Nothing was committed, because …“.
  - Erfolg „Committed {id} holding N note(s) and M file(s)“ bzw. „Amended …“, danach ein Sync.
  - NoIdentity → Dialog „Your name for commits“: Name und E-Mail mit '@', Escape bricht ab.
- [~] **M-19.5** Discard-Dialog:
  - Texte je Fall (neu, zurück wie committet, N Zeilen gehen verloren).
  - Wirkung je Art: neu → löschen; umbenannt → zurück; sonst `restore_committed`. Bei Konflikt verweigert.
  - Der Tab wird neu geladen. „Discarded the changes of X“.
- [ ] **M-19.6** Branch-Menü (UC-013, Ctrl+Shift+B):
  - Fenster unten links mit Filter: erst lokale, dann Remote-Branches, Kennzeichen „(current)“ und „(on the remote)“.
    Up/Down/Enter, Escape. Während eines Merge deaktiviert.
  - „New branch…“ vorbefüllt mit dem Filter, live „Branch: {clean}“ oder der Fehler. Existiert der Branch: „The
    branch X exists; choose it in the list to switch to it.“
  - Weitere Buttons: „Show changes“, „Pull“, „Push“.
- [ ] **M-19.7** Branch wechseln:
  - Vorher speichern, sonst „Did not switch, because …“.
  - Tabs, deren Datei weg ist, schliessen; geänderte neu laden.
  - „Switched to X; carried along N change(s); closed A, B, not on this branch“.
  - WouldOverwrite → „… Commit them first, or stay.“ mit „Commit first“.
- [ ] **M-19.8** Sync (UC-014):
  - Nur mit `.git` und Remote.
  - Auslöser: Workspace öffnen, nach Commit, nach gelösten Konflikten, Sync now, Pull, Push, Intervall
    (`git_sync_interval` Minuten, mindestens 1, ab dem Ende des letzten Syncs) und Schliessen.
  - Vorher wird alles gespeichert; bei einem Fehler läuft der Sync trotzdem, mit Meldung.
  - Ablehnungen (kein Remote, Merge, detached) werden nur gemeldet, wenn der Benutzer gefragt hat.
  - Ergebnismeldungen: Conflict, InTheWay, PushRejected, SignIn, NotPossible (Texte in `app:3725-3840`). Synced →
    Tabs neu laden und neu einlesen.
- [ ] **M-19.9** Konfliktleiste (UC-015):
  - „Conflict: place i of n open (line L)“.
  - Previous/Shift+F7, Next/F7, „All: Here“, „All: Remote“, „Abort the sync“.
  - Spalten Here/Remote (Monospace, max. 160 px).
  - Here Alt+H, Remote Alt+R, Both Alt+B, „Or edit the place in the note.“
- [ ] **M-19.10** Konfliktauflösung:
  - Eine Notiz ohne Marker wird gespeichert und als gelöst markiert.
  - Ist alles gelöst → `complete_joining`, „The conflicts are resolved; syncing“.
  - Todo-Konflikte: Raster mit fetten Unterschieden, „Keep mine“/„Take remote“, „Todo #a is now #b“.
  - Abort-Rückfrage mit dem Text aus UC-015.

## M-20 Einstellungen (UC-010)

- [~] **M-20.1** Fenster „Settings“, Ctrl/Cmd+, schaltet es um, 460 px breit, in der Grösse veränderbar.
- [~] **M-20.2** Bereiche „For me“ und „This workspace“ (nur mit Workspace). Liegt die Workspace-Datei im
  Repository, warnt „These values are in underline.yaml of the repository and apply to everyone who uses it.“
- [~] **M-20.3** Einträge (`SettingKey::ALL`):
  - Default format (nur Workspace).
  - Theme, Input mode, Line number side.
  - Line numbers, Minimap, Copy on select, Menu bar, Search hits in the minimap.
  - Git sync every 1–60 min.
  - Panelbreiten 100–1000.
  - Agent access (im Workspace nur bis zur Benutzerstufe).
- [~] **M-20.4** Dritte Spalte:
  - Workspace: „As for me“, „own“, „as for me“.
  - Benutzer: „this workspace asks for more; not followed“, „this workspace uses its own value“.
  - Format: „{fmt}“, „{fmt} (from the notes)“, „Follow the notes“.
- [~] **M-20.5** Jede Änderung wirkt sofort und wird sofort geschrieben. Schnellumschalter gehen in die
  Workspace-Datei, wenn diese den Schlüssel setzt, sonst in die Benutzerdatei (BR-003). Fehlermeldungen stehen in
  `settings.rs:360-377`.
- [~] **M-20.6** Die Benutzerdatei wird alle 500 ms auf Änderungen geprüft. Die Workspace-Datei kommt über den
  Watcher.

## M-21 Workspace öffnen, anlegen, schliessen (UC-001, UC-002)

- [x] **M-21.1** Dialog Open:
  - „Recently opened:“, „Another directory:“ als Pfadfeld (*kein Systemdialog*).
  - Angebote: „Create a new workspace there“, „Open {outer}“, „Open {inner} anyway“. Sonst „{p}. Choose another
    directory.“
  - Ist es derselbe Workspace, schliesst nur der Dialog.
  - Vorher wird gespeichert, sonst „The workspace stays open, because …“.
  - „Opened {root}“ samt Hinweisen zu den Einstellungen.
- [x] **M-21.2** Dialog Create in drei Schritten:
  1. Verzeichnis, mit „Open it as a workspace instead“.
  2. Format und „Create as git repository“ (aus).
  3. Optionaler Remote mit dem Hinweis „The remote is not contacted now; …“.

  Erfolg „Workspace created in {root}“, mit Home-Notiz aus der Registry.
- [ ] **M-21.3** Workspace-Wechsel startet Syncer, Watcher, Searcher und Tracker, stellt die Tabs wieder her und
  räumt alte gelöschte Notizen auf.
- [ ] **M-21.4** Schliessen:
  - Alles speichern. Bei Fehler bleibt das Fenster offen, mit „Changes not stored“: „Close {the tab X|Underline}
    anyway and lose these changes?“, „Keep open“/„Close anyway“.
  - Mit Syncer: „Syncing before closing…“, dann schliessen (höchstens 10 s).
  - Am Ende State schreiben und Präsenz entfernen.

## M-22 Persistenz

- [x] **M-22.1** `<data_dir>/underline/state.yaml`: letzter und zuletzt geöffnete Workspaces, geschrieben 500 ms
  nach einer Änderung und beim Beenden.
- [x] **M-22.2** `<ws>/.underline/state.yaml`: Tabs (Notiz, Position, Ansicht, aktiv), „zuletzt geöffnet“ je Notiz,
  Zustand des Todos-Tabs.
- [~] **M-22.3** Einstellungen in `~/underline.yaml` und `<ws>/underline.yaml`, sofort geschrieben.
- [ ] **M-22.4** Nicht gespeichert: Fenstergeometrie, Klappzustände, Web-Zustimmung, letzter Export-Ordner,
  Wiki-URL, Such- und Find-Text, Commit-Nachricht, Cursor- und Scrollpositionen.

---

## Bekannte Defekte und Lücken der heutigen App

Was hier steht, wird **nicht** stillschweigend übernommen oder korrigiert. Entschieden wird es mit Frage F-4 im Plan.

| ID   | Befund | Ort |
|------|--------|-----|
| D-01 | Delete-, Purge- und Discard-Dialoge bestätigen mit Enter **überall**; Enter und Escape werden nicht verbraucht und erreichen andere Widgets. | `app:5150, 5359, 5533` |
| D-02 | New note, Open, Create, Tool, Web und Close-Confirm haben kein Escape. | diverse |
| D-03 | Export-Pfadfeld: `replace` bleibt nach Pfadänderung gesetzt; eine andere vorhandene Datei wird ohne Warnung überschrieben. | `app:5774-5783` |
| D-04 | Undo/Open/Show-Angebote verschwinden mit jeder späteren Statusmeldung, z. B. „Saved X“. | `app:6273-6331` |
| D-05 | Die Fenster-Präsenz wird beim Workspace-Wechsel nicht entfernt. | `app:5956` |
| D-06 | `git_state` (UC-002 Schritt 9) wird nie gesetzt. | `app:64, 1441, 1634` |
| D-07 | Das Schliessen des ältesten Tabs am Tab-Limit ignoriert Speicherfehler, dadurch Datenverlust möglich. | `app:2305` |
| D-08 | `request_sync` setzt „running“, auch wenn das Speichern davor scheiterte. | `app:3746-3758` |
| D-09 | Der Export-Rückfall gilt schon, wenn der Benutzer den Dialog innerhalb von 300 ms abbricht. | `app:5728` |
| D-10 | Diagramme bekommen nur hell/dunkel, nicht die Theme-Palette (FR-038 nur teilweise erfüllt). | `facade/diagram/mod.rs:65-67` |
| D-11 | „Ctrl+Shift+E“ ist im Tab-Menü auch auf macOS fest beschriftet. | `app:6613` |
| D-12 | Keine Systemdialoge für Verzeichnisse und Bilder (ADR-016 sagt „später“). | – |
| D-13 | Das Wiki-Menü ist ein Stub. | `app:4718-4734` |
| D-14 | Der Editor meldet keine Accessibility-Knoten; IME-Komposition ist unsichtbar. | `sv:1225` |
| D-15 | Drop von Links aus dem Browser (FR-034) ist nicht umgesetzt. | – |
| D-16 | Viel synchrone Git- und Datei-E/A im UI-Thread; Repaint mindestens jede Sekunde. | `app:3218` u. a. |
| D-17 | Facade: Enter und danach Backspace verschluckt ein Leerzeichen (`image block` wird zu `imageblock`). Gefunden im Spike F0, in der egui-App nicht nachgeprüft. | `fv:1628-1656`, `fv:1789-1851` |
| D-18 | `insert_typed` verschiebt die Quell-Offsets von Tabellenzellen nicht; sie sind bis zum nächsten vollständigen Parsen falsch. | `facade/blocks.rs:165-244` |

Entscheidung (2026-11-09):

- D-01 bis D-09, D-11, D-17 und D-18 werden in der Tauri-App behoben und in `paritaet.md` als gewollte Abweichung
  geführt.
- Die egui-App bleibt die unveränderte Referenz.
- D-10 und D-12 bis D-16 bleiben wie heute.
