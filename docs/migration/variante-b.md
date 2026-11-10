# Variante B: Entwurf für die Schritte B3 bis B5

Bezug: [ADR-025](../architecture/decisions/ADR-025-the-webview-owns-the-note-text-with-a-wasm-editor-core.md),
`plan.md` Abschnitt 17. Stand 2026-11-10, nach B1 (`underline-markup`) und B2 (`underline-editor-core::facade`,
Commit 9a97580). Nur Entwurf; die Fragen am Schluss gehören nach `offene-fragen.md`.

## 0. Wo der Text des offenen Tabs heute berührt wird

| Stelle heute (Tauri) | Was | In Variante B |
|---|---|---|
| `ed_input`, `ed_apply`, `ed_select`, `ed_fold`, `ed_keys_status`, `ed_clipboard` (`commands.rs:387–576`, `session/editing.rs`) | Taste, Vim-Änderung, Auswahl, Falten, Statuszeile, Zwischenablage | im WASM; Commands entfallen in B5 |
| `fa_open`, `fa_input`, `fa_clipboard`, `fa_keys_status` (`commands.rs:285–384`, `session/facade/input.rs`) | Facade-Views, Eingaben | im WASM; `fa_open` geht in `ed_open` auf |
| Tick: `autosave` (`session/editing.rs:619`), `Tab::flush` (`session/tab.rs:155`), `flush_all` | Speichern | auf dem Spiegel |
| Tick: `handle_outside_changes` → `outside_change` (`session/workspace.rs:854`, `tab.reload` :894) | Aussenänderung, A9 | Backend-Änderung `Reload` mit Sperre (3.6) |
| Tick: `editor_events` (Sektionen, Nano-Treffer), `highlight_event`, `facade_events` | Ergebnisse | Highlighting bleibt; Sektionen/Treffer/Views ins WASM (4) |
| noch nicht portiert: Agent (ADR-018, F14), Umbenennen (F8), Konflikt/Verwerfen/Branch (F12/F13) | Backend ändert den Tab | Backend-Änderung über den Spiegel (3.6) |

Referenz egui: Agent `app.rs:3445–3515` und `source_view.rs:285–292`; Aussenänderung `app.rs:1776–1816` und
`source_view.rs:401–416`; Umbenennen `app.rs:553–566`, `app.rs:5056`; Konfliktwahl `app.rs:3019`;
Verwerfen/Branch `app.rs:5589`, `app.rs:4082`; Cursorregel `source_view.rs:422–444`.

## 1. Crates und DTOs

**Entscheid: neue Crate `crates/underline-editor-view`** (wasm32-fähig), dazu `crates/underline-editor-wasm`.

```
underline-markup  <-  underline-editor-core (+ sync)  <-  underline-editor-view (DTOs, EditorTab)
                                                            ^                 ^
                                        underline-editor-wasm (cdylib)   underline-session (Spiegel)
```

- **`underline-editor-core`** erhält das Modul `sync`: Edit-Typ in Zeichen, Changeset, `transform`, `hash`,
  `Server` (Spiegel mit Log) und `Client` (Schatten, unterwegs, Puffer). Ohne serde, ohne Uhr.
- **`underline-editor-view`** (Abhängigkeiten: editor-core, markup, serde; `specta` optional als Feature wie heute in
  der Session):
  - `editor_view.rs` und `facade_view.rs` ziehen aus `underline-session` hierher. Die Session behält nur
    `NotePropertiesView`, `TabPropertiesView` und `CloseQuestion`.
  - Neu: `sync_view.rs` (Sync-DTOs, 3.4) und `enrich_view.rs` (Anreicherung, 2.3).
  - **`EditorTab`**: der Tab, wie das Fenster ihn hält. Er umfasst `SourceEditor`, `Keys`, die Falten (aus
    `session/folding.rs`, mit `now_ms: f64` statt `Instant`), den Facade-Zustand, den `Client` und den View-Macher
    `CachedViews`.
    - Er antwortet mit den heutigen DTOs (`EditorPatch`, `FacadePatch`, `BlocksView`, `SectionsView`, `FoundView`).
    - Dazu zieht der reine Teil der Adapter hierher: `after_input`/`patch` aus `session/editing.rs`, der Teil von
      `session/facade/input.rs` ohne Highlighting, aus `facade/build.rs` `WebWalker`, `run_view`, `prefix_text`,
      die Art-Abbildung in `view()` und die Nummerierung der Captions.
- **`underline-session`** re-exportiert `underline_editor_view::{editor_view, facade_view}` unter den bisherigen
  Pfaden. Commands und `bindings.rs` ändern ihre Importe nicht.
- **TypeScript:** Es bleibt ein Generator, nämlich tauri-specta.
  - DTOs, die nur über die WASM-Grenze gehen (z. B. `RemoteApplied`, `TabViews`), werden in `bindings.rs` mit
    `Builder::typ::<T>()` exportiert (vorhanden in tauri-specta 2.0.0-rc.26, `builder.rs:275`).
  - Damit stehen alle Typen in `bindings.ts`, und der bestehende Test `checked_in_bindings_are_current` deckt sie mit.
- **WASM-Signaturen:** Das WASM tauscht JSON-Strings aus (`&str` → `String`), wie im Spike gemessen (≤ 3 ms in der
  Facade, RESULTS.md Tabelle „Owner“).
  - Ein handgeschriebener Wrapper `ui/src/lib/editor/core.ts` ist die einzige Stelle, die `pkg/` importiert. Er
    typisiert mit `bindings.ts`.
  - Verworfen: `tsify`/`serde-wasm-bindgen`. Das wäre ein zweiter Typgenerator, und der Zeitgewinn ist nicht
    gemessen.

## 2. WASM-API (`underline-editor-wasm`)

`wasm-bindgen = "=0.2.130"` steht exakt gepinnt in `[workspace.dependencies]`; Cargo.lock hat schon 0.2.130. Die CLI
hat dieselbe Version (5). Die Crate ist `crate-type = ["cdylib", "rlib"]`, damit clippy und Tests nativ laufen.

### 2.1 Handle je Tab

`WasmTab` hält ein `EditorTab`.

- Ein Registry-Modul `ui/src/lib/editor/tabs.ts` legt es beim ersten Zeigen eines Tabs an. Es lebt über
  Ansichtswechsel und Tabwechsel hinweg.
- Erst nach erfolgreichem `tab_close` wird es mit `free()` freigegeben.
- Vor dem ersten Zeigen hält nur das Backend den Text. Es gibt dann keine offenen Edits.

### 2.2 Funktionen (spiegeln die heutigen Commands, damit `EditorBackend`/`FacadeBackend` gleich bleiben)

| WASM (`camelCase`) | ersetzt | Antwort |
|---|---|---|
| `WasmTab.open(TabOpenView)` | `ed_open` + `fa_open` | Handle |
| `state()` / `facadeState()` | `ed_open` / `fa_open` | `EditorState` / `FacadeState` |
| `input(EditorInput)` | `ed_input` | `EditorPatch` |
| `apply(base, VimChange[], anchor, head, newStep)` | `ed_apply` | `EditorPatch \| null` |
| `select(rev, anchor, head)` | `ed_select` | Ort (`string`) |
| `fold(line)` | `ed_fold` | `EditorPatch` |
| `keysStatus(VimState \| null)` / `facadeKeys()` | `ed_keys_status` / `fa_keys_status` | `KeysStatusView \| null` |
| `clipboard(action, text \| null)` | `ed_clipboard` ohne System-E/A | `{ copied, patch }` |
| `facadeInput(at, input, rows, clipboard)` | `fa_input` | `FacadePatch` |
| `facadeClipboard(at, action, plain, text)` | `fa_clipboard` ohne System-E/A | `{ copied, patch }` |
| `views(nowMs)` | Events `ed-sections`, `ed-found` | `TabViews { sections?, found? }` (lokale Revision) |
| `enrich(EnrichView)` | Event `ed-blocks` | `BlocksView[]` (lokale Versionen) |
| `setView(EditingViewName)`, `setMode(InputModeName)`, `setLooks(theme, dark)` | Zustand der Session | – |
| `takeOutbox()` | – | `SyncBatch \| null` |
| `acknowledge(SyncAnswer)` | `text-replaced` + `ed_open` | `RemoteApplied` (3.3) |
| `syncedRev()`, `inStep(rev)`, `text()`, `hash()` | – | Sync-Zustand |

- **Zwischenablage:** Das Fenster liest die Zwischenablage mit `clip_read_text` und schreibt sie mit
  `clip_write_text`; ADR-014 bleibt. Danach ruft es das WASM.
- **`EditorPatch.base/rev`, `FacadePatch.base/rev` und die Block-`version`** sind ab B5 **lokale** Revisionen des
  WASM-Dokuments. `SourceView.#apply` und `FacadeView` prüfen sie unverändert.
- **Ergebnisse des Backends** tragen die **Sync-Revision** (3.1). Nur `takeColors` fragt dafür `core.inStep(rev)`
  statt `#inStep`.

### 2.3 Facade-Views ohne Backend und die Anreicherung

`CachedViews` implementiert `MakeViews` (B2) aus einem Anreicherungs-Cache. Schlüssel sind Inhalte, nicht Revisionen:

| Block | Schlüssel | Ohne Eintrag zeigt das WASM |
|---|---|---|
| Code (kein Diagramm) | `fnv(theme, language, code)` | `colors: None`, beim Ersetzen alte Spans über `carry_spans` (wie heute in A, „recolor“) |
| Diagramm | `fnv(language, code, dark)` | das letzte Bild des Blocks (`last_pictures`, M-09.7, `build.rs:627–671`), sonst `message_figure("Rendering the {name} diagram…", false)` |
| Bild | `target` (relativ zur Notiz) | `FigureView { loading: "Loading {target}…" }` |
| Tabelle/Bild mit Caption | – | Nummer lokal („Figure n.“, „Table n.“), wie `build.rs` |

