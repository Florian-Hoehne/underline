# Paritätsbericht: egui-App → Tauri 2

Wird je Feature fortgeschrieben. Grundlage ist [feature-inventar.md](feature-inventar.md). Die endgültige Fassung
entsteht in Phase 4.

## Stand je Feature

| Feature | Inventar | Stand | Commit |
|---|---|---|---|
| F0 Spike | – | entschieden: Variante A ([ADR-024](../architecture/decisions/ADR-024-the-backend-owns-the-note-text.md)) | `8181d28` |
| F1 Shell | M-01.2–6, M-03.1/3/5/6, M-04.7 ✔; M-01.1, M-02.1 und M-03.4 folgen mit F2, M-03.2 mit F9, M-03.7 mit F10 | fertig | siehe git log „F1“ |
| F2 Workspace | M-01.1, M-02.1, M-03.4, M-05.1, M-05.2, M-06.1–3 (ohne Editor-Teile), M-06.5 (nur Zustand), M-06.7, M-21.1–3, M-22.1, M-22.2 ✔ (M-06.1–3 und M-21.3 werden mit F3 bzw. den Dienst-Features abgehakt). Folgen: M-05.3 mit F12, M-05.4–6 und „New note…“ mit F8, M-06.4 mit F8, M-06.6 mit F16, Tab-Marken „ ⚠“/„ •“ und die Rückfrage beim Schliessen mit F3, Tab „Todos (N)“ mit F15, Syncer/Searcher/ChangeTracker mit F13/F9/F12, D-05 mit F14 | fertig | siehe git log „F2“ |

| F3 Quellansicht | M-06.1 (Marken), M-06.3, M-07.5, M-08.1–M-08.4, M-08.6 (Text), M-08.9, M-08.10, M-10.1, M-10.8, M-10.9 (nur `Store`, Befehl `tab_store`), M-16.1, M-16.2, M-21.4 (Speichern und Rückfrage) umgesetzt, noch nicht von app-tester geprüft. Folgen: M-07.2 (Facade gesperrt bis F5, Q-12), M-08.5 nur synthetisch geprüft (Q-09), M-08.7/M-08.8/M-10.2/Statuszeile/Minimap/Vim/Nano mit F4, Bilder in der Zwischenablage mit F6, Sync vor dem Schliessen mit F13, Werkzeugpanel ohne aktiven Tab mit F12/F15 | umgesetzt, Prüfung offen | – |

## Gewollte Abweichungen

Vom Benutzer entschieden:

| ID | Abweichung | Entscheidung |
|---|---|---|
| A-01 | Vim in der Quellansicht über `codemirror-vim`. Die Quellansicht kann mehr Befehle, und Vim verhält sich in Quelle und Facade nicht mehr identisch. | F-2 |
| A-02 | Die Defekte D-01 bis D-09, D-11, D-17 und D-18 werden in der Tauri-App behoben. | F-4, Spike F0 |

## Abweichungen und Lücken, die noch offen sind

