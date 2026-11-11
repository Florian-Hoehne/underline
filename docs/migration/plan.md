# Migrationsplan: Underline von egui/eframe nach Tauri 2

Stand: 2026-11-09, Branch `migration/tauri`. Zur Freigabe; die offenen Entscheidungen stehen am Ende (F-1 bis F-4).
Die Parität wird gegen [feature-inventar.md](feature-inventar.md) geprüft.

## 1. Ausgangslage

### 1.1 Aufbau des Workspace

| Crate | Zeilen | Rolle | UI-Abhängigkeit |
|---|---|---|---|
| `crates/underline-core` | 12'800 | Workspace, Notizen, Einstellungen, State, atomare Writes, Autosave-Saver, Watcher, Suche, Git, Sync, Konflikte, Todos, Agent-Requests | keine |
| `crates/underline-facade` | 8'200 | Blockmodell (comrak, eigener AsciiDoc-Parser), Formatieren, Sektionen, syntect, Themes/Paletten, Clipboard-HTML, Bilder, Diagramme (merman, PlantUML in QuickJS, resvg), Export (Typst) | keine |
| `crates/underline-mcp` | 1'900 | MCP-Server über stdio, Installer für Agent-Tools | keine |
| `crates/underline-wiki` | 100 | Wiki-Remote-URLs | keine |
| `crates/underline-editor` | 8'300 | eigenes Editor-Widget: Rope und Undo, Quellansicht, Facade, Vim/Nano, Minimap, Toolbar, Clipboard, Vergrössern | **egui** |
| `apps/underline-desktop` | 8'300 | Fenster, alle Panels und Dialoge, Orchestrierung, CLI inkl. `mcp` | **egui, rfd** |

Es gibt rund 300 Tests: Unit-Tests in den Libraries und kittest-Tests im Editor. Das App-Crate hat keine Tests.
Die CI führt fmt, clippy und test auf Linux, Windows und macOS aus.

### 1.2 Was ist UI, was ist Kernlogik