- **Hash:** `fnv` ist FNV-1a 64 aus `editor-core::sync::hash`. Bisher nimmt `colors_key` `DefaultHasher`
  (`build.rs:597`); dessen Werte sind über Rust-Versionen nicht zugesagt. Ab B3 nehmen beide Seiten FNV.
- **Links:** `RunView.link` bleibt das rohe Ziel. Aufgelöst wird beim Folgen (`ed_follow_link`, F7).
- **Asset-Pfade:** Sie kommen fertig aus dem Backend (`file_asset_path` mit `?v=`). `assetUrl()` bleibt.
- **`EnrichView { note, rev, clear, colors: [key, CodeColors|null][], figures: [key, FigureView][] }`**
  - Das Backend schickt es als Event `ed-enrich`.
  - Es parst dazu den Spiegel. Gesendet werden die Schlüssel, die der Tab noch nicht hat, und Bilder und Diagramme,
    deren Zustand sich geändert hat (Diagramm fertig, Bilddatei nach `IMAGE_FRESH`).
  - Bei Themewechsel oder Wiederöffnen setzt das Backend `clear`.
  - `ed_open` liefert die Anreicherung der ganzen Notiz mit. Die erste Facade sieht dann aus wie heute, ohne Flackern.
- **Mergen:**
  - `enrich()` legt die Einträge in den Cache. Er ist begrenzt wie heute auf 4096 Code-Einträge (`CODE_COLORS_KEPT`).
  - Ein Eintrag mit kleinerer `rev` als der gespeicherte zum selben Schlüssel wird verworfen. Das betrifft nur Bilder,
    deren Datei sich ändert.
  - Danach macht `enrich()` die Views der betroffenen Blöcke neu und gibt die Splices „an Ort“ zurück
    (`splices`, Fall `shift == 0`).
  - Revisionsmarken werden nicht umgerechnet, weil die Schlüssel Inhalte sind.
- **Diagramm-Priorität:** Die Reihenfolge nach Blockindex (`ahead`) bestimmt das Backend aus seinem eigenen Parse.

## 3. Sync-Protokoll

### 3.1 Revisionen

- Jeder Tab hat im Backend einen eigenen Zähler `sync_rev: u32`. Er beginnt bei `ed_open`/Tab-Öffnen mit 1 und steigt
  um 1 je angewandtem Stapel des Fensters und je Backend-Änderung. Lücken sind erlaubt (3.6).
- Er ist getrennt von `Document::revision`, der lokal im WASM weiterzählt.
- Ein Überlauf führt zu Resync (3.10) und kommt praktisch nicht vor.
- Das Fenster kennt `synced` (die letzte bestätigte Sync-Revision), `inflight` (gesendet, unbestätigt) und `buffer`
  (seit dem Senden).

### 3.2 Edit-Format: Zeichen des Ropes

`SyncEdit { at: u32, remove: u32, insert: String }` in **Zeichen (Unicode-Skalarwerte)**. Die Edits eines Stapels sind
sequenziell: Jeder gilt auf dem Ergebnis des vorigen, wie im Spike (`mirror.rs`). `SyncBatch { base: u32, edits }`.

Begründung gegen UTF-16 bzw. Web-Einheiten:

1. Beide Enden halten ein ropey-Rope, das nach Zeichen indiziert ist. Es braucht keine Umrechnung, und kein Edit kann
   ein Surrogatpaar teilen.
2. Web-Einheiten zählen `\r\n` als 1 (`utf16.rs:8–13`). Ein Edit zwischen `\r` und `\n` wäre nicht ausdrückbar, und
   das Backend hält die Bytes der Datei (NFR-007).
3. JavaScript rechnet nie damit. Die Edits entstehen im WASM und werden im Backend angewandt; TS reicht nur JSON
   weiter.

Byte-Bereiche des Backends (Umbenennen, Agent: `apply_edits` nimmt Bytes, `source_view.rs:425–427`) rechnet der
Spiegel beim Anlegen der Backend-Änderung in Zeichen um.

### 3.3 Ablauf: Pull über einen geordneten Kanal je Tab

Tauri liefert Events per Script-Auswertung und Command-Antworten über das IPC-Protokoll. Ihre Reihenfolge ist nicht
zugesagt.

**Entscheid:** Edits des Backends kommen **nur in der Antwort von `ed_sync`**. Ein Event ist bloss die Klingel. Je Tab
ist höchstens ein `ed_sync` unterwegs. Das kostet bei Backend-Änderungen einen Roundtrip (2–5 ms), weit unter
NFR-008 (1 s).

1. **Eingabe:** Das WASM wendet sie an, legt die Edits in `buffer` und gibt den Patch zurück. TS ruft
   `sync.schedule()`.
2. **`schedule`:** Ist nichts geplant, wird nach dem Frame gesendet: `requestAnimationFrame` + `setTimeout(0)`, die
   Methode des Spikes (RESULTS.md:60–61). Ist ein Aufruf unterwegs, wird nach seiner Antwort gesendet.
3. **Senden:**
   - `takeOutbox()` verschiebt `buffer` nach `inflight` und gibt `{ base: synced, edits }` zurück, oder `null`, wenn
     nichts offen ist und keine Klingel wartet.
   - Danach folgt `ed_sync(note, batch)`. Ein leerer Stapel ist erlaubt (Abholen).
4. **Backend:**
   - Es transformiert `inflight` über die eigenen Änderungen seit `base`, wendet das Ergebnis an und hängt es ans Log.
   - Es setzt die Autosave-Uhr (`Tab::changed`) und weckt den Tick.
   - Antwort: `SyncAnswer { rev, hash, remote: RemoteOp[], resync: bool }`. Dabei sind `remote` die Änderungen des
     Backends seit `base` in Log-Reihenfolge, je mit `rev`.
5. **Fenster:** `acknowledge(answer)`:
   - transformiert `remote` über `inflight` und `buffer`, wendet es an und transformiert `buffer` über `remote`;
   - setzt `synced = rev`, leert `inflight` und prüft den Hash (3.10);
   - gibt `RemoteApplied { source: EditorPatch?, facade: FacadePatch?, reloaded, desync }` für die gezeigte Ansicht
     zurück;
   - die Ansicht wendet den Patch über ihre normale Warteschlange an (`SourceView.#enqueue`, ohne `scroll`).
6. **Klingeln:**
   - `ed-changed { note, rev }` ersetzt `text-replaced`. Ist `rev > synced`, sendet das Fenster sofort, auch leer.
   - `ed-flush { note? }` heisst: sofort senden, auch leer (3.9).

### 3.4 Commands und Events (ab B4/B5)

| Neu | Typ |
|---|---|
| `ed_open(note) -> TabOpenView` | `{ note, rev, text, format, view, anchor, head, enrich: EnrichView }`; Cursor des Öffnens (Sprung, `focus_end`) in Web-Einheiten |
| `ed_sync(note, SyncBatch) -> SyncAnswer` | synchron wie heute `ed_input` (kurz unter dem Lock, geordnet) |
| `ed_resync(note, text) -> SyncAnswer` | Fenster-Text ersetzt den Spiegel (3.10) |
| Event `ed-changed`, `ed-flush`, `ed-enrich` | ersetzen `text-replaced`, `ed-blocks` |
| bleiben | `ed_viewport`, `ed-highlight` (rev = Sync-Revision), `clip_read_text`, `clip_write_text`, `tab_set_view` |
| entfallen in B5 | `ed_input`, `ed_apply`, `ed_select`, `ed_fold`, `ed_keys_status`, `ed_clipboard`, `fa_open`, `fa_input`, `fa_clipboard`, `fa_keys_status`, Events `ed-sections`, `ed-found`, `ed-blocks`, `text-replaced` |

`RemoteOp { rev, change: Edits { edits: SyncEdit[], sealed: bool } | Reload { text } }`.

### 3.5 Transformation (ein Server, kein TP2 nötig)

Intern wird jeder Stapel zu einem Changeset über seinem Basistext: eine geordnete Folge von Behalten, Löschen und
Einfügen, wie `ChangeSet` von CodeMirror.

- Ein Edit ist dabei **Löschbereich `[at, at+remove)` plus Einfügen bei `at`**.
- `transform(a, b, seite)` liefert `a'`, das nach `b` gilt. Die Seite ist **Backend** oder **Fenster**.
- **Bei Gleichstand gewinnt das Backend links.**

Regeln für die Einfügestelle `p` von `a` gegenüber `b` (mit Δb = |b.insert| − b.remove):

| Lage von `p` | `p'` |
|---|---|
| `p < b.at` | `p` |
| `p > b.at + b.remove` | `p + Δb` |
| `b.at < p < b.at + b.remove` (im Gelöschten) | `b.at + \|b.insert\|` (hinter den Text von `b`) |
| `p == b.at` (Gleichstand) | Seite Backend: `p`; Seite Fenster: `b.at + \|b.insert\|` |
| `p == b.at + b.remove`, `b.remove > 0` | `b.at + \|b.insert\|` |

- **Löschen:**
  - `a'` löscht die Zeichen von `a` ohne die schon von `b` gelöschten.
  - Liegt das Einfügen von `b` im Löschbereich von `a`, bleibt es stehen. `a'` zerfällt dann in zwei Teile davor und
    danach.
  - Eingefügter Text der anderen Seite geht so nie verloren.
- **Herleitung:** Fallbeispiele mit Gleichstand, `b` in `a`, `a` in `b` und Rand an Rand konvergieren von Hand
  gerechnet. Verbindlich ist der Eigenschaftstest (6): `apply(apply(t, a), b') == apply(apply(t, b), a')`.
- **Cursor:** Die Regel „Backend links“ passt zur egui-Regel, nach der ein Einfügen am Cursor den Cursor dahinter
  schiebt (`source_view.rs:430–431`, `at >= end`).
- **Listen:** Liste über Liste ist die übliche Doppelschleife über die Changesets. Bei einem Server reicht TP1.

### 3.6 Änderungen des Backends