| ID | Feature | Abweichung | Grund |
|---|---|---|---|
| A-03 | F1 | Das Fenster startet verborgen und erscheint, sobald das Theme gesetzt ist; nach spätestens 3 s erscheint es in jedem Fall. | Verhindert ein Aufblitzen in falschen Farben; egui zeichnet sofort im Theme. |
| A-04 | F1 | Eine lange Statusmeldung bricht um (höchstens 4 Zeilen, danach scrollt sie), statt abgeschnitten zu werden. | Q-02, offen |
| A-05 | F1 | Ist das Fenster sehr schmal, bricht die Menüleiste in eine zweite Zeile um. | egui schneidet ab; im Webview gibt es keinen horizontalen Scroll. |
| A-06 | F1 | Die Ränder der Bedienelemente nutzen die Palettenfarbe `weak` (egui-Buttons haben keinen Rand). | Kontrast 3:1 nach NFR-010 |
| A-07 | F1 | Eine Panic im Hintergrund-Tick wird geloggt, und der Tick läuft weiter. | Robustheit; egui hat keinen Tick-Thread. |
| A-08 | F2 | Open- und Create-Dialog schliessen mit Escape (Behebung D-02 für diese beiden Dialoge, Teil von A-02). | F-4 |
| A-09 | F2 | Die Dialoge erscheinen oben mittig über dem Mittelbereich und lassen sich nicht verschieben; egui-Fenster sind verschiebbar. | Nicht-modal bleibt; Verschieben bringt im Webview wenig. |
| A-10 | F2 | Ein Dialog gibt die Tastatur an das leere Pfadfeld wie egui, sonst an sich selbst, damit Tab im Dialog weitergeht. Beim Schliessen geht die Tastatur dorthin zurück, wo sie vorher war. Während der Backend-Antwort sind die Buttons gesperrt. | Tastaturbedienung; egui fokussiert sonst nichts. |
| A-11 | F2 | Während der Workspace vom Start öffnet, bleibt der Mittelbereich leer, statt dass die App blockiert. | Plan Abschnitt 14, Q-04 |
| A-12 | F2 | Dialogprobleme stehen in der Textfarbe mit einem Balken links statt in egui-Orange. | NFR-010, Q-05 |
| A-13 | F2 | Die Übersicht ist höchstens so breit, dass rechts 120 px bleiben. | Im Webview gibt es keinen horizontalen Scroll des Fensters. |
| A-14 | F2 | Für die Prüfung „Home-Verzeichnis“ und für bekannte Workspaces (UC-002 BR-007) gilt der Ordner der Benutzer-Einstellungsdatei der Sitzung; nur ohne sie das Home-Verzeichnis des Systems. egui fragt immer das System. Im Betrieb ist es derselbe Ordner. | Prüfung und Einstellungsdatei bleiben im Gleichschritt; Tests können ein eigenes Home verwenden. |
| A-15 | F2 | Der aktive Eintrag der Übersicht hat links einen 3-px-Balken, der aktive Tab eine 2-px-Linie unten, beide in der Textfarbe. Das Überfahren mit der Maus ist schwächer getönt als die Wahl (egui tönt beides gleich). | NFR-010: Die Wahlfarbe hebt sich nur 1.4–2.0:1 ab; der Zustand darf nicht nur an der Farbe hängen. |
| A-16 | F2 | Tastatur: Enter löst in jedem Schritt des Create-Dialogs die erste Schaltfläche aus (egui nur im ersten Schritt); nach einem Problem steht die Tastatur im Pfadfeld, mit markiertem Text; „Open“ und „Next“ sind ohne Pfad gesperrt; nach dem Schliessen eines Tabs geht die Tastatur zum Nachbartab, sonst zur Übersicht; nach dem Öffnen eines Workspace und nach einem Klick in der Übersicht steht sie in der Übersicht. | UX-Review F2; NFR-011. |
| A-17 | F3 | Die Tab-Leiste ist ein `tablist` mit wanderndem `tabindex`; „×“ liegt nicht in der Tab-Reihenfolge, Delete schliesst den fokussierten Tab. | Plan Abschnitt 14, Q-08 |
| A-18 | F3 | Nur echte Änderungen markieren den Tab und lösen das Speichern aus (egui: auch Backspace am Anfang, Delete am Ende, Einfügen ohne Text). | Q-11 |
| A-19 | F3 | Die Facade fehlt bis F5: jeder Tab zeigt die Quellansicht, „Facade“ ist gesperrt, die gemerkte Ansicht bleibt. | Q-12 |
| A-20 | F3 | „Not stored: …“ steht halbfett in der Textfarbe mit einem Balken links statt in egui-Rot. | Q-10 (wie Q-05) |
| A-21 | F3 | Eine IME-Komposition ist im Text sichtbar, bis sie fertig ist. | Q-09; CodeMirror zeigt sie. |
| A-22 | F3 | Die Eingaben gehen nacheinander als synchrone Befehle an das Backend (Hauptthread). Hält der Tick oder ein anderer Befehl die Sitzung, wartet die Eingabe. Ohne Sitzung laufen seit dem Review: das Lesen und Prüfen des Verzeichnisses beim Öffnen (`ws_start`, `ws_open`), das Anlegen samt git (`ws_create`), das Lesen und Schreiben der Zwischenablage und das Laden der Grammatiken für die Hervorhebung (eigener Thread, beim Start und nach einem Themewechsel; bis dahin bleiben die Zeilen schlicht). Noch mit gehaltener Sitzung: das Speichern vor einem Wechsel oder Schliessen (`flush_all`, samt Warten auf ein laufendes Hintergrund-Speichern, höchstens 5 s), der Wechsel selbst (Watcher starten, die Notizen der Tabs lesen, alte gelöschte Notizen aufräumen), das Lesen der Eigenschaften, das Ablegen der Kopie bei Änderungen von aussen und je Tick höchstens 4 ms Hervorhebung. | ADR-024; der Spike mass synchrone Befehle als die schnellsten. |
| A-23 | F3 | Die Eigenschaften (M-16.2) werden beim Öffnen, Aktivieren, Speichern und bei Änderungen von aussen gelesen, nicht in jedem Frame. Eine Änderung der Datei, die der Watcher nicht meldet, zeigt sich erst beim nächsten dieser Anlässe. | Vorgabe F3 |
| A-24 | F3 | Die Hervorhebung rechnet im Tick höchstens 4 ms und macht sofort weiter; bis dahin bleiben Zeilen schlicht (wie egui mit 4 ms je Frame). Nach einem Themewechsel kommen die Farben je sichtbarem Bereich neu. | ADR-010, ADR-024 |
| A-25 | F3 | Die Scrollposition eines Tabs bleibt beim Tabwechsel erhalten, solange das Fenster lebt (CodeMirror wird je Tab neu aufgebaut; egui behält das Widget). | wie egui im Ergebnis |
| A-26 | F3 | Ein `\r\n`-Paar ist für Pfeile, Backspace und Delete ein Schritt (egui: zwei). Die Datei behält ihre Zeilenenden Byte für Byte (NFR-007); Enter fügt wie in egui `\n` ein. Ins Fenster gehen Text und Offsets mit vereinheitlichten Zeilenumbrüchen (CodeMirror zählt jeden Umbruch als eine Einheit). | MUSS 1 aus dem Review F3; im Webview gibt es keine Stelle zwischen `\r` und `\n`. |