- **Kernlogik, unverändert wiederverwendbar (~23'000 Zeilen):** core, facade, mcp, wiki. Sie liefern schon
  Hintergrunddienste mit Wake-Callbacks: `NoteSaver`, `WorkspaceWatcher`, `Searcher`, `ChangeTracker`, `Syncer`,
  `Diagrams` und `Images`.
- **Kernlogik, die im App-Crate steckt (~2'600–3'000 Zeilen):**
  - Workspace-Sitzung, Tabs, Speicher-Pipeline, Abgleich mit Änderungen von aussen
  - Agent-Requests auf Tabs anwenden (ADR-018), Git-Abläufe (Commit, Discard, Branch-Wechsel mit Tab-Abgleich,
    Sync-Regeln, Konflikte)
  - Todos-Polling, Settings-Zustandsautomat, Export-Orchestrierung, Links folgen, Drop/Einfügen
  - Statustexte

  Hindernis: `OpenTab` hält die egui-Widgets, und diese besitzen das Dokument.
- **Editor-Logik im Editor-Crate, portierbar (~3'200 Zeilen):**
  - `document.rs`: Rope, Undo-Regeln, Siegel
  - `keys.rs`: Vim/Nano-Automat hinter einem `Target`-Trait; braucht nur die egui-Tasttypen
  - die Bearbeitungslogik von Quelle und Facade: Tippen mit Escaping, Enter, Backspace, Einfügen, Faltlogik
- **Reines egui (~3'200 Zeilen plus ~1'500 Zeilen Tests):** Zeichnen, Layout, Galleys, Minimap, Blinken,
  Vergrössern, Event-Verteilung. Das wird im Frontend neu gebaut.

### 1.3 Nebenläufigkeit heute

Ein egui-Frame-Loop pollt die Dienste und wird per `request_repaint` geweckt. Dazu kommen feste Takte:

| Takt | Aufgabe |
|---|---|
| 100 ms | Agent-Requests |
| 250 ms | Saver |
| 500 ms | Benutzereinstellungen, State |
| 700 ms | Diff |
| 1 s | Präsenz, Todos |
| 3 s | ChangeTracker |
| Intervall | Sync |

Export und rfd laufen in eigenen Threads mit `mpsc`. Viel Git- und Datei-E/A läuft synchron im UI-Thread.

## 2. Frontend-Wahl

**Empfehlung: b) TypeScript + Svelte 5 (Vite).** Der Editor wird ein eigenes Modul auf CodeMirror 6. Die
Editor-Kernlogik bleibt Rust und läuft, je nach Spike-Ergebnis (Abschnitt 6), als WASM-Modul im Frontend oder im
Backend.

Begründung:

1. **Der Editor bestimmt die Wahl.** Er ist der grösste und schwierigste Teil. Er braucht direkte DOM-Arbeit:
   `beforeinput`, IME-Komposition, `contenteditable`, Selection-API, virtuelles Scrollen. Dazu kommt CodeMirror 6,
   das 10'000 Zeilen, Soft-Wrap, IME, Gutter, Decorations und Accessibility schon mitbringt. Das alles ist
   TypeScript. Mit Leptos würde man dieselben JS-APIs über `web-sys`/`wasm-bindgen` ansprechen, mit mehr Reibung und
   ohne Gewinn.
2. **Rust-Logik bleibt trotzdem Rust.** Wo Logik im Frontend laufen muss (Vim/Nano-Automat, Undo, Facade-Edits),
   wird das *bestehende* Rust-Crate nach WASM kompiliert und von TS aufgerufen. Das geht unabhängig vom
   UI-Framework.
3. **Svelte statt React:** kleineres Bundle und weniger Laufzeit-Overhead. Das hilft beim Startzeit-Ziel
   NFR-001 (≤ 1 s). Der feingranulare Zustand mit Runes passt auf viele kleine Push-Events (Status, Sync, Agenten).
   Für die vielen Dialoge braucht es keine grosse Komponentenbibliothek. Das heutige UI ist schlicht, und die
   Optik kommt aus den Theme-Paletten.
4. **Leptos (a)** wäre die Wahl, wenn das UI überwiegend aus Formularen bestünde. Hier spricht dagegen: längere
   Build-Zyklen (WASM bei jeder UI-Änderung), weniger Hilfe beim Testen (Vitest/Testing Library, Playwright-Mocks)
   und die Brücke zu CodeMirror.

## 3. Workspace-Struktur (Ziel)

```
Cargo.toml                       # Workspace: + neue Members
crates/
  underline-core/                # unverändert
  underline-facade/              # unverändert (+ Palette an Diagramme, falls D-10 behoben wird)
  underline-mcp/                 # unverändert
  underline-wiki/                # unverändert
  underline-editor/              # egui-Widget, bleibt bis zur Abnahme; nutzt künftig editor-core
  underline-editor-core/         # NEU: Document/Undo, keys.rs mit eigenem Tasttyp, Quell- und Facade-Edit-Logik,
                                 #      Falt-Logik; ohne egui; baut nativ und für wasm32
  underline-session/             # NEU: Workspace-Sitzung ohne UI: Tabs, Speichern, Änderungen von aussen,
                                 #      Agent-Requests, Git-Abläufe, Sync-Regeln, Todos-Poll, Settings-Automat,
                                 #      Export-Orchestrierung, Links, Statustexte; mit Tests (UC-IDs)
apps/
  underline-desktop/             # egui-App, bleibt unverändert als Referenz bis zur Abnahme
  underline-tauri/
    src-tauri/                   # Tauri-2-Crate: Commands/Events als dünne Schicht über underline-session,
                                 #   CLI inkl. `mcp`-Subcommand, Capabilities, tauri.conf.json
    ui/                          # Svelte 5 + TS + Vite; src/lib/editor (CodeMirror 6 + Facade),
                                 #   src/lib/bindings.ts (generiert), Vitest, ESLint, Prettier
    ui/wasm/ (optional)          # wasm-bindgen-Hülle um underline-editor-core (nur bei Spike-Variante B)
```

Damit die Kernlogik möglichst unverändert bleibt, gilt:

- **underline-editor-core** entsteht durch *Verschieben* aus `underline-editor`. Das egui-Widget hängt danach
  davon ab. Die vorhandenen kittest-Tests sichern ab, dass die egui-App sich nicht ändert. Die einzige echte
  Änderung: `keys.rs` bekommt einen eigenen Tasttyp mit `From<egui::Key>` im Widget-Crate.
- **underline-session** entsteht als Portierung aus `app.rs`, je nach Antwort auf F-3.
- Binärname während der Migration: `underline-tauri`. Bei der Ablösung wird er zu `underline` umbenannt, damit
  bereits installierte MCP-Einträge (`current_exe` plus `mcp`) weiter stimmen.

## 4. Schnittstelle Frontend ↔ Backend

**Typen:** `specta` plus `tauri-specta` (Tauri-2-Linie) erzeugen `ui/src/lib/bindings.ts` mit allen Commands,
Events und DTOs. Die Datei wird bei jedem Debug-Start neu geschrieben; ein CI-Test prüft, dass sie aktuell ist.
DTOs liegen in `underline-session` und hängen hinter einem Cargo-Feature `specta`. So bleiben core und facade
frei von Codegen-Abhängigkeiten. **Rückfall:** Ist tauri-specta beim Setup nicht stabil für die verwendete
Tauri-Version, nehmen wir `ts-rs` für die Typen und schreiben die Command-Wrapper von Hand. Das entscheidet sich in
Phase 2 nach einer Versionsprüfung.

**Fehler:** jeder Command gibt `Result<T, UiError>` zurück. `UiError` ist `{ kind, message }`, und `message` ist
der heutige Statustext. So bleiben die Meldungen 1:1 gleich.

### 4.1 Commands (gruppiert; Typen sind DTOs aus underline-session)

| Gruppe | Commands | Inventar |
|---|---|---|
| App | `app_start() -> StartInfo` (CLI-Arg, letzter Workspace, Notice, Plattform für „Cmd/Ctrl“), `app_request_close() -> CloseOutcome`, `app_close_anyway()` | M-01, M-21.4 |
| Workspace | `ws_open(path) -> Opened \| OpenProblem`, `ws_check_dir(path) -> DirectoryProblem?`, `ws_create(NewWorkspace)`, `ws_recent() -> Vec<Path>`, `ws_overview() -> Vec<OverviewEntry>`, `ws_deleted() -> Vec<DeletedNote>`, `ws_restore(id, title?, folder?)`, `ws_purge(ids)` | M-05, M-14.4, M-21 |
| Tabs/Notizen | `tab_open(note, at?) -> TabSnapshot`, `tab_activate(tab)`, `tab_close(tab) -> CloseOutcome`, `tab_set_view(tab, EditingView)`, `tab_store(tab)`, `note_plan(NewNote) -> Plan`, `note_create(NewNote)`, `note_plan_rename(..)`, `note_rename(..)`, `note_delete(id, for_good)`, `note_undo_delete()`, `note_properties(tab)` | M-06, M-07, M-14, M-16.2 |
| Editor | Hängt von Spike-Variante A/B ab (Abschnitt 6). A: `ed_input(tab, rev, EditorInput) -> EditorPatch` (Tasten, Text, Einfügen, Toolbar). B: `ed_sync(tab, base_rev, Vec<Change>) -> Ack`. Beide: `ed_highlight(tab, lines) -> Vec<Spans>`, `ed_blocks(tab, rev) -> Vec<BlockView>`, `ed_sections(tab)`, `ed_indexes(tab) -> Indexes`, `ed_follow_link(tab, link) -> LinkTarget`, `ed_drop(tab, at, paths)`, `ed_paste_image(tab, png)`, `ed_insert_link/image(..)` | M-08 bis M-12, M-16.5 |
| Diagramme/Bilder | `fig_diagram(lang, src, theme) -> Svg \| Pending \| Error`, Bilder über das eigene Protokoll `ul-asset://` (nur Workspace und Web-Cache) | M-09.6/7, M-09.15 |
| Clipboard | `clip_copy(markup, html?)`, `clip_paste() -> ClipContent`, `clip_copy_on_select(..)` (arboard im Backend, wie ADR-014) | M-09.14, M-10.7 |
| Suche | `search_set(query)`, `find_in_note(tab, text, dir) -> FindResult` | M-13 |
| Settings | `settings_get() -> SettingsView`, `settings_set(scope, key, value)`, `settings_reset(key)`, `settings_quick(key, value)` | M-03, M-20 |
| Export | `export_choose_path(kind) -> Path?` (Dialog-Plugin), `export_run(tab \| note, kind, path)` | M-15 |
| Git | `git_changes()`, `git_diff(tab)`, `git_commit(msg, chosen, amend)`, `git_set_identity(..)`, `git_discard(file)`, `git_take_side(..)`, `git_branches(filter)`, `git_switch(..)`, `git_create_branch(..)`, `sync_request(mode)`, `conflict_choose(tab, place, Choice)`, `conflict_abort()`, `todo_conflict_resolve(..)` | M-19 |
| Agenten | `agents_status()`, `agents_set_access(level)`, `mcp_install_states()`, `mcp_install(tool, access?)`, `mcp_remove(tool)`, `mcp_copy_snippet()` | M-17 |
| Todos | `todos_list(sort, filter)`, `todo_create(title)`, `todo_save(Todo)`, `todo_delete(n)`, `todo_restore(..)`, `todo_toggle(n)` | M-18 |
| Links | `open_web(url)` (nach Zustimmung), `open_file(path)` | M-07.3 |

### 4.2 Events (Backend → Frontend)

Die Frontend-Stores halten jeweils den letzten Stand. Jedes Event trägt den vollständigen Teilzustand, keine
Deltas. Ausnahme ist der Editor.

| Event | Payload | Ersetzt heute |
|---|---|---|
| `status` | `{ text, offer?: Undo\|Open\|Show… }` | `self.status` + Angebote |
| `overview` | `Vec<OverviewEntry>` + Change-Marks | Rescan/Watcher |
| `tab-changed` | `{ tab, reason: Reloaded\|Deleted\|Stored\|SaveFailed\|OutsideCopy, snapshot? }` | Watcher, Saver |
| `agent-edit` | `{ tab, rev, edit, sealed: true }` | ADR-018-Anwendung |
| `agent-marks` | `Vec<{ tab, lines \| blocks, name }>` | `set_agent_marks` |
| `agents` | Präsenz, Zugriff | 1-s-Poll |
| `search-result` | `SearchResult` | Searcher |
| `changes` | `Changes` | ChangeTracker 3 s |
| `sync` | Sync-Zustand + Distanz | Syncer |
| `conflicts` | Liste der Stellen | `check_resolved` |
| `todos` | `Todos` + Fehler | 1-s-Poll |
| `settings` | `SettingsView` | Settings-Poll, Watcher |
| `diagram-ready` | `{ key }` | Diagrams-Wake |
| `export-done` | `Exported \| Error` | mpsc |

## 5. State-Handling

- **Backend:** `tauri::State<Session>`. `Session` kommt aus underline-session und hält Workspace, Tabs (Dokument,
  Platteninhalt, Autosave-Uhr, Fehler), Dienste und Einstellungen. Zugriff über `parking_lot::Mutex`; Dienste mit
  eigenen Threads halten den Lock nur kurz.
- **Der egui-Frame-Loop wird zu einem Backend-Tick.** Ein Thread mit 100-ms-Takt ruft `session.tick(now)` auf.
  `tick` übernimmt die heutigen Takte 1:1 (Agent-Requests 100 ms, Settings 500 ms, Präsenz/Todos 1 s,
  ChangeTracker 3 s, Sync-Intervall, Autosave-Fristen) und liefert eine Liste von Events. Wake-Callbacks der
  Dienste stossen den Tick sofort an, statt `request_repaint` zu rufen. So bleiben Zeitverhalten und NFR-005/008
  gleich, und das Frontend muss nicht pollen.
- **Synchrone Git-/Datei-E/A** läuft in Commands auf Tauris Thread-Pool, nicht mehr im UI-Thread. Das ist keine
  Verhaltensänderung, nur keine Ruckler mehr.
- **Frontend:** Svelte-Stores je Event (Spiegel des Backends) plus reiner UI-Zustand: offene Dialoge, Filtertexte,
  Fokus, Klappzustände. Wie heute ist dieser UI-Zustand nicht persistent (M-22.4).
- **Dokument im Editor:** siehe Abschnitt 6.

## 6. Der Editor (grösstes Risiko) und der Spike F0

Ziel: dasselbe Verhalten wie `underline-editor` (Inventar M-08 bis M-12). Dazu gehören byte-genaue Facade-Edits
(NFR-007), dieselben Vim/Nano-Automaten in beiden Ansichten und 16 ms Tipp-Latenz bei 10'000 Zeilen (NFR-002) auf
WebKitGTK, WebView2 und WKWebView.

**Darstellung, in beiden Varianten gleich:**

- **Quellansicht:** CodeMirror 6 als View-Schicht.
  - Ohne eigene Keymaps und ohne eigene History; Eingaben gehen an den Rust-Automaten.
  - Hervorhebung als Decorations aus den syntect-Spans des Backends (ADR-010 bleibt).
  - Falten über `foldService`, gespeist aus `sections()`.
  - Zeilennummern links oder rechts als eigener Gutter.
  - Treffer und Agent-Cursor als Decorations.
- **Facade:** ein eigenes `contenteditable` über HTML-Blöcken, die aus dem Rust-Blockmodell gerendert werden
  (`data-block`, Offsets je Run).
  - Jedes `beforeinput` wird abgefangen und als Befehl an die Rust-Facade-Logik gegeben.
  - Nur geänderte Blöcke werden ausgetauscht.
  - Diagramme als SVG direkt. Für die Anzeige braucht es kein resvg mehr; der Export braucht es weiter.
  - ProseMirror, TipTap und Milkdown scheiden aus: Sie serialisieren neu und verletzen damit NFR-007, und sie können
    kein AsciiDoc.
- **Minimap mit Sieb, Vergrössern, Statuszeile:** eigene Svelte-/Canvas-Komponenten.

**Wem gehört das Dokument? Das klärt der Spike:**

- **A: Backend-autoritativ.** Die Rope liegt in `Session`, und jede Eingabe ist ein IPC-Command. Einfach, eine
  Wahrheit, Agent-Edits und Rename wie heute. Risiko: ein IPC-Roundtrip je Taste plus Parse plus DOM-Patch, also
  nahe an 16 ms.
- **B: Editor-Kern als WASM im Frontend.** `underline-editor-core` wird nach wasm32 kompiliert. Tasten
  verursachen keinen IPC-Verkehr, das Verhalten ist identisch, weil es derselbe Rust-Code ist. Das Backend erhält
  Änderungen asynchron mit Revisionsnummern; Autosave und Agent-Edits laufen über ein Sync-Protokoll mit Rebase.
  Mehr Aufwand, aber keine Latenzgrenze.

**Spike F0** (vor allen Editor-Features, etwa 3–5 Tage):

- Quellansicht und Facade minimal, je in A und B.
- Messung auf Linux (WebKitGTK) und Windows (WebView2): Tipp-Latenz median und p99 bei 10'000 Zeilen, IME
  (deutsch und japanisch), Emoji, Ansichtswechsel.
- Ergebnis als ADR. Kriterium: A, wenn p99 ≤ 16 ms auf WebKitGTK, sonst B.

## 7. Zielplattformen

Desktop: Linux (X11 und Wayland), Windows 10+ und macOS 13+, wie C-001 und NFR-013. Mobile und Web sind laut
`docs/vision.md` ausdrücklich ausserhalb des Scope, deshalb gibt es keine Android/iOS-Targets.

Bundles:

- Linux: AppImage, deb
- Windows: NSIS
- macOS: dmg

Signieren und Notarisieren gehören zu Phase 4 und werden dort abgefragt.

## 8. Tauri-Plugins und Capabilities (minimal)

| Plugin | Wofür | Permissions |
|---|---|---|
| – | Export-Speicherdialog: bleibt rfd im Backend (ADR-016, entschieden in F11, A-106); kein `tauri-plugin-dialog` | – |

Mehr braucht es nicht:

- Browser und Systemprogramme öffnet weiter das `open`-Crate im Backend; kein Opener-Plugin.
- Die Zwischenablage bleibt arboard im Backend (Copy on select muss den alten Inhalt still lesen); kein
  Clipboard-Plugin.
- Kein FS-Plugin, weil alle Dateizugriffe über eigene Commands laufen.
- Kein window-state-Plugin, weil die Geometrie heute nicht gespeichert wird.
- Kein single-instance-Plugin, weil es heute keine Einzelinstanz-Regel gibt.

Capabilities:

- Eine Capability `main` für das Hauptfenster: `core:default` ohne Webview-Erzeugung, die eigenen Commands
  (`allow-*` per Build-Script).
- Drag & Drop über Tauris `onDragDropEvent` (liefert Pfade, wie heute `dropped_files`).
- CSP: `default-src 'self'; img-src 'self' ul-asset: data:; style-src 'self' 'unsafe-inline'; connect-src ipc:
  http://ipc.localhost`. Kein Fremd-Host. Web-Bilder kommen über das Backend und `ul-asset://` (ADR-013,
  NFR-015).
- Das Protokoll `ul-asset://` liefert nur Dateien unterhalb des Workspace und aus dem Web-Bild-Cache.

## 9. Feature-Reihenfolge

Jedes Feature durchläuft: Commands/Events → UI → Tests → rust-reviewer → ux-reviewer → app-tester → Inventar
abhaken → Commit.

| # | Feature | Inventar | Voraussetzung |
|---|---|---|---|
| F0 | Spike Editor A/B, Messung, ADR | – | Phase 2 |
| F1 | Shell: Start/CLI inkl. `mcp`, Theme als CSS-Variablen aus `Palette`, Top-Bar, Menü, Statusleiste (Grundgerüst), leerer Bildschirm | M-01, M-02, M-03, M-04.7 | Phase 2 |
| F2 | Workspace öffnen/anlegen, State, Übersicht, Tabs, Wiederherstellen | M-05.1–3, M-06, M-21.1–3, M-22 | F1 |
| F3 | Quellansicht lesen + bearbeiten, Autosave, Undo, Änderungen von aussen, Properties | M-07.5, M-08, M-10.1, M-10.8, M-16.2 | F0, F2 |
| F4 | Eingabemodi Vim/Nano, Statuszeile, Toolbar, Falten, Zeilennummern, Minimap | M-10.2, M-11, M-12 | F3 |
| F5 | Facade: Darstellung, Code, Tabellen, Bilder, Diagramme, Vergrössern, Bearbeiten | M-09 | F3 |
| F6 | Clipboard, Copy on select, Paste-Bild, Drop, Link/Bild-Dialog | M-09.14, M-10.3–7 | F5 |
| F7 | Links folgen, Outline/Code/Figures | M-07.3/4, M-16.5 | F5 |
| F8 | Notiz anlegen/umbenennen/löschen/wiederherstellen | M-05.4–6, M-14 | F3 |
| F9 | Suche (Overlay, Markierung, Sieb), Suchleiste | M-13 | F4, F5 |
| F10 | Einstellungen | M-20 | F4 |
| F11 | Export | M-15 | F5 |
| F12 | Git: Changes, Diff, Discard, Commit, Identität | M-19.1–5 | F3 |
| F13 | Branches, Sync, Konflikte | M-04.1–3, M-19.6–10 | F12 |
| F14 | Agenten: Zugriff, Install, Cursor, Requests | M-04.4/9, M-17 | F5 |
| F15 | Todos | M-04.5/6, M-06.5, M-16.3, M-18 | F2 |
| F16 | Schliessen/Beenden, Tab-Limit, Packaging | M-06.6, M-21.4 | alle |

## 10. Teststrategie

- **Rust:**
  - Bestehende Tests bleiben grün. Die kittest-Tests des Editors sichern das Verschieben nach editor-core ab.
  - Neue Unit-Tests in `underline-session` je portiertem Verhalten, benannt nach UC und Inventar
    (`uc005_m10_8_autosave_retries_after_failure`).
  - In `underline-editor-core` werden die Logik-Fälle aus den kittest-Tests als reine Tests übernommen (Tippen mit
    Escaping, Enter in Listen, Vim/Nano-Sequenzen).
  - Commands werden mit `tauri::test` (Mock-Runtime) getestet.
- **Frontend:** Vitest und `@testing-library/svelte` je Komponente. IPC wird mit `@tauri-apps/api/mocks`
  (`mockIPC`) gemockt und ist dank der generierten Typen typgeprüft.
- **E2E durch app-tester:**
  - Playwright gegen den Vite-Dev-Server (`http://localhost:1420`) im Mock-Modus. Ein Mock-Backend auf Basis der
    Bindings deckt Abläufe, Screenshots und Konsole ab.
  - Dazu Smoke-Tests der echten App mit `cargo tauri dev` und Blick in die Rust-Logs.
  - Echte E2E über `tauri-driver`/WebDriver nur auf Linux und Windows, weil es kein macOS-WebDriver gibt.
- **Leistung:**
  - NFR-002 (Benchmark aus F0, wiederholt in F3/F5), NFR-001 Startzeit, NFR-008 Agent-Änderung ≤ 1 s.
  - Werte gehen nach `paritaet.md`.
- **CI:**
  - Die bestehenden drei Checks.
  - Dazu `npm ci && npm run lint && npm run check && npm test` im `ui/` und die Prüfung, dass `bindings.ts`
    aktuell ist.
  - Systempakete für Tauri auf Linux: `libwebkit2gtk-4.1-dev` u. a.

## 11. Risiken

| Risiko | Bewertung | Gegenmassnahme |
|---|---|---|
| Tipp-Latenz NFR-002 über IPC/Webview, vor allem WebKitGTK | **hoch** | Spike F0 mit Messung; Variante B als Ausweg |
| Eigenes `contenteditable` für die Facade: IME, Autokorrektur, Selection auf 3 Webviews | **hoch** | `beforeinput`-Modell, Komposition durchlassen und bei `compositionend` committen, Spellcheck aus; frühe Tests auf allen OS |
| Offset-Einheiten: Rust Bytes/Chars vs. JS UTF-16 | mittel | eine Umrechnungsschicht an der Grenze; Tests mit Emoji/CJK |
| Startzeit NFR-001 (≤ 1 s) mit Webview | mittel | schlankes Bundle (Svelte), Fenster erst nach erstem Render zeigen; messen in F1 |
| Facade-DOM bei Tausenden Blöcken (Reflow, Minimap, Falten) | mittel | nur sichtbare Blöcke im DOM (Platzhalterhöhen), wie heute „nur sichtbare zeichnen“ |
| Zwei Kopien des Texts (nur Variante B): Agent-Edits, Rename, Änderungen von aussen | mittel | Revisionen und Rebase, Tests aus UC-019 A5/A6 |
| PlantUML-SVG mit Inline-Styles, Theme-Farben (D-10) | niedrig | wie heute nur hell/dunkel, ausser F-4 entscheidet anders |
| SVG-Sicherheit (Inline-SVG aus Diagrammen) | niedrig | als `<img>` über `ul-asset://` oder sanitisieren |
| Drag & Drop: Tauri-Dateidrop vs. HTML5-Drop schliessen sich auf Windows aus | niedrig | wie heute nur Dateien (FR-034 bleibt offen, D-15) |
| tauri-specta-Reife | niedrig | Rückfall ts-rs |
| Accessibility: das egui-Original hat keine; CM6 bringt sie mit, die Facade braucht ARIA | Chance | — |
| Es ist kein Spiel, Echtzeit-Rendering spielt keine Rolle. | — | — |

## 12. Architekturentscheide (AIUP)

Laut Projektregeln braucht ein Technologiewechsel ADRs, die über `/architecture-decision` entstehen. Geplant für
Phase 2:

- **ADR-020:** Tauri 2 mit Svelte/TS (ersetzt ADR-001)
- **ADR-021:** Editor im Webview, CM6 und eigene Facade, Rust-Editor-Kern (ersetzt ADR-012)
- **ADR-022:** IPC-Typen mit tauri-specta
- **ADR-023:** Tick-Modell und Events

Angepasst werden ADR-016 (rfd → Dialog-Plugin) und ADR-013 (CSP, `ul-asset://`).

Die AIUP-Dokumente (vision, requirements, use cases) bleiben unverändert. Einzig „native app“ in der Vision ist
eine Formulierung, die der Benutzer prüfen sollte.

Hinweis: ADR-012 beschreibt, dass die Facade den Block als Markup zurückschreibt. Der Code patcht aber Bytes an Ort
und Stelle (`fv:1561-1600`). Die Migration folgt dem Code.

## 13. Entscheidungen des Benutzers (2026-11-09)

| Frage | Entscheidung | Folge für den Plan |
|---|---|---|
| F-1 | Svelte 5 + TypeScript | wie in Abschnitt 2 |
| F-2 | `codemirror-vim` in der Quellansicht | **Gewollte Abweichung**: Die Quellansicht kann mehr Vim-Befehle als heute; Facade-Vim (Rust-Automat) und Quell-Vim verhalten sich nicht mehr identisch. Anzubinden: `:w`/`:q`/`:wq`/`:x` über `Vim.defineEx` an `tab_store`/`tab_close`, `"+`/`"*` an die Backend-Zwischenablage, Modus/Pending für die Statuszeile (M-11.4), Undo über die gemeinsame History. Nano bleibt der Rust-Automat. In `paritaet.md` eintragen. |
| F-3 | Portieren nach `underline-session`, egui-App unberührt | wie in Abschnitt 3 |
| F-4 | D-01–D-09 und D-11 beheben, Rest wie heute | Behebungen als gewollte Abweichung in `paritaet.md`; D-10, D-12–D-16 bleiben offene Punkte |

## 14. Vorgaben aus den Reviews des Grundgerüsts (Phase 2)

- **F0 / Webview-Adapter:**
  - `Modifiers` im Editor-Kern muss egui exakt nachbilden: `command` ist Ctrl ausserhalb von macOS und Cmd auf
    macOS, `mac_cmd` ist nur auf macOS gesetzt.
  - Das kommt als Konstruktor je Plattform mit Test, sonst weicht Nano/Vim im Webview still ab.
- **F1:**
  - Theme-Paletten als CSS-Variablen auf `<html>`, gesetzt vor dem ersten Paint.
  - Bedienelemente erreichen in allen 11 Themes mindestens 3:1 Kontrast (NFR-010), mit einem Test je Theme.
    Die Platzhalterfarben des Grundgerüsts werden nicht übernommen.
  - `color-scheme` folgt dem Theme.
  - DejaVu Sans und Monospace werden eingebunden (M-01.5).
  - Das Drittel der Höhe (M-02.1) wird auf den Mittelbereich zwischen Top-Bar und Statusleiste bezogen.
  - Solange `app_start` den letzten Workspace öffnet, gibt es einen Lade- bzw. Fehlerzustand, damit der leere
    Bildschirm nicht kurz aufblitzt.
- **F3 (aus dem Review von F2):** Die Tab-Leiste wird ein `tablist` mit Pfeiltasten und rovierendem `tabindex`,
  sobald der Editor als `tabpanel` dahintersteht. Gleiche Tab-Titel bleiben wie in egui ohne Ordner.
- **Laufend:** Die Liste der Commands steht dreifach (Builder, `build.rs`, Capability). Ein Test über die
  Command-Namen des Builders soll die Abweichung verhindern; er entsteht spätestens mit dem zweiten Command.
- **Ablösung:** Ob die Windows-Release-Binary ohne Konsole startet (`windows_subsystem`), muss mit dem
  `mcp`-Subcommand über Pipes auf Windows geprüft werden.

## 15. Ergebnis Spike F0 (2026-11-09)

Entschieden ist Variante A, Text im Backend ([ADR-024](../architecture/decisions/ADR-024-the-backend-owns-the-note-text.md)).
Messungen und Methode stehen in `spikes/f0-editor/RESULTS.md`.

Vorgaben für F3 und F5:

- Die Facade wird virtualisiert: ein Fenster von Blöcken im DOM, Platzhalterhöhen für den Rest, kein
  `content-visibility`.
- Enter und Backspace parsen in der Facade inkrementell.
- Der Tastenbefehl rechnet nur Edit und Patch. Highlighting, Treffer, Minimap und Sektionen kommen danach als
  Events.
- Die Bearbeitungsregeln der Facade (`facade_view.rs`) ziehen nach `underline-editor-core` um.
- Messpunkt: Nachmessen mit der Spike-Methode. Schafft die Facade weniger als 90 % der Tasten im nächsten Frame,
  wechseln wir per ADR zu Variante B.
- In F3/F5 wird nachgeholt, was der Spike nicht getestet hat:
  - echte Tasten über `xdotool` (`sudo apt install xdotool`), Tottasten und Compose;
  - eine AsciiDoc-Notiz;
  - Bearbeiten über das Tippen hinaus und Scrollen im virtualisierten Bereich.
- D-17 und D-18 werden in der Tauri-App behoben.

## 16. Offene Entscheidungen (ursprüngliche Fragen)

- **F-1 Frontend:** Svelte 5 + TS (empfohlen), React + TS oder Leptos?
- **F-2 Vim-Verhalten:** der eigene Rust-Automat in beiden Ansichten (empfohlen, exakte Parität) oder
  `codemirror-vim` in der Quellansicht (mehr Befehle, anderes Verhalten als in der Facade)?
- **F-3 App-Logik auslagern:**
  - Empfohlen: als Portierung nach `underline-session`, mit Tests; die egui-App bleibt unberührt als Referenz.
  - Alternative: die egui-App so umbauen, dass sie `underline-session` nutzt. Das ergibt eine Wahrheit, aber die
    Referenz ändert sich, und das App-Crate hat keine Tests.
- **F-4 Bekannte Defekte D-01 bis D-16:** 1:1 übernehmen oder in der Tauri-App beheben? Empfohlen: D-01 bis D-09
  und D-11 beheben, als gewollte Abweichung in `paritaet.md`; den Rest (D-10, D-12 bis D-16) wie heute lassen und
  als offene Punkte führen.

## 17. Umbau auf Variante B (2026-11-10)

Der Messpunkt von F5b wurde verfehlt (Q-24); der Benutzer hat Variante B gewählt
([ADR-025](../architecture/decisions/ADR-025-the-webview-owns-the-note-text-with-a-wasm-editor-core.md), ersetzt
ADR-024). F5b (Variante A) ist als Basis committet; die Regeln, das Reparse und der Edit sind reine Rust-Logik und
ziehen unverändert um. Reihenfolge, je Schritt ein Commit mit allen Prüfungen grün:

- **B1 `underline-markup`:** neue wasm32-fähige Crate mit `NoteFormat` (und `InputMode`, falls der Editor-Kern es
  braucht), `blocks`, `asciidoc`, `sections`, `formatting`, `link_at`. `underline-core` und `underline-facade`
  re-exportieren alles unter den bisherigen Pfaden; die egui-App, MCP und Wiki bleiben unverändert. Die Tests ziehen
  mit. Prüfung: `cargo check -p underline-markup --target wasm32-unknown-unknown`, ganzer Workspace grün.
- **B2 Facade-Regeln in den Editor-Kern:** `rules`, `reparse`, `edit` und der reine Teil von `input` (Splice der
  Views ohne Highlighting, Bilder, Diagramme) nach `underline-editor-core::facade`. Das Bauen der Views wird hinter
  ein Trait gelegt; die Anreicherung (Code-Farben, Figuren, Asset-URLs) bleibt im Backend. `mac: bool` statt
  `Platform::current()`. Kein `Instant`, kein `Mutex` im portablen Teil. `underline-session` nutzt den Kern weiter
  (Spiegel), die Tests bleiben grün.
- **B3 `underline-editor-wasm`:** `wasm-bindgen` (exakt gepinnt, CLI gleiche Version lokal unter `.tools/`), Build
  per `npm run wasm` vor `vite build`; `cargo tauri build` ruft es über `beforeBuildCommand` auf.
- **B4 Sync:** Revisionen je Tab im Backend; Edits `{base, from, removed, inserted}` (Zeichen) nach dem Frame in einem
  Befehl; Backend-Änderungen (Agent, Aussenänderung, Umbenennen, Neu laden) als Event mit Rebase im Webview;
  Backend-Ergebnisse (Highlighting, Sektionen, Treffer, Minimap, Figuren) tragen die Revision. Vor Speichern beim
  Schliessen und vor jedem Befehl, der den Text liest, schickt das Fenster offene Edits. Tests: gleichzeitige Edits,
  Agent-Edit während des Tippens, Schliessen mit unbestätigten Edits, Hash-Vergleich.
- **B5 Quellansicht und Facade auf WASM, Messpunkt:** beide Ansichten nutzen den Kern im Webview; Nachmessen mit der
  Spike-Methode. Ziel p99 ≤ 16 ms und ≥ 90 % der Tasten im nächsten Frame. Danach Reviews (rust, ux), app-tester,
  F5b abhaken, Commit.