| Auslöser (egui) | Änderung | Undo im Fenster |
|---|---|---|
| Agent (`app.rs:3498–3515`) | `Edits { sealed: true }` | eigener Schritt: `seal`, anwenden, `seal` (`source_view.rs:288–292`, UC-019 BR-007) |
| Umbenennen (`app.rs:553–566`, `:5056`), Konfliktwahl (`app.rs:3019`) | `Edits { sealed: false }` | wie `apply_edits`: ohne Siegel (`source_view.rs:422–444`) |
| Aussenänderung (`app.rs:1813`), Verwerfen (`:5589`), Branch/Sync (`:4082`) | `Reload { text }` | History neu, Cursor gleiche Zeile/Spalte (`SourceEditor::reload`, `source.rs:286–296`; egui `source_view.rs:401–416`) |

- **Session-API:** `Session::backend_edit(note, edits_in_bytes, sealed)` und `Tab::reload(text)` legen die Änderung
  ins Log und klingeln mit `ed-changed`. F8, F13 und F14 nutzen nur diese API.
- **Agent:**
  - Die egui-Regel UC-019 BR-004 prüft gegen den Text mit allen Tasten (`plan_for_tab(base = on_disk, current)`).
    Darum gilt: Eine Agent-Anfrage auf einem offenen Tab klingelt zuerst mit `ed-flush { note }`.
  - Der Tick wendet sie erst an, wenn danach ein `ed_sync` dieses Tabs kam, spätestens nach 300 ms.
  - Danach prüft er gegen den Spiegel, wendet an und speichert sofort wie egui (`save_now`).
  - Ein Restfenster von einem Roundtrip bleibt (8).
- **Sperre beim Reload:** Ein Stapel mit `base` vor einem `Reload` lässt sich nicht transformieren. Das Backend macht
  dann UC-004 A9 nachträglich, so wie egui, wenn die Taste vor dem Watcher-Ereignis kam (`app.rs:1789–1810`):
  - Es stellt das Dokument vor dem Reload wieder her und wendet den Stapel an.
  - Es legt die Aussenversion als Kopie ab (`outside_copy_path`).
  - Es setzt den A9-Status („… was changed outside while you were editing; your text stays, the other version is
    kept as {name}“).
  - Die `rev` des Reloads entfällt; es entsteht eine Lücke im Zähler.
  - Das Fenster wendet ein `Reload` in einer Antwort nur an, wenn sein `buffer` leer ist. Sonst überspringt es das
    Reload und alles danach, setzt `synced` auf die `rev` davor und sendet den Puffer mit dieser `base`.
  - Bis das Fenster eine `rev` ≥ Reload bestätigt, antwortet der Tab Agenten mit `Busy`, wie bei `tab.saving`
    (`app.rs:3493`). So liegt nie eine Agent-Änderung hinter einem Reload, der wieder zurückgenommen wird.
- **Log:** Das Log wird bis zur jeweils letzten empfangenen `base` gekürzt. Höchstens 1024 Einträge; ältere `base`
  führt zu Resync (3.10).

### 3.7 Undo

- Das Undo liegt nur im WASM (`Document` mit `seal`/`merges`).
- Lokale Edits stehen in der Reihenfolge des Tippens in der History. Backend-Änderungen kommen transformiert hinter sie
  und mit den Siegeln aus 3.6. Die History bleibt dadurch linear zum Fenstertext, ohne dass etwas umgerechnet wird.
- Ctrl+Z nach einer Agent-Änderung nimmt die Agent-Änderung zurück, wie in egui. Das Ergebnis ist ein normaler lokaler
  Edit.
- Der Spiegel führt keine History. Er hält Rope, `sync_rev` und Log, kein `Document`.

### 3.8 Cursor und Auswahl

- **Backend-Änderung:** Cursor und Anker werden mit der egui-Regel verschoben (`source_view.rs:428–437`):
  - `at ≥ end` → `+Δ`;
  - `start < at < end` → `start + min(|insert|, at − start)`;
  - sonst bleibt er.
  - Es wird nicht gescrollt.
- **Reload:** gleiche Zeile und Spalte.
- **Facade:** Sie nutzt dieselbe Auswahl. Die Lücke nach Enter folgt aus `gap_at` am neuen Cursor (egui
  `facade_view.rs:783`).
- **Vim:** Noch nicht gesendete Vim-Änderungen der Quellansicht (A-27) verschiebt `SourceView.#apply` wie heute
  über den Patch.

### 3.9 Flush vor Speichern, Schliessen und lesenden Befehlen

- **`flush(note)` / `flushAll()`** in `ui/src/lib/editor/sync.ts`: senden, bis `buffer` und `inflight` leer sind.
- **Der Helfer `flushed(fn)`** umschliesst jeden Command, der den Text liest oder den Tab ersetzt:
  - heute: `tab_close`, `tab_store` (auch `EditorCommandName.Store`/`Close` aus Patches), `ws_open`, `ws_start`,
    `ws_create`, `ed_follow_link` (F7: Link und Überschrift werden im Spiegel gesucht, die Stelle trägt die Revision);
  - später: `note_rename`, `note_delete`, `export_run`, `git_*`, `sync_request`, `conflict_choose`;
  - neue lesende Commands kommen in diese Liste (Review-Regel).
- **Fenster schliessen:**
  - `CloseRequested`/`ExitRequested` (`main.rs:118`, `:159`) verhindert das Schliessen und klingelt mit
    `ed-flush {}`.
  - Das Fenster ruft `flushAll()` und dann den neuen Command `app_request_close`, der den heutigen
    `ask_before_closing` ausführt.
  - Antwortet das Fenster nicht innerhalb von 1 s, macht das Backend auf dem Spiegel weiter (Log-Eintrag).
- **`RunEvent::Exit`:** speichert den Spiegel.
- **Autosave:** Er speichert immer den Spiegel. Spätere Edits machen den Tab wieder „unstored“ (höchstens ein Frame
  plus Roundtrip später als in A).

### 3.10 Fehler: Desync und Resync

- **Hash-Vergleich:** `hash` ist FNV-1a 64 über die UTF-8-Bytes, als 16 Hex-Zeichen, in `editor-core::sync`.
  - Das Backend schickt den Hash seines Textes bei `rev`. Das WASM vergleicht ihn mit seinem Schatten, also dem Text
    bei `synced`, den es nachführt (ropey-Klon).
  - Kosten etwa 0,5 ms bei 450 K Zeichen nach dem Frame; wird in B5 gemessen.
- **Desync** liegt vor bei abweichendem Hash, `resync: true`, einem Edit ausserhalb des Textes (Backend lehnt den
  ganzen Stapel ab) oder einem Überlauf.
  - Das Fenster ruft dann `ed_resync(note, text)`.
  - **Der Text des Fensters gewinnt** (ADR-025: das Fenster besitzt den Text). Das Backend ersetzt den Spiegel durch
    gemeinsames Präfix und Suffix plus Mitte, damit wenig Highlighting verworfen wird.
  - Das Backend schreibt eine Zeile ins Rust-Log. Eine Meldung an den Benutzer gibt es nicht (offene Frage).

## 4. Was im Backend bleibt, und die Revisionsmarken

| Ergebnis | Ort in B | Marke, Behandlung im Fenster |
|---|---|---|
| Syntax-Farben der Quellansicht (syntect, ADR-010) | Backend, Tick, `ed_viewport` | Sync-`rev`; siehe unten |
| Code-Farben, Diagramme, Bilder der Facade | Backend, `ed-enrich` | Inhaltsschlüssel, keine Umrechnung (2.3) |
| Link-Auflösung, Assets, Dateieigenschaften | Backend, auf Anfrage | – |
| Agent-Cursor (F14) | Backend | Sync-`rev`, wie die Farben |
| Sektionen, Falten (M-08.8) | **WASM** (`views(nowMs)`, 300 ms wie `SECTIONS_EVERY`) | lokale `rev`, wie heute `#inStep` |
| Nano-Treffer (M-11.3) | **WASM** (`find_all`) | lokale `rev` |
| Facade-Blöcke, Captions, Lücke, Statuszeile, Ort | **WASM** | lokal |
| Minimap | Fenster (aus CM-Dokument und Farben, wie heute) | – |

- **Falten wandern ins WASM.** Sie hängen an jeder Taste: Up/Down überspringen gefaltete Zeilen, ein Cursor im
  Verborgenen faltet auf (`folding.rs:132`, `:206`). Ein Roundtrip wäre zurück im Tastenpfad.
- **Regel für Sync-markierte Ergebnisse:**
  - `rev == synced` und nichts offen → anwenden.
  - `rev > synced` → als neuestes je Art halten, bis `acknowledge` die `rev` erreicht. Das passiert, wenn das Event
    vor der Antwort kommt.
  - `rev < synced` → verwerfen; nach jedem Stapel rechnet das Backend neu.
  - Bis dahin verschiebt CodeMirror die gezeigten Dekorationen mit jeder Transaktion (`highlight.ts`, `colorField`).
  - Ergebnisse durch die offenen Edits zu verschieben (`changesSince` → `ChangeSet` → `RangeSet.map`) ist in B5 nicht
    vorgesehen. Es kommt nur, wenn die Messung sichtbaren Farbverzug zeigt (8).
- **Workspace-Suche (M-13, F9):** Wo ihre Markierung entsteht, entscheidet F9. Der Entwurf empfiehlt das WASM mit der
  Anfrage als Eingabe.

## 5. Build-Pipeline

- **CLI:** `cargo install wasm-bindgen-cli --version 0.2.130 --locked --root .tools` im Repo-Wurzelverzeichnis.
  - `/.tools/` kommt in `.gitignore`.
  - Das Target dazu: `rustup target add wasm32-unknown-unknown`.
- **Skript `ui/scripts/wasm.js`** (Node, plattformneutral wie `scripts/tauri.js`):
  1. Es sucht die CLI unter `.tools/bin/wasm-bindgen[.exe]`, sonst in `UL_WASM_BINDGEN`, sonst im `PATH`.
  2. Es vergleicht `--version` mit der Version von `wasm-bindgen` in `Cargo.lock`. Weicht sie ab, bricht es mit der
     Installationszeile ab.
  3. `cargo build -p underline-editor-wasm --release --target wasm32-unknown-unknown`.
  4. `wasm-bindgen --target web --out-dir ui/src/lib/wasm/pkg target/wasm32-unknown-unknown/release/underline_editor_wasm.wasm`.
  - Kein `wasm-opt`: etwa 0,7–1 MB, wie in ADR-025 angenommen.
