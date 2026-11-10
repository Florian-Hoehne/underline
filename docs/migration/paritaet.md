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
| F4 Eingabemodi, Statuszeile, Toolbar, Falten, Zeilennummern, Minimap | M-08.7, M-08.8, M-10.2 (ohne Link-/Bild-Dialog, F6), M-10.9 (Vim `:w`/`:q`/`:q!`/`:wq`/`:x`, Nano Ctrl+O/Ctrl+S/Ctrl+X), M-11.1–M-11.4, M-12.1–M-12.3, M-12.4 (Darstellung, mit den Treffern der Vim-/Nano-Suche; die Suche selbst folgt mit F9) umgesetzt, noch nicht von app-tester geprüft. Folgen: Link- und Bild-Dialog mit F6 (bis dahin gesperrt, Q-14), Copy on select selbst mit F6 (der Umschalter speichert schon), Falten in der Facade mit F5 | fertig | – |

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
| A-27 | F4 | Vim der Quellansicht (A-01) im Einzelnen: `codemirror-vim` ändert den Text zuerst im Fenster und schickt jede Änderung mit ihrer Revision ans Backend (`ed_apply`); ein Befehl, der mehrere Stellen ändert, geht als eine Änderung über den ganzen Bereich. Das Backend prüft alle Änderungen eines Aufrufs (Bereich, Reihenfolge, keine Stelle in einem Surrogatpaar), bevor es eine anwendet. Eine Antwort des Backends, die inzwischen kommt (Undo, Toolbar, Einfügen), wird über die noch nicht gesendeten Änderungen verschoben, wie bei CodeMirrors gemeinsamem Bearbeiten. Hat sich der Text im Backend sonst geändert (von aussen, später Agenten) oder passt eine Änderung nicht, wird sie verworfen und die Ansicht zeigt den Text des Backends. Undo-Schritte: Das Fenster markiert den Anfang jedes Vim-Befehls und jeder INSERT-Sitzung; jeder Befehl ist ein eigener Schritt, und das Tippen einer INSERT-Sitzung beginnt einen neuen Schritt, in ihr fasst das Backend wie in Normal wortweise zusammen (egui: dieselben Regeln über das Dokument, ohne Markierung). `u`, Ctrl+R, `:undo` und `:redo` nutzen die eine History des Backends. `"+` und `"*` lesen und schreiben die Systemzwischenablage über das Backend; ein Einfügen liest sie jedes Mal neu; `"+` steht als wartende Taste in der Statuszeile. Die Treffer der Suche markiert `codemirror-vim` selbst im sichtbaren Bereich, in den Farben von egui; Escape und `:noh` beenden die Markierung. Hinweise von `codemirror-vim` („3 lines yanked“) erscheinen in seiner Zeile unten im Editor in der Textfarbe und ändern den Modus der Statuszeile nicht; den Hinweis „(JavaScript regexp: set pcre)“ blendet das Fenster aus. Der Blockcursor hat die Textfarbe des Themes. In INSERT gelten die Tasten von Normal (M-08.4) über das Backend. In NORMAL und VISUAL belegt `codemirror-vim` Ctrl+A, B, C, D, E, F, I, N, O, P, Q, R, T, U, V, W, X, Y und Ctrl+[; davon fehlen dort die Toolbar-Tasten Ctrl+B (Bold), Ctrl+E (Code) und Ctrl+I (Italic); die übrigen Toolbar- und die Falttasten wirken (Q-15). | F-2; Q-15, Q-16 |
| A-28 | F4 | Die Statuszeile misst „unter 720 px“ und „unter 450 px“ an ihrer ganzen Breite; egui misst den Platz neben dem Badge. Die Auswahl „Keys:“ ist eine native Auswahlliste mit dem Text „Keys:“ davor (schmal ohne ihn, dann heisst sie für Screenreader „Keys“); egui zeigt „Keys: Vim“ als einen Text der Combo. In einem schmalen Editor fallen zuerst die Hinweise weg; reicht der Platz dann noch nicht, rückt der rechte Teil (Umschalter, Auswahl, Ort) in eine zweite Zeile, statt abgeschnitten zu werden (egui schneidet ab). Die wartenden Tasten bleiben lesbar (höchstens 40 % der Breite). Der Ort ist mindestens 14 Zeichen breit. Das Badge ist für Screenreader ein höflicher Status. Gedrückte Umschalter haben zusätzlich zur Farbe einen Rand. | Webview; NFR-010, NFR-011 (UX-Review F4) |
| A-29 | F4 | Die Zeilennummern rechts sind eine eigene Ebene neben dem Text (CodeMirror hat rechts keinen Gutter); die Breite folgt der Stellenzahl wie links. | Plan Abschnitt 6 |
| A-30 | F4 | Die Sektionen kommen nach der Eingabe als Event (`ed-sections`, ADR-024): Der Pfeil und das „…“ erscheinen einige Millisekunden nach der Eingabe; solange nichts gefaltet ist, sucht das Backend höchstens alle 300 ms neu (wie egui). Ist etwas gefaltet, rechnet eine Eingabe, die Zeilen hinzufügt oder entfernt, die Sektionen sofort neu (wie egui), damit Auf/Ab nie in einen gefalteten Bereich gehen. | ADR-024 |
| A-31 | F4 | Die Vim-Tasten werden erst geladen, wenn der Modus Vim gewählt ist (eigenes Modul); der erste Wechsel zu Vim wartet darauf. | NFR-001: der Start lädt rund 190 kB weniger. |
| A-32 | F4 | Die Minimap legt die Balken nach Zeilenlänge aus (eine Zeile je angefangene Textbreite, wie CodeMirror Zeilen schätzt, die es nicht gezeichnet hat) und streckt sie auf die Höhe des Inhalts; egui nimmt die gemessene Höhe jeder Zeile. Bei sehr unterschiedlich langen umbrochenen Zeilen können Balken und Treffer um wenige Pixel von der Lage im Text abweichen. | ADR-024, Messpunkt F4: die Lage jeder Zeile bei CodeMirror zu erfragen kostete beim Tippen bis 10 ms. |
| A-33 | F4 | F6 und Shift+F6 wechseln in jedem Eingabemodus zwischen Tab-Leiste, Ansichtsschalter mit Toolbar, Text und Statuszeile; so verlässt man den Text auch in Vim und Nano, wo Escape und Tab im Text bleiben. egui hat diese Taste nicht. „Link“ und „Image“ sind bis F6 gesperrt, ihre Tasten (Ctrl+K, Ctrl+Shift+I) tun nichts; Nanos Ctrl+K schneidet weiter. Das „…“ einer gefalteten Überschrift steht in der Textfarbe und heisst für Screenreader „folded“; der Pfeil nimmt Klicks über die ganze Breite des Gutters und 24 px hoch an und bleibt sichtbar, wenn der Zeiger in den Gutter geht. | NFR-011 (Tastaturfalle), Q-14, Q-17, UX-Review F4 |
| A-34 | F4 | Die markierten Treffer der Nano-Suche gehen höchstens alle 300 ms neu ans Fenster, solange der Text sich ändert, und höchstens 10'000 davon (mit der Gesamtzahl). | ADR-024 |

Nicht testbar war in F1 der Screenshot des echten Tauri-Fensters: Es ist ein natives Wayland-Fenster, und `xdotool`,
`grim` und `import` fehlen. Geprüft wurde im Mock-Modus mit Playwright. Das echte Fenster lief ohne Fehler im
Rust-Log.

## Messwerte

| Messung | egui | Tauri | Quelle |
|---|---|---|---|
| Tippen Quellansicht, p99 (10'000 Zeilen) | 1,2 ms (Verarbeitung) | 7 ms (Verarbeitung, Spike) | ADR-012, ADR-024 |
| Tippen Facade, p99 (10'000 Zeilen) | 1,4 ms (Verarbeitung) | 9–10 ms (Verarbeitung, Spike) | ADR-012, ADR-024 |
| Tippen Quellansicht F3, p99 (10'000 Zeilen) | 1,2 ms (Verarbeitung) | 5–7 ms ohne, 7–10 ms mit Hervorhebung (Verarbeitung) | Messpunkt F3, unten |
| Tippen Quellansicht F4, p99 (10'000 Zeilen, Hervorhebung, Zeilennummern, Minimap, eine Sektion gefaltet) | 1,2 ms (Verarbeitung) | Normal 9–12 ms, Vim 9 ms, Nano 9 ms (Verarbeitung) | Messpunkt F4, unten |

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

### Messpunkt F4: Eingabemodi mit Zeilennummern, Minimap und Falten

Methode wie F3, Build mit `VITE_UL_BENCH=1 VITE_UL_BENCH_F4=1` (Treiber `bench.ts`, `measureF4`), die App mit
eigenem `HOME` gestartet, damit die Benutzereinstellungen unberührt bleiben. Je Modus öffnet der Treiber eine
Quellansicht mit Hervorhebung und Zeilennummern links, daneben die Minimap (`Minimap.svelte`), und faltet die erste
Sektion ab 3 % der Notiz. Der Modus wird über die Einstellungen gesetzt, da das Backend ihn dort liest. In Vim wird
mit `A` in INSERT getippt. 500 gezählte Tasten je Stelle.

| Modus | Stelle (Zeile) | median | p95 | p99 | max | nächster Paint p99 | im ersten Frame |
|---|---|---|---|---|---|---|---|
| Normal | Anfang (200) | 4 | 7 | 10 | 16 | 28 | 88 % |
| Normal | Mitte (5'001) | 4 | 8 | 9 | 17 | 30 | 86 % |
| Normal | Ende (9'801) | 4 | 7 | 12 | 19 | 29 | 85 % |
| Vim | Anfang (200) | 4 | 8 | 9 | 31 | 32 | 87 % |
| Vim | Mitte (5'001) | 4 | 8 | 9 | 13 | 31 | 86 % |
| Vim | Ende (9'801) | 4 | 8 | 9 | 15 | 31 | 87 % |
| Nano | Anfang (200) | 4 | 7 | 9 | 21 | 29 | 90 % |
| Nano | Mitte (5'001) | 4 | 6 | 9 | 11 | 29 | 88 % |
| Nano | Ende (9'801) | 4 | 8 | 9 | 12 | 31 | 88 % |

Werte in ms, Lauf nach den Korrekturen aus dem Review F4. Öffnen: 8–30 ms. Die ersten beiden Läufe lagen bei p99 15–29 ms bzw. 10–18 ms. Ursachen, mit einem
Diagnoselauf je Teil (`VITE_UL_BENCH_DIAG=1`) gefunden und behoben: Die Minimap fragte CodeMirror alle 150 ms nach
der Lage jeder der 10'000 Zeilen (jetzt aus Zeilenlängen geschätzt wie CodeMirror selbst, auf die Inhaltshöhe
gestreckt); das Backend suchte bei gefalteter Sektion die Sektionen bei jeder Taste neu (jetzt nur, wenn sich die
Zahl der Zeilen ändert, sonst höchstens alle 300 ms im Tick). Ohne Zeilennummern, Minimap und Falten liegt p99 wie in
F3 bei 9 ms. Die Werte bleiben unter 16 ms p99; 85–90 % der Tasten kommen in den nächsten
Frame. Nicht gemessen: echte OS-Tasten, WebView2, WKWebView.