Nicht testbar war in F1 der Screenshot des echten Tauri-Fensters: Es ist ein natives Wayland-Fenster, und `xdotool`,
`grim` und `import` fehlen. Geprüft wurde im Mock-Modus mit Playwright. Das echte Fenster lief ohne Fehler im
Rust-Log.

## Messwerte

| Messung | egui | Tauri | Quelle |
|---|---|---|---|
| Tippen Quellansicht, p99 (10'000 Zeilen) | 1,2 ms (Verarbeitung) | 7 ms (Verarbeitung, Spike) | ADR-012, ADR-024 |
| Tippen Facade, p99 (10'000 Zeilen) | 1,4 ms (Verarbeitung) | 9–10 ms (Verarbeitung, Spike) | ADR-012, ADR-024 |
| Tippen Quellansicht F3, p99 (10'000 Zeilen) | 1,2 ms (Verarbeitung) | 5–7 ms ohne, 7–10 ms mit Hervorhebung (Verarbeitung) | Messpunkt F3, unten |

### Messpunkt F3: Quellansicht (ADR-024)

Methode wie im Spike F0 (`spikes/f0-editor/RESULTS.md`): In-Page-Treiber (`ui/src/lib/editor/bench.ts`, nur in
einem Build mit `VITE_UL_BENCH=1`), Release-Build mit eingebettetem Frontend
(`cargo build --release -p underline-tauri --features tauri/custom-protocol`), WebKitGTK 2.52 auf Wayland,
dieselbe Maschine wie der Spike. Die Testnotiz ist die des Spikes (10'000 Zeilen, `write_note`), als `Home.md`
eines leeren Workspace. Der Treiber öffnet eine eigene Quellansicht derselben Klasse über dem Fenster und schickt
je Stelle 30 Tasten zum Aufwärmen und 500 gezählte Tasten als synthetische `beforeinput`-Ereignisse, jede nach dem
Zeichnen der vorigen plus 0–17 ms Jitter. „Verarbeitung“ läuft vom Start der Eingabe in der Warteschlange bis der
Patch in CodeMirror ist (IPC-Befehl `ed_input` eingeschlossen). „Mit Hervorhebung“: die Ansicht meldet ihre Zeilen,
und das Backend schickt nach jeder Taste `ed-highlight`. Auflösung von `performance.now()`: 1 ms.

| Hervorhebung | Stelle (Zeile) | median | p95 | p99 | max | nächster Paint p99 | im ersten Frame |
|---|---|---|---|---|---|---|---|
| aus | Anfang (200) | 3 | 4 | 5 | 12 | 24 | 88 % |
| aus | Mitte (5'001) | 4 | 5 | 6 | 9 | 26 | 88 % |
| aus | Ende (9'801) | 3 | 6 | 7 | 15 | 27 | 88 % |
| an | Anfang (200) | 4 | 7 | 10 | 13 | 30 | 87 % |
| an | Mitte (5'001) | 3 | 6 | 7 | 8 | 27 | 91 % |
| an | Ende (9'801) | 3 | 6 | 8 | 33 | 28 | 90 % |

Werte in ms, je 500 Tasten. Öffnen der Quellansicht (`ed_open` und CodeMirror aufbauen): 15–24 ms. Die
Quellansicht bleibt unter 16 ms p99 und bringt 87–91 % der Tasten in den nächsten Frame (Spike: 87–89 %). Das
Anwenden der Farben kostet Zeit zwischen den Tasten, nicht in der gemessenen Verarbeitung. Nicht gemessen:
echte OS-Tasten (`xdotool` fehlt, kein `sudo`), Tottasten, eine echte IME, WebView2 und WKWebView.