- **npm-Skripte:**
  - `"wasm": "node scripts/wasm.js"`;
  - die Hooks `"predev"`, `"prebuild"`, `"precheck"` und `"pretest"` rufen jeweils `npm run wasm`. Ohne Änderung ist
    cargo inkrementell nach etwa 1 s fertig.
  - `lint` und `format:check` brauchen das Paket nicht.
- **`tauri.conf.json`:**
  - `beforeDevCommand` und `beforeBuildCommand` bleiben `npm run dev` bzw. `npm run build`; die npm-Hooks bauen das
    WASM vorher.
  - Die CSP (`csp` und `devCsp`) erhält `script-src 'self' 'wasm-unsafe-eval'` und `connect-src 'self' …` für den
    `.wasm`-Fetch, wie im Spike (RESULTS.md:263). Es wird nichts aus dem Netz geladen, ADR-023 bleibt erfüllt. Die
    CSP-Änderung wird in ADR-023 nachgetragen (8).
- **Das Paket wird erzeugt, nicht eingecheckt.**
  - `ui/src/lib/wasm/pkg/` kommt in `.gitignore`, `.prettierignore` und die ESLint-`ignores`.
  - Grund: Das Binary ändert sich mit jedem Rust-Commit, eingecheckt würde es veralten.
- **Laden:**
  - Browser und Tauri: `core.ts` ruft bei App-Start `init()` des Pakets ohne Warten auf. Vite bündelt die `.wasm` über
    `new URL(…, import.meta.url)`.
  - `SourceView.open` und `FacadeView` warten auf dieses Promise.
  - Vitest: `src/test-setup.ts` ruft `initSync({ module: readFileSync(…_bg.wasm) })`.
  - `svelte-check` braucht nur die `.d.ts` aus `pkg/`.
- **CI (`ci.yml`, alle drei OS):**
  - `dtolnay/rust-toolchain` mit `targets: wasm32-unknown-unknown`.
  - `.tools` per `actions/cache` mit dem Schlüssel `wasm-bindgen-cli-0.2.130-${{ runner.os }}`; sonst `cargo install`
    wie oben.
  - Danach der Frontend-Block unverändert, dank der Hooks.
  - Zusätzlich `cargo check --target wasm32-unknown-unknown -p underline-markup -p underline-editor-core -p underline-editor-view -p underline-editor-wasm`.
  - `cargo clippy --workspace` baut die WASM-Crate nativ als rlib.

## 6. Tests

**Nativ (Rust), `editor-core::sync`:**

- Tabellentests aller Zeilen von 3.5 mit beiden Seiten, Namen `adr025_…`.
- **Konvergenz, eigenschaftsbasiert, ohne neue Abhängigkeit:** ein eigener xorshift-PRNG mit festen Seeds. Bei einem
  Fehler wird der Seed ausgegeben.
  - Texte mit Emoji, `\r\n`, leer.
  - Ein Simulator mit Server und Client und zufälliger Verschachtelung von lokalem Edit, Senden, Backend-Edit,
    Backend-Empfang, Antwort-Empfang (auch verspätet) und Reload.
  - Am Ende, wenn nichts mehr unterwegs ist: Text und Hash gleich, Undo-History des Clients konsistent (alles
    rückgängig ergibt einen Text, den der Client hatte).
  - Mindestens 2000 Seeds × 50 Schritte in `cargo test`.
- **Reload-Sperre:** später Stapel über einem Reload ergibt A9; Puffer beim Reload ergibt Überspringen; Agent während
  einer unbestätigten Reload-Sperre ergibt `Busy`.
- **Cursor-Regel:** gegen die Fälle von `source_view.rs:428–437`.

**Nativ, `underline-editor-view`:**

- Die Editor-, Facade- und Faltentests der Session ziehen mit dem Code um (UC- und M-Namen bleiben).
- **Äquivalenz:** `CachedViews` mit der vollen Anreicherung des Backends ergibt dieselben `BlockView`s wie der
  Backend-Macher, auf je einer Markdown- und AsciiDoc-Notiz mit Code, Diagramm, Bild, Tabelle und Captions.

**Nativ, `underline-session`:** `ed_sync` mit

- Agent-Änderung während des Tippens: Backend-Edit, dann später Stapel, Hash gleich, gespeicherter Text = Spiegel;
- Flush-Handshake des Agenten mit 300-ms-Frist;
- Schliessen mit unbestätigten Edits: Stapel vor `tab_close` gespeichert, Stapel danach → `NotOpen`;
- Autosave auf dem Spiegel;
- Aussenänderung mit spätem Stapel → Kopie und A9-Status;
- Umbenennen mit Byte-zu-Zeichen-Umrechnung.

**Vitest mit dem echten WASM:**

- `core.test.ts`: Eingabe → Patch, `takeOutbox`, `acknowledge` mit Backend-Edits und Reload, Desync → `ed_resync`.
- `sync.test.ts` mit Fake-Timern: ein Aufruf unterwegs, Bündeln nach dem Frame, Klingel, `flushed()`.
- `SourceView`- und `FacadeView`-Tests laufen statt über `fakeEditor.ts`/`fakeFacade.ts` über das WASM mit
  Fake-Sync. Die Fakes fallen in B5 weg.

**Mock-Backend (Dev-Server, app-tester):**

- `mock-backend.ts` beantwortet `ed_open` (Text + Anreicherung), `ed_sync` und `ed_resync`.
- Es hält seinen Text mit einem kleinen Zeichen-Apply (`Array.from`) und rechnet den FNV-Hash in TS (TextEncoder).
  So prüft auch der Browser.
- Die Bearbeitung macht das echte WASM.
- Für Playwright gibt es `window.__ulMock.backendEdit(note, { at, remove, insert, sealed })` und
  `window.__ulMock.outsideChange(note, text)`. Damit lassen sich Rebase und Reload sichtbar testen.

## 7. Schritte

Jeder Schritt ist ein Commit mit allen Prüfungen aus `CLAUDE.md`; Muss-Findings der Reviews behoben.

| Schritt | Inhalt | Abnahme |
|---|---|---|
| **B3a** View-Crate | `underline-editor-view` anlegen; DTOs aus der Session hierher, Session re-exportiert; FNV statt `DefaultHasher` für Farbschlüssel | `bindings.ts` byteweise gleich; `cargo check -p underline-editor-view --target wasm32-unknown-unknown`; alle Tests grün |
| **B3b** `EditorTab` | Adapter, Falten (Uhr als `now_ms`), Nano-Treffer, `CachedViews` nach `EditorTab`; die Session nutzt `EditorTab` für die A-Commands weiter (gleicher Code, den später das WASM fährt) | Session-Tests unverändert grün; Äquivalenztest (6); kein `Instant`/`Mutex` in der Crate |
| **B3c** WASM und Build | `underline-editor-wasm`, `scripts/wasm.js`, npm-Hooks, `.gitignore`, CSP, CI, `core.ts`, Laden in Vitest | `npm run wasm` auf Linux; Vitest: dieselbe Eingabe ergibt im WASM denselben `EditorPatch`/`FacadePatch` wie ein nativ erzeugtes Golden; App startet unverändert (A) |
| **B4a** Sync-Kern | `editor-core::sync` (Changeset, `transform`, `hash`, `Server`, `Client`), DTOs in `sync_view.rs` | Tabellen-, Konvergenz- und Reload-Tests (6) |
| **B4b** Spiegel | `Tab` hält `Server` statt `SourceEditor` für Speichern/Highlighting; `ed_sync`, `ed_resync`, Events `ed-changed`, `ed-flush`, `ed-enrich`; `backend_edit`, `Reload` mit Sperre; Anreicherung im Tick | Session-Tests (6); A-Commands laufen noch und legen ihre Änderung als lokalen Stapel ins Log |
| **B4c** Sync im Fenster | `sync.ts` (Bündeln, Klingel, `flushed`, Schliessen-Handshake mit `app_request_close`), Mock-Backend | Vitest (6); `cargo tauri dev`: Schliessen mit frischer Eingabe speichert sie |
| **B5a** Quellansicht | `wasmBackend: EditorBackend`; `takeColors` mit `core.inStep`; Sektionen/Treffer aus `views()` | Vitest und app-tester: M-08, M-10.1, M-11, M-12 wie bisher; Bench (unten) |
| **B5b** Facade | `wasmFacadeBackend`; `ed-enrich` → `enrich()`; `ed_open` ersetzt `fa_open` | app-tester M-09; Bench |
| **B5c** Aufräumen, Messpunkt | A-Commands, -Events, Fakes und Session-Eingabecode entfernen; Messung; Reviews; F5b abhaken | Messpunkt erreicht; `paritaet.md` ergänzt; Commit |

**Messpunkt (B5a/B5b/B5c):**

- **Treiber:** `lib/editor/bench.ts` und `lib/facade/bench.ts` laufen mit `VITE_UL_BENCH=1` (`VITE_UL_BENCH_F4`,
  `VITE_UL_BENCH_F5`) in einem Release-Build auf WebKitGTK.
  - Statt `tauriBackend`/`tauriFacadeBackend` nehmen sie das WASM-Backend. Sonst ist alles gleich: 30 Aufwärmtasten,
    500 Tasten an 2/50/98 %, Jitter 0–17 ms, mit und ohne Farben, Normal/Vim/Nano.
  - In der Facade: Tippen, Enter, Backspace.
  - Testnotizen: die 10'000-Zeilen-Notiz des Spikes und dazu eine AsciiDoc-Notiz.
- **Ziel:** p99 ≤ 16 ms und mindestens 90 % der Tasten im nächsten Frame, in beiden Ansichten.
- **Zusätzlich gemessen:**
  - Dauer von `ed_sync` (Command und Antwort-Verarbeitung) samt Hash;
  - lange Tasks über 16 ms nach dem Frame;
  - Öffnen (Text ins WASM, Spike: 17 ms);
  - `init()` beim Start (NFR-001).
- **Am Ende:** Hash des Spiegels = Hash des WASM, 0 Desyncs.
- **Echte Tasten** über `xdotool` (plan Abschnitt 15), sobald installiert.

### 7.1 Umsetzung B3a und B3b (2026-11-10): Abweichungen vom Entwurf

- **`editor-core::sync` schon in B3a:** Das Modul enthält vorerst nur FNV-1a (`Fnv1a` mit `field()` für
  längenpräfixierte Felder, `hash()`), weil die Farbschlüssel es brauchen. Changeset, `transform`, `Server` und
  `Client` folgen wie geplant in B4a.
- **Schlüssel:** `colors_key(theme, language, code)` und `diagram_key(language, code, dark)` liegen in
  `underline_editor_view::views` und hashen Felder mit Längenpräfix; das Theme geht als Name (`Theme::label()`) ein.
  `session/diagrams.rs` nimmt für die SVG-Dateinamen weiter `DefaultHasher`. Das ist kein Farbschlüssel und bleibt im
  Backend.
- **Gemeinsamer View-Bau:** Statt `CachedViews` nur neben den Backend-Macher zu stellen, baut beide derselbe
  `ViewMaker` über den Trait `Enrich` (Farben, Diagramm, Bild). Der Backend-`Maker` ist ein `Enrich` über `Context`.
  Damit ist die Gleichheit der Views strukturell gesichert; der Äquivalenztest prüft die Anreicherung.
- **Aufteilung der Facade:** Blöcke, Views, Version, `parsed` und `refs` liegen als `FacadeModel` in
  `EditorTab.facade`. Die Session-`Facade` behält nur Jobs, Look, Diagramm-Generation, `last_pictures` und `recolor`.
- **Uhr:** `EditorTab::set_now(ms)` statt `now_ms` als Argument jeder Methode. Die Session rechnet mit
  `editing::millis(Instant)` (Epoche eine Stunde vor dem ersten Aufruf) und setzt die Uhr vor jedem Editor-Command
  und vor `reload`.
- **Facade-Aufrufe** bündeln Notiz, Auswahl, Plattform, vorherige Änderungen und den View-Macher in `FacadeCall`.
- **Tests:** Die sieben Faltentests sind mit `folds.rs` in die View-Crate umgezogen (Uhr in ms statt `Instant`). In
  den Session-Tests haben sich nur Zugriffspfade geändert (`tab.edit.editor`, `tab.edit.own_markup`, `commit` mit
  `FacadeModel`); keine Erwartung wurde geändert.
- **`Maker.uncoloured`** wurde gesetzt, aber nie gelesen, und ist entfallen.
- **Nachtrag nach dem Review (Soll):**
  - **Schlüssel sind Hex-Strings.** `colors_key` und `diagram_key` geben 16 Hex-Zeichen zurück (`Fnv1a::hex()`), wie
    `hash()`. Eine JavaScript-Zahl trägt nur 53 Bit exakt; als Strings kommen die Schlüssel unverändert über JSON
    (`EnrichView` in 2.3, ab B5b). Auch der Farb-Cache des Backends führt sie als String.
  - **Ein Diagramm-Schlüssel.** `session/diagrams.rs` rechnet seinen Schlüssel mit `diagram_hash` (FNV) statt mit
    `DefaultHasher`. Der Bildname `diagram/{hash:016x}.svg` ist damit genau der `diagram_key` des Webviews.
  - **Eine Regel für die Uhr.** Jede Methode mit `now_ms` (`sections_view`, `found_view`) setzt die Uhr über
    `set_now`. Die Uhr hat eine Epoche je Tab und läuft nur vorwärts; nicht endliche Werte werden ignoriert.
  - **Kollisionen** von 64-Bit-Schlüsseln werden hingenommen (etwa 1 zu 10^12 bei einigen Tausend Blöcken; sichtbar
    wären nur Farben oder Bild eines anderen Blocks bis zur nächsten Änderung).

### 7.2 Umsetzung B3c (2026-11-10): Stand und Abweichungen

- **WASM-API in B3c:**
  - Vorhanden sind die Funktionen von 2.2 für beide Ansichten, dazu `setMode`, `setView`, `setLooks`, `setNow`,
    `views`, `text` und `hash`.
  - Noch nicht vorhanden sind der Sync (`takeOutbox`, `acknowledge`, `syncedRev`, `inStep`, B4) und `enrich` (B5b).
  - Geöffnet wird mit dem Konstruktor `new WasmTab(note, text, format, mac)` statt mit `open(TabOpenView)`; diesen
    DTO gibt es erst mit `ed_open` in B5.
  - `setView` nimmt ein Flag (Facade ja/nein). `select` antwortet wie alle Aufrufe mit JSON, hier mit einem
    JSON-String.
- **Aufbau:** Die Aufrufe liegen ohne wasm-bindgen in `crates/underline-editor-wasm/src/calls.rs` (JSON hinein und
  heraus); `lib.rs` ist nur die Hülle `WasmTab`. Das Modul heisst `calls`, damit es die Crate `core` nicht verdeckt.
- **Neue DTOs:** `Clipped<P>` (Zwischenablage) und `TabViews` (Sektionen und Treffer). `bindings.rs` exportiert sie
  mit `.typ()`.
- **Golden-Test:**
  - `src/golden.rs` fährt drei Szenarien nativ (Quellansicht Markdown, Facade Markdown, Facade AsciiDoc) und schreibt
    sie mit `UNDERLINE_UPDATE_GOLDEN=1` nach `golden/calls.json` (eingecheckt).
  - `ui/src/lib/editor/core.test.ts` spielt dieselben Aufrufe gegen das WASM ab.
  - Verglichen wird als JSON-Wert, nicht als Text: Im ganzen Workspace schaltet ein anderes Crate serde_jsons
    `preserve_order` zu, und dann ändert sich die Reihenfolge der Schlüssel.
- **Vitest:** `src/test-setup.ts` lädt das WASM mit `initSync` aus den Bytes. Node-`fs` kommt dabei über
  `process.getBuiltinModule` (Node ≥ 22.3), damit es kein `@types/node` braucht. Der Pfad ist relativ zu
  `process.cwd()`, weil jsdom `import.meta.url` als http-Adresse setzt.
- **Start:** `main.ts` ruft `loadEditorCore()` beim Start ohne Warten auf. Ein Fehler landet nur in der Konsole, und
  die App läuft weiter auf Variante A.
- **Gemeinsam genutzt:** `EditorTab::bring_facade_up_to_date` (Parse und alle Views) nutzen die WASM-Hülle und die
  Session gleichermassen.
- **CSP:** Hinzu kommen `script-src 'self' 'wasm-unsafe-eval'` und `connect-src 'self'`.
  - `connect-src` ist nötig, weil das Glue die `.wasm` per `fetch` lädt (`instantiateStreaming`; Spike
    RESULTS.md:263).
  - Im Smoke-Start (`npm run tauri dev`) lief die App, und der Dev-Server lieferte die Datei als `application/wasm`.
  - Die Konsole des Webviews wurde nicht ausgelesen.
- **`scripts/wasm.js`** nimmt `CARGO` als Ersatz für `cargo` und sagt, was fehlt (CLI-Version, cargo im `PATH`).
- **Messung (Q-34), Node 24 / V8, nicht JavaScriptCore:**
  - `.wasm` 1'117'610 Bytes roh, 362'492 Bytes mit gzip -9;
  - `WebAssembly.compile` 1,5–5,5 ms, `initSync` 0,6 ms;
  - Tab mit 10'000 Zeilen öffnen 4 ms, `facadeState` dieser Notiz 100 ms (alle Blöcke samt JSON, wie heute `fa_open`).
  - Auf WebKitGTK wird in B5 gemessen.

**Nachtrag nach dem Review von B3c:**

- **Panik im WASM:**
  - `#[wasm_bindgen(start)]` setzt beim Laden einen Panic-Hook, der die Meldung über `console.error` (eigener
    `extern`-Import) schreibt, bevor die WASM-Instanz trapt (`panic = abort`).
  - `core.ts` führt jeden Aufruf über einen Helfer:
    - Ein `Error` aus `JsError` heisst „Argument falsch“, und der Tab arbeitet weiter.
    - Ein Trap (`WebAssembly.RuntimeError`) oder die Glue-Meldung „recursive use of an object“ heisst „Kern kaputt“:
      Der Tab wird freigegeben, `EditorCore.onDead` erfährt es einmal, und jeder spätere Aufruf wirft
      `EditorCoreDead`.
- **Wiederaufnahme (Haken für B4/B5):** Wer den `EditorCore` hält (die Registry `tabs.ts`), setzt `onDead`.
  - Er öffnet den Tab neu aus dem Text des Backends (`ed_open`) bzw. mit Resync (3.10).
  - Edits seit der letzten Bestätigung sind dann verloren. Das ist zu melden; der Text dafür ist eine Produktfrage für
    B4.
- **Ohne Panik:** `src/hostile.rs` prüft nativ alle Aufrufe in allen drei Modi auf drei Notizen (Emoji, CRLF, leer):
  - Offsets am Ende, dahinter und mitten in einem Surrogatpaar;
  - Bereiche rückwärts, fremde Revisionen, riesige Zeilen und Seiten;
  - falsches JSON für jeden DTO-Parameter;
  - dazu 60 zufällige Läufe mit je 150 Aufrufen.
  - Es fand sich keine Panik; der Editor klemmt bereits an Textlänge und Zeichengrenzen.
- **Fehler:**
  - `CallError` ist ein `thiserror`-Enum (`Invalid`, `NoFormat`, `NoMode`, `Answer`); `JsError::from` übernimmt die
    Meldung.
  - `json()` gibt einen Fehler zurück, statt „null“ zu schreiben.
  - Die Golden-Szenarien enthalten auch Fehlerschritte, die als `{ "error": … }` auf beiden Seiten verglichen werden.
    Der Golden-Test nennt das erste abweichende Szenario mit seinem Schritt und unterscheidet eine fehlende von einer
    ungültigen Datei.
- **Build:**
  - `scripts/wasm.js` baut mit `--locked`, gibt beim Scheitern den Hinweis auf `rustup target add` und bricht ab, wenn
    `Cargo.lock` mehr als eine wasm-bindgen-Version hält.
  - Die CI liest die Version aus `Cargo.lock` und verwendet sie für den Cache-Schlüssel und für die Installation.
- **Uhr:** Ein Tab hat genau einen Taktgeber: in der Session `millis`, im Webview `performance.now()`. Eine frühere
  Zeit drosselt nicht erneut (Test `adr025_an_earlier_time_does_not_throttle_anew`).

### 7.3 Umsetzung B4a und B4b (2026-11-10): Stand und Abweichungen

- **`ChangeSet`** (`editor-core::sync`):
  - Die Form folgt ot.js/CodeMirror: Behalten, Einfügen, Löschen in Zeichen; ein Einfügen steht vor einem Löschen an
    derselben Stelle.
  - `transform(a, b)` gibt `a` Vorrang bei Gleichstand. Der Server ruft es mit seiner Änderung als `a` auf; so ergibt
    sich die Regel „Backend links“ von 3.5.
  - Die Zeilen der Tabelle in 3.5 stehen als Test `adr025_transform_follows_the_table` in beiden Reihenfolgen.
- **`Server`:**
  - `receive` nimmt Stapel, `backend_edit` und `local_edit` nehmen Zeichen, dazu gibt es `reload`, `confirm_reload`
    und `resync`. `KEPT` = 1024 Revisionen.
  - Ein Stapel auf einer unbekannten oder künftigen Revision, oder einer, der nicht passt, ändert nichts und antwortet
    mit `resync`.
- **`Client`:**
  - Mit `local`, `ring`, `take_outbox`, `acknowledge` und `reset`.
  - Nach einem angewandten Reload klingelt der Client selbst (`pull`), damit das Backend den Reload bestätigt bekommt
    und Agenten nicht dauerhaft `Busy` hören. Das ist eine Ergänzung zu 3.6.
- **`apply_remote`** wendet eine Backend-Änderung wie egui auf einen `SourceEditor` an: Siegel bei Agenten,
  Cursorregel `shifted`, kein Scrollen.
- **Neu im Editor-Kern: das Edit-Journal von `Document`** (`record_edits`/`take_edits`, in `SourceEditor`
  durchgereicht).
  - Damit gehen die Änderungen des Session-Editors (Variante A) als lokale Stapel an den Spiegel.
  - In B5 liefert es dem `Client` die lokalen Edits des WASM-Editors.
  - Das Journal ist nur eingeschaltet, wo es abgeholt wird; die egui-App bleibt unberührt.
- **Session (B4b):**
  - `Tab` hält `mirror: Server`. Speichern, Autosave und Highlighting lesen den Spiegel; die Invalidierung kommt aus
    `Touched`.
  - Bis B5 folgt der Session-Editor (`tab.edit`) dem Spiegel (`follow_mirror`), und alle A-Commands schicken ihre
    Edits über das Journal als lokalen Stapel. Beide Texte sind immer gleich; jeder Sync-Test prüft das.
  - `window_synced`: Solange kein Fenster mit `ed_sync` synchronisiert hat, bestätigt die Session einen Reload sofort.
  - `backend_edit` nimmt Byte-Bereiche wie egui. Ein Byte mitten in einem Zeichen zählt ab dessen Anfang, wie
    `apply_edits`.
  - `request_flush` gibt es schon; aufgerufen wird es erst ab B4c (Schliessen) und F14 (Agenten).
  - `ed-changed` kommt zusätzlich zu `ed-replaced`, weil die Ansichten von Variante A nur dieses kennen.
- **`ed-enrich`:**
  - Das Event führt `diagrams` und `images` als getrennte Listen statt einer Liste `figures` (2.3), weil die Schlüssel
    verschieden sind.
  - In B4b leitet der Tick die Einträge aus den Views ab, die das Backend für die Facade gebaut hat. Das geschieht nur,
    solange die Facade gezeigt wird, und nur, wenn sich die Views oder der Look geändert haben.
  - Code, dessen Farben noch ausstehen (`recolor`), wird ausgelassen. Ein neuer Look (Theme, Farben bereit) sendet mit
    `clear` alles neu.
  - Ab B5 rechnet das Backend die Einträge ohne eigene Views.
- **Busy für Agenten:** `Server::reload_unconfirmed` steht bereit; der Agentenpfad (F14) fragt es ab.

**Nachtrag nach dem Review von B4a/B4b:**

- **Übersprungener Reload:** Wer einen Reload wegen ungesendeter Edits überspringt, klingelt danach selbst (`pull`). Ein
  Puffer, der sich zu nichts aufhebt (x tippen, x löschen), holt den Reload so trotzdem.
- **Rücknahme eines Reloads:**
  - Ein Stapel nimmt einen Reload nur zurück, wenn seine Edits zusammen etwas ändern.
  - Als Kopie bleibt der Text, den der Spiegel zuletzt hatte: der neueste von aussen samt allem, was das Backend
    danach daran tat. Nichts, was der Autosave gespeichert haben kann, geht ohne Kopie verloren.
  - `receive` plant zuerst (`plan`) und ändert erst danach etwas.
- **Resync über einen unbestätigten Reload:** Der Text des Spiegels wird ebenfalls als A9-Kopie behalten, mit
  demselben Status (`Resynced::outside`).
- **Kopie ausserhalb des Locks:**
  - Die Session gibt die Kopie als `KeepCopy` zurück (`Synced`). `ed_sync` und `ed_resync` schreiben sie, nachdem die
    Session freigegeben ist, über `NoteWriter` (neu in `underline-core`, wie `NoteSaver::save_now`).
  - Danach setzt `Session::copy_kept` den Status.
  - Die Commands bleiben synchron. Die Reihenfolge hielte auch asynchron, weil je Notiz höchstens ein Stapel unterwegs
    ist.
- **Hash:** FNV läuft über die Chunks des Ropes (`hash_rope`), ohne den Text zu kopieren.
- **`apply_remote`** prüft die Länge und gibt einen `Result` zurück. Passt ein Satz nicht, übernimmt der Session-Editor
  den Text des Spiegels ganz.
- **`backend_edit`** weist Byte-Bereiche hinter dem Ende, rückwärts, in falscher Reihenfolge oder mitten in einem
  Zeichen als `Invalid` ab. Das ist strenger als egui, das dort rundet.
- **`Client::failed`:** Das Fenster ruft es bei jedem Fehler von `ed_sync` auf. Der Stapel kommt zurück in den Puffer
  und geht mit der nächsten Outbox erneut auf derselben Revision.
  - Kam er doch an und nur die Antwort ging verloren, wendet ihn das Backend doppelt an. Der Hash merkt das, und der
    Resync mit dem Fenstertext stellt es richtig.
- **Tests:**
  - TP1 und `compose` als Eigenschaftstests (je 20'000 zufällige Sätze).
  - Der Zufallslauf deckt zusätzlich ab: Kürzen auf `KEPT` mit Refused und Resync, `local_edit`, Backend-Edits an
    mehreren Stellen, Undo/Redo im Fenster, Puffer, die sich zu nichts aufheben, verlorene Stapel.
  - Nach jedem Schritt gilt im Zufallslauf: Fenstertext = Schatten + unterwegs + Puffer.
  - Ein Stapel auf `floor` wird angenommen.
  - Nachträglich gefärbter Code kommt über `ed-enrich` an.

### 7.4 Umsetzung B4c und B5a (2026-11-10): Stand, Abweichungen, Messpunkt

- **Neue Commands:**
  - `ed_mirror(note) -> MirrorView`: Text samt `\r\n`, Revision, Format und Cursor des Tabs. Es ersetzt `ed_open` für
    den Kern; `ed_open` bleibt bis B5c für Variante A.
  - `app_request_close`.
- **`HighlightView.rev`** ist jetzt die Revision des Spiegels; ein Fenster mit WASM-Kern prüft sie mit `inStep`.
- **`ed-changed`:** Der Tick klingelt auch, wenn der Session-Editor (Facade, Variante A) einen Text ändert, den ein
  Fenster-Kern hält (`told_rev`).
- **`FlushView.closing`:** Beim Schliessen schickt das Fenster alle Edits und ruft dann `app_request_close`.
- **Schliessen:** `CloseRequested` hält das Fenster offen, startet den Handshake (`Closer::begin_handshake`) und
  klingelt mit `ed-flush { closing }`. Nach 1 s ohne Antwort schliesst das Backend mit seinen Kopien.
  `app_request_close` speichert alles und beendet die App oder fragt „Changes not stored“.
- **WASM:**
  - Die Hülle hat jetzt `startSync`, `takeOutbox`, `acknowledge` (mit `RemoteApplied`, einem Patch für die
    Quellansicht), `ring`, `failed`, `reset`, `inStep` und `idle`.
  - Lokale Edits holt `record` aus dem Journal des Editors, bevor der Client etwas tut.
  - `EditorTab::apply_incoming` wendet Backend-Änderungen an: wie egui, ohne Scrollen, ein Reload als ein Ersatz des
    ganzen Textes.
- **Spiegel im Mock:** Die WASM-Datei enthält auch den Spiegel (`WasmMirror`/`MirrorCore`). Der Mock des Dev-Servers
  rechnet damit statt mit einem FNV in TypeScript und hat dadurch dieselben Revisionen, Transformationen und Hashes
  wie das Backend.
  - Hooks: `window.__ulMock.backendEdit(note, at, remove, insert, sealed)`, `outsideChange(note, text)`,
    `mirrorText(note)`.
- **`sync.ts`:**
  - Je Notiz ein `TabCore` mit einem Stapel unterwegs. Gesendet wird nach dem Frame (rAF + `setTimeout 0`); `ring`
    kommt von `ed-changed`.
  - `failed()` mit bis zu drei Wiederholungen; bei Desync `ed_resync`.
  - `flushed()` um `tab_close`, `tab_store`, `tab_set_view`, `ws_open` und `ws_create`.
  - `takeFlush` für `ed-flush`. `onDead` öffnet den Kern neu aus dem Text des Backends (Q-40, vorläufig).
- **Quellansicht (B5a):**
  - `wasmBackend(mac)` ist das Standard-Backend von `SourceEditor`.
  - Sektionen und Treffer kommen aus `views()` des Kerns; die Events `ed-sections`, `ed-found` und `ed-replaced`
    ignoriert die Ansicht dann.
  - Farben gelten, wenn der Kern mit ihrer Revision im Takt ist.
  - Die Pfade von Variante A bleiben bis B5c.
- **Tests:**
  - Die Frontend-Tests, die Aufrufe von `ed_input`/`ed_apply`/`ed_fold` prüften, prüfen jetzt Text, Spiegeltext und
    `ed_sync`.
  - Neu sind `sync.test.ts` und Tests für Agent und Aussenänderung im Editor.
- **Echter Schliess-Test** (`cargo tauri dev`, Scratch-Workspace): tippen und sofort `close()` speichert den Text über
  den Handshake, ohne die Rückfall-Frist. Der Treiber und die Capability `core:window:allow-close` waren nur dafür da
  und sind wieder entfernt.

**Messpunkt B5a (Quellansicht auf dem WASM-Kern):**

- **Methode:** wie F3/F4, Release-Build mit eingebettetem Frontend, WebKitGTK auf Wayland, dieselbe Maschine, eigenes
  `HOME`, 500 gezählte Tasten je Stelle.
- **Was „Verarbeitung“ misst:** vom Start der Eingabe bis der Patch in CodeMirror ist. `ed_sync` läuft danach und
  nicht im Tastenpfad.

| Notiz / Lauf | Verarbeitung p99 (ms) | max | im nächsten Frame |
|---|---|---|---|
| Markdown, ohne/mit Farben, Anfang/Mitte/Ende | 2–6 | 3–8 | 98–100 % |
| Markdown, F4 Normal (Farben, Zeilennummern, Minimap, Falten) | 5 | 6–7 | 98–99 % |
| Markdown, F4 Vim (INSERT) | 6 | 6–8 | 96–99 % |
| Markdown, F4 Nano | 5–7 | 7–8 | 99 % |
| AsciiDoc, ohne/mit Farben | 3–4 | 3–6 | 99–100 % |

- **`ed_sync`:**
  - Roundtrip median 2–3 ms, p99 12–13 ms, max 30 ms; der Roundtrip läuft asynchron und blockiert nichts.
  - `acknowledge` und das Anwenden: p99 3 ms.
  - `views()` (Sektionen, gedrosselt auf 300 ms, und Treffer): p99 5–8 ms, max 10 ms.
- **Frames:** Abstände median 17 ms, p99 22–25 ms. Je Lauf 2–6 Abstände über 50 ms, beim Öffnen und Schliessen der
  Ansichten.
- **WASM:**
  - laden und kompilieren 18–20 ms;
  - 1'225 kB roh, 403 kB gzip (mit Spiegel und Sync);
  - keine CSP-Verletzung (`securitypolicyviolation`) mit der Release-CSP.
- **Öffnen:** Markdown 45 ms, AsciiDoc 96 ms; beim zweiten Öffnen mit bestehendem Kern 8–9 ms.
- **Ziel erreicht:** p99 ≤ 16 ms und mindestens 90 % im nächsten Frame. In F3/F4 waren es 85–90 %.

**Nachtrag nach dem Review von B4c/B5a:**

- **Nummer je Stapel (`SyncBatch.seq`):**
  - Ein Stapel, dessen Antwort ausblieb, geht unverändert noch einmal, mit derselben Nummer (`Client::failed`).
  - Das Backend erkennt ihn an der Nummer, antwortet mit der gespeicherten Antwort und wendet ihn nicht zweimal an.
  - Getipptes geht im nächsten Stapel.
  - Damit entfällt die Doppelanwendung, die zuvor erst der Hash fand.
- **Resync:** `reset(sent, rev)` setzt den Client auf den gesendeten Text. Was während des Resync getippt wurde, bleibt
  als Differenz zu senden.
- **Wiederholen:** Ein fehlgeschlagenes `ed_sync` wird mit wachsender Wartezeit wiederholt (100 ms bis 5 s), solange
  Edits warten. Nach drei Fehlschlägen nennt die Statuszeile den Grund, einmal je Störung.
- **Flush:**
  - `flush()` meldet, ob alles ankam (höchstens 3 s).
  - `flushed()` führt den Befehl sonst nicht aus und antwortet wie ein fehlgeschlagener Befehl (Statuszeile).
  - Schliessen eines Tabs oder von Underline fragt dann „Changes not stored“, wie wenn das Speichern scheitert (UC-005
    A14). „Close anyway“ schliesst ohne Flush.
- **`app_request_close(sent)`:**
  - Ohne `sent` endet der Handshake, und das Fenster fragt.
  - Eine späte Antwort nach dem Rückfall schliesst nicht ein zweites Mal; die Antworten sind nicht an die Nummer des
    Handshakes gebunden.
- **Ein Fenster gilt erst mit seinem ersten `ed_sync` als synchronisiert**, nicht schon mit `ed_mirror`. Ein Kern, der
  nicht laden konnte, hält so keinen Reload auf. Beim Zeigen holt der Kern, was sich geändert hat.
- **Kerne geschlossener Notizen** werden mit dem `tabs`-Event verworfen. Ein veralteter Kern schickt so nie seinen
  Text über die Datei.
- **Ausgefallener Kern:** Die Statuszeile meldet ihn (Q-40, vorläufiger Text). Geschlossene Kerne melden nichts mehr.
- **Regel:** Jeder neue Befehl, der den Text liest, geht über `flushed()`.
- **Undo nach einer Agent-Änderung:** Der Cursor steht am Ende des zurückgesetzten Textes. Das ist derselbe
  Code wie in egui (`Document::undo` → `after_history`), also gleiches Verhalten.

### 7.5 Umsetzung B5b (2026-11-10): Stand, Abweichungen, Messpunkt

- **Facade auf dem WASM-Kern:** `facade/wasmBackend.ts` (`wasmFacadeBackend(mac)`) ist das Standard-Backend von
  `FacadeView`.
  - `open` öffnet den Kern (`openCore`) und meldet dem Backend parallel die gezeigte Ansicht (`tab_set_view`).
  - Jede Eingabe, Auswahl, Faltung und Zwischenablage-Aktion beantwortet der Kern in derselben Task wie die Taste;
    die Edits gehen nach dem Frame mit `ed_sync`.
  - Die System-Zwischenablage bleibt ein Command des Backends.
- **`FacadeBackend.attach`:** Damit hängt sich die Facade an den Kern. Über diesen Weg kommen:
  - Änderungen des Backends (`patch` → `facadeRefresh`);
  - Sektionen für die Faltpfeile (`views`);
  - Blöcke mit Farben und Bildern (`blocks`);
  - ein neu geöffneter Kern (`reopened`).

  Die Events `ed-blocks` und `ed-sections` des Backends ignoriert eine angehängte Facade.
- **Anreicherung (Abweichung von 2.3):**
  - Der Tick leitet Farben und Bilder aus den Facade-Views der Session ab, solange die Facade gezeigt ist
    (`tab_set_view(facade)` setzt `shown` und weckt den Tick). Er schickt sie als `ed-enrich` mit Thema und `dark`.
  - Der Kern übernimmt sie mit `enrich()` in seine Views (`CachedViews::enrich`). Nur die geänderten Blöcke gehen
    an die Facade, als Blockänderungen mit Version.
  - `open` bringt keine Anreicherung mit. Bis zum ersten `ed-enrich` zeigt die Facade Code ohne Farben und Diagramme
    als Platzhalter.
  - `ed_mirror` setzt das Gesendete zurück (`forget_sent`), damit ein neuer Kern alles bekommt.
  - Der Mock rechnet die Anreicherung mit `WasmMirror.enrichment` aus derselben WASM-Datei.
- **Keine `ed-blocks` mehr für eine Facade im Fenster:**
  - Nach `ed_mirror` (`Tab::window_facade`) macht die Session ihre Facade-Views weiter, aber nur für die
    Anreicherung. Ihre Blockänderungen schickt der Tick nicht mehr.
  - Der erste Aufbau einer langen Notiz war ein einziges `ed-blocks` mit allen Blöcken: 3'697 Blöcke, 2,7 MB. Das
    Fenster warf es weg, brauchte aber bis 180 ms, es anzunehmen. Ein `tab_set_view` direkt danach wartete so lange.
  - `fa_open` (Variante A) schaltet das wieder ab.
- **Facade wieder zeigen:**
  - Beim Verlassen gibt die Facade ihre Blöcke samt Version an das Backend (`FacadeBackend.keep`); der `TabCore` hält
    sie (`facadeKept`).
  - Hat der Kern beim nächsten `open` noch dieselbe Version, holt `open` nur den Kopf (`facadeHead`: der
    `FacadeState` ohne Blöcke) und nimmt die gehaltenen Blöcke. Sonst holt es den ganzen `facadeState`.
  - Vorher kostete das zweite Öffnen einer langen Notiz so viel wie das erste.
- **Blockänderungen in einem Zug:** `FacadeView.takeBlocks` nimmt eine Liste. Eine Folge von Änderungen kostet eine
  Kopie der Blöcke, eine Rechnung der Höhen und ein Layout, nicht eine je Änderung.
- **Eingaben beim Wechsel der Ansicht:** Eingaben in der Warteschlange gehen beim Abbau einer Ansicht nicht mehr
  verloren. Facade und Quellansicht arbeiten sie noch ab, auch das Vim-Register der Quellansicht.
- **Tests:**
  - Vitest: Wechsel in derselben Task in beide Richtungen; Agent-Änderung in der Facade; die Facade nimmt beim
    erneuten Zeigen ihre Blöcke, ausser der Text änderte sich.
  - Rust: `facade_head` ist `facade_state` ohne Blöcke; die Facade im Fenster bekommt Farben, aber keine
    `ed-blocks`; die Anreicherung kommt mit Thema; `CachedViews::enrich`.

**Messpunkt B5b (Facade auf dem WASM-Kern):**

- **Methode:** wie B5a; `VITE_UL_BENCH_F5B=1` für Tippen, Backspace und Enter; `VITE_UL_BENCH_F5=1` für Öffnen,
  Wechsel und Scrollen. Der Bench nimmt das WASM-Backend der Facade.

| Notiz / Lauf | Verarbeitung p99 (ms) | max | im nächsten Frame |
|---|---|---|---|
| Markdown, Normal/Vim/Nano, Tippen, Backspace, Enter, Anfang/Mitte/Ende | 2–4 | 2–10 | 98–100 % |
| AsciiDoc, ebenso | 2–7, einmal 14 (Enter+Backspace Mitte) | 2–14 | 93–100 % |

- **Ziel erreicht:** p99 ≤ 16 ms und mindestens 90 % im nächsten Frame.
- **`ed_sync`:** Roundtrip median 1 ms, p99 6 ms, max 73–75 ms. `views()` p99 2 ms.
- **Anreicherung** (Markdown; von der Taste bis das `ed-enrich` da ist):
  - Farben median 56 ms, p99 83 ms;
  - Diagramme median 52 ms, p99 79 ms. Der Bench ändert den Diagrammcode im Kern. Ob das `ed-enrich` schon das neue
    Bild oder noch das letzte bringt (M-09.7), unterscheidet er nicht.
  - Die AsciiDoc-Notiz ist aus der Markdown-Notiz umgewandelt und hat keine Sprache an den Codeblöcken. Darum gibt
    es dort keine Messwerte für die Anreicherung.
- **Öffnen und Wechsel** (Markdown, 3'697 Blöcke, 2,7 MB Facade):

  | | F5a | B5b vorher | B5b |
  |---|---|---|---|
  | `open` kalt (Kern neu) | 216 ms | 193 ms | 187–220 ms |
  | `open` warm | 38 ms | 199 ms | 41–46 ms |
  | Facade zeigen bis gemalt, median | 83 ms | 51 ms | 30–32 ms |
  | Facade → Quelle, median | 29 ms | 24 ms | 29–35 ms |
  | Quelle → Facade, median | 69 ms | 54 ms | 38 ms |
  | Scrollen je Seite median / max | 34 / 682 ms | 33 / 589 ms | 33–34 / 547–582 ms |

  - „warm“ ist in F5a ein zweites `fa_open` mit fertigen Blöcken im Backend. In B5b ist es ein zweites `open` mit
    bestehendem Kern: `tab_set_view` und `facadeHead`. Ohne das Warten auf `tab_set_view` sind es 2 ms.
  - Vorher (199 ms) wartete `tab_set_view`, bis das Fenster das grosse `ed-blocks` angenommen hatte (siehe oben).
- **Lange Frames (über 50 ms), nach Phase:**
  - Öffnen und Einhängen: 123–226 ms beim ersten Öffnen. Der Kern baut die Facade der ganzen Notiz im Hauptthread
    (`facadeState` kalt 169 ms). In F5a lief das im Backend.
  - Ende der Notiz, beim ersten Tippen: ein Frame von 544–570 ms. Der Kern übernimmt Farben und Bilder in derselben
    Messung in 1 ms, und die Facade braucht für die 76 geänderten Blöcke unter 1 ms. Der Frame ist also das Malen
    der Diagrammbilder in WebKitGTK, wie das Maximum beim Scrollen (547–682 ms schon in F5a).
  - Phase Anreicherung: 56–105 ms je Probe. Das ist der ganze `facadeState`, den der Bench für jede Probe holt.
    Beim Warten auf `ed-enrich` gibt es keine langen Frames mehr.
  - Tippen selbst: keine langen Frames ausser beim Setzen des Cursors (53–78 ms, Sprung an die Stelle).
- **WASM:**
  - 1'325 kB roh.
  - Laden und kompilieren: 17–88 ms, zweigipflig um 20 und 85 ms. Ein frisches und ein behaltenes `HOME` streuen
    gleich: 44, 85, 87 gegen 17, 88, 21, 85 ms. WebKitGTK legt für die App keinen Cache des kompilierten WASM an, und
    die Streuung kommt vom gleichzeitigen Start der App.

### 7.6 Umsetzung B5c (2026-11-10): Variante A entfernt

- **Entfernt (Tauri-App und Sitzung):**
  - die Befehle `fa_open`, `fa_input`, `fa_clipboard`, `fa_keys_status`, `ed_open`, `ed_input`, `ed_select`,
    `ed_apply`, `ed_fold`, `ed_keys_status` und `ed_clipboard`, samt Berechtigungen und Capability-Einträgen;
  - die Events `ed-blocks`, `ed-sections`, `ed-found` und `ed-replaced` (`Event::Blocks`, `Sections`, `Found`,
    `TextReplaced`);
  - in der Sitzung die Eingabe-Methoden (`editor_*`, `facade_input`, `facade_clipboard`, `facade_prepare` und
    `facade_finish` mit `FacadePrep`/`FacadeBuilt`), `Tab::sync_local`, `Tab::window_facade` und `replaced`;
  - im Fenster `tauriBackend`, `tauriFacadeBackend`, die Fälle ohne Kern in Quellansicht und Facade (`attach`,
    `inStep`, `setMode`, `refresh` und `leave` sind jetzt Pflicht), `editor/fakeEditor.ts` und die Mock-Befehle der
    Variante A.
- **Bleibt in der Sitzung:** Spiegel, Speichern, Änderungen von aussen, Hervorhebung (`ed_viewport`,
  `ed-highlight`) und die Facade-Views nur noch für `ed-enrich` (`facade/recolor.rs` färbt Code nach).
- **Die DTOs**, die nur noch zwischen Fenster und WASM gehen (`EditorInput`, `EditorState`, `FacadeInput`,
  `FacadeAt`, `FacadeState`, `RowsView`, `VimChange`, `VimState`, `ClipAction`), exportiert `bindings.rs` mit
  `.typ`.
- **Tests:** Die Regeltests der entfernten Befehle laufen jetzt auf dem `EditorTab`, den das WASM fährt:
  `underline-editor-view/src/tab/source_tests.rs` (21, vorher 20 Sitzungstests) und `tab/facade_tests.rs` (19 und
  ein Messlauf, vorher dieselben 20 in der Sitzung). Die Sitzungstests ändern den Text wie ein Fenster mit `ed_sync`
  (`Session::window_edit`, nur in Tests). Wegfallen ohne Ersatz, weil ihr Gegenstand fehlt: der Wettlauf `fa_open`
  gegen den Tick, `fa_open` gibt die Blöcke zurück, Edits der Sitzung erreichen ein Fenster, keine `ed-blocks` für
  ein Fenster.
- **Q-46** gegen egui geprüft: gleiches Verhalten (gleicher Code), festgehalten im Test
  `q46_nano_ctrl_k_on_the_last_block_as_in_egui`.

## 8. Risiken und offene Fragen (für `offene-fragen.md`)

1. **Agent und ungesendete Tasten (UC-019 BR-004):**
   - Nach dem Flush-Handshake bleibt ein Fenster von einem Roundtrip.
   - Trifft in dieser Zeit eine Taste dieselbe Zeile, bleiben beide Texte erhalten (3.5). Die Zeile hätte egui aber
     abgelehnt (A5).
   - Frage: Reicht das, oder soll der Spiegel späte Stapel, die eine Agent-Zeile berühren, melden?
2. **Desync: Der Fenstertext gewinnt.** Das kann eine schon bestätigte Agent-Änderung verwerfen. Sollen Benutzer oder
   Agent davon erfahren? Heute gibt es nur einen Log-Eintrag.
3. **Reload-Sperre bei Verwerfen, Branch-Wechsel und Sync:** Eine Taste im selben Roundtrip würde A9 auslösen und das
   Verworfene als Text behalten. Gewollt, oder soll für diese Auslöser der Backend-Text gewinnen?
4. **Farbverzug:** Während dauernden Tippens kommen Farben neuer Zeilen erst, wenn das Fenster im Takt ist.
   Verschieben durch offene Edits nur nach Messung?
5. **CSP:** `'wasm-unsafe-eval'` und `connect-src 'self'`. In ADR-023 nachtragen oder in einem neuen ADR?
6. **Schliessen über das System bei hängendem Fenster:** Nach 1 s wird auf dem Spiegel gespeichert. Die Edits des
   letzten Frames können fehlen.
7. **Speicher:** ein Rope je offenem Tab im WASM und ein Spiegel im Backend; Kopieren beim Öffnen etwa 17 ms bei
   450 K. Gilt das Tab-Limit (M-06.6) auch dafür?
8. **Startzeit:** `init()` und Kompilieren von etwa 1 MB WASM gegen NFR-001. Messen in B3c.
9. **Umbenennen ohne Siegel** (wie egui) kann mit einem Tipp-Schritt des Benutzers zu einem Undo-Schritt verschmelzen,
   wenn die Stellen aneinanderstossen (`merges`). Übernehmen wie egui?
10. **IME/Komposition** in der Quellansicht, während der Text dem WASM gehört (CM-Komposition wird zurückgenommen,
    Spike nur synthetisch). Echte Tests in F16/Phase 4.
11. **Versionspflege:** `wasm-bindgen` in `Cargo.lock` und die CLI müssen gleich bleiben. Ein Update von Abhängigkeiten
    kann sie trennen; das Skript bricht dann ab. Soll Dependabot den Pin ausnehmen?
12. **Die Markierung der Workspace-Suche** (M-13): im WASM oder im Backend? Entscheidet F9.
13. **Plattformen:** WebView2 und WKWebView: `instantiateStreaming`, MIME von `.wasm` über `tauri://` bzw.
    `http://tauri.localhost`. Bisher nur unter WebKitGTK geprüft.
