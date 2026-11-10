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
| F5a Facade: Darstellung | M-07.2, M-07.4 (nur der Hinweis; Folgen mit F7), M-08.8 (Facade-Hälfte), M-09.1–M-09.9, M-09.16, M-12.2 (Facade) umgesetzt; M-09.15 nur so weit, dass die Webview nichts selbst lädt (Web-Bilder bis Q-19 nicht geladen). Noch nicht von app-tester geprüft. Folgen: Bearbeiten in der Facade (M-09.10–M-09.13, Vim/Nano in der Facade) mit F5b, Einfügen/Kopieren (M-09.14) mit F6, Links folgen mit F7, Suchtreffer in der Facade mit F9 | umgesetzt, Prüfung offen | – |
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
| A-19 | F3 | Die Facade fehlt bis F5: jeder Tab zeigt die Quellansicht, „Facade“ ist gesperrt, die gemerkte Ansicht bleibt. *Erledigt mit F5a: der Umschalter wirkt, ein neuer Tab zeigt wie in egui die Facade.* | Q-12 |
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
| A-35 | F5a | Die Minimap der Facade misst die Layoutzeilen nicht: Je Block wird seine Höhe in Zeilen seiner Zeilenhöhe geteilt; ein Code-Block hat seine Zeilen, ein Textblock volle Balken bis auf den letzten. Eine Abbildung ist eine Fläche über den ganzen Block, auch über den Markup-Rahmen darunter (egui: nur über das Bild). | ADR-024, wie A-32 |
| A-36 | F5a | Tabellen: Die Spaltenbreiten kommen aus der Tabellen-Auslegung der Webview (nach Inhalt, zu breit wird zwischen Mindest- und Wunschbreite verteilt); egui verkleinert alle Spalten im selben Verhältnis, jede mindestens 40 px. Abstände, Gitter, Kopfzeile und Ausrichtung wie egui. | Webview |
| A-37 | F5a | Bilder kommen nur über `ul-asset` (ADR-023): Ein Bild ausserhalb des Workspace (voller Pfad, `file://`) oder in einem versteckten Ordner (`.git`, `.underline`, …) zeigt „{target} lies outside the workspace“, egui zeigt es (Q-20). Ist das Ziel kein Bild (nach der Endung), steht „The image {alt} cannot be shown: the image cannot be read: The image format could not be determined“ (der Text der Bildbibliothek; egui nennt den Grund des Decoders). Kann die Webview ein Bild nicht lesen, steht „The image {alt} cannot be shown: the image cannot be read“ ohne den Grund des Decoders (egui nennt ihn). Grosse Bilder verkleinert egui auf 4096 px, die Webview nicht. Einer auf der Platte geänderten Bilddatei folgt die Facade, wenn ihre Blöcke neu entstehen (nächste Änderung der Notiz); egui prüft in jedem Frame. | ADR-023 |
| A-38 | F5a | Web-Bilder werden nicht geladen; an ihrer Stelle steht egui's Text für ein nicht ladbares Web-Bild „The image {alt} cannot be shown: {address} could not be loaded“. egui lädt sie ohne Rückfrage. | Q-19 |
| A-39 | F5a | Diagramme erscheinen als SVG (kein Rastern, das Vergrössern rendert nichts nach). Das Backend rendert alle Diagramme der Notiz im Hintergrund, die nächsten am Anfang zuerst; egui rendert die sichtbaren, das zuletzt verlangte zuerst. Meldungen der Abbildungen: ein Fehler in der Textfarbe, halbfett mit Balken (wie A-12/A-20), egui in Rot; „Rendering…“/„Loading…“ wie egui in der schwachen Farbe. | ADR-009, Q-05 |
| A-40 | F5a | Bis F5b nimmt die Facade keine Eingabe an, die den Text ändert (Tippen, Enter, Löschen, Einfügen, Ziehen); Cursor und Auswahl setzt man mit Maus und Pfeilen (die Pfeile der Webview, nicht egui's Regeln M-09.13), sie gehen ans Backend. Escape führt in jedem Eingabemodus zum Tab der Notiz (Vim und Nano der Facade kommen mit F5b), Tab tut nichts. Toolbar und Falttasten wirken. Kopieren ist bis F6 das der Webview. Ein Klick auf eine Abbildung setzt den Cursor an den Anfang ihres Markups; der Klick auf die Lupe nicht. | Feature-Schnitt F5a/F5b |
| A-41 | F5a | Die Facade ist virtualisiert (ADR-024): nur Blöcke nahe der Sicht stehen im DOM, die übrigen als Höhe, geschätzt bis gezeichnet. Ändert ein Block oberhalb der Sicht seine Höhe, verschiebt sich die Scrollposition um ebenso viel. Die Scrollposition je Notiz bleibt wie A-25. Der Fokus zeigt sich wie in der Quellansicht als feine Linie um den Text. | ADR-024 |
| A-42 | F5a | Hinweise ohne egui-Gegenstück: Der Hinweis zu einem Link ist ein eigener, sofort erscheinender Kasten am Zeiger, Text wie egui. Die vergrösserte Ansicht ist ein modaler Dialog „Enlarged picture“, ihr Schliessknopf heisst für Screenreader „Close“, Tab bleibt im Dialog; Pinch kommt als Rad mit Ctrl oder als Geste der Webview. Die Lupe heisst für Screenreader „Enlarge picture“, ihr Hover-Text ist egui's „Enlarge (double click)“. Bilder tragen ihren Alternativtext, Diagramme ihre Beschriftung oder „Mermaid diagram“/„PlantUML diagram“. Der Text der Facade hat wie die Quellansicht die Beschreibung „Escape leaves the text“ und ist bis F5b schreibgeschützt (`aria-readonly`). Der Faltpfeil ist wie in der Quellansicht nur für den Zeiger, „…“ heisst „folded“. | NFR-011, Q-22 |
| A-43 | F5a | Zeilenhöhe der Facade 1,25 der Schriftgrösse (egui: Zeilenhöhe der Schrift). Die 4 px Code-Hintergrund liegen innerhalb des Blocks statt über dem Abstand; die Abstände zwischen den Blöcken (8 px, Abbildungen 16, Trennlinie 24) wie egui. | Webview |
| A-44 | F5a | Links der Blöcke stehen 24 px statt egui's 12 px, damit der Faltpfeil den Zeiger auf 24 × 24 px annimmt, ohne über dem Text zu liegen (Glyphe wie egui). Die Format-Anzeige steht auf dem Hintergrund der Facade, damit Text darunter nicht durchscheint. | WCAG 2.2 Zielgrösse, UX-Review F5a |
| A-45 | F5a | Ctrl+Enter (Cmd+Enter auf macOS) vergrössert die Abbildung, in der der Cursor steht; egui öffnet die Vergrösserung nur mit der Maus. | NFR-011, Q-23 |
| A-46 | F5a | Umschalter und Formatierungs-Toolbar bleiben eine Zeile: Ist der Editor zu schmal, scrollt die Zeile waagrecht, statt umzubrechen (egui bricht um). | UX-Review F5a |
| A-47 | F5a | Die Blöcke der Facade entstehen nicht unter der Sitzung (ADR-024): Der Tick gibt eine Momentaufnahme des Texts an einen eigenen Thread und nimmt das Ergebnis mit einem späteren Tick, aber nur, wenn es noch zum Text des Tabs passt und kein späteres schon genommen wurde; sonst entsteht es neu. Eine Änderung zeigt sich so einen Tick später; wer schneller tippt, als die Blöcke entstehen, sieht die Facade erst in der Pause nachziehen (F5b parst inkrementell). Ein fertiges Diagramm ändert nur seine Abbildung, unter der Sitzung. Die Adresse eines Diagramms trägt seine Version, ein gecachtes SVG bleibt höchstens bis 128 Diagramme oder 32 MB. Startet der Diagramm-Dienst nicht oder endet er, sagen die Diagramme „… cannot be shown: the diagram could not be rendered: …“ statt ewig „Rendering…“. | ADR-024, Review F5a |

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
| Facade öffnen, Median (10'000 Zeilen, 3'697 Blöcke) | – | 83 ms bis gezeichnet; `fa_open` allein 216 ms beim ersten Mal, 38 ms danach | Messpunkt F5a, unten |
| Wechsel Facade → Quelle / Quelle → Facade, Median | – | 29 ms / 69 ms | Messpunkt F5a, unten |

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

### Messpunkt F5a: Facade öffnen, Ansicht wechseln, scrollen

Methode wie F3/F4 (Release-Build mit eingebettetem Frontend, `VITE_UL_BENCH=1 VITE_UL_BENCH_F5=1`, Treiber
`ui/src/lib/facade/bench.ts`, eigenes `HOME`, Testnotiz des Spikes mit 10'000 Zeilen als `Home.md`, WebKitGTK 2.52
auf Wayland, dieselbe Maschine). Gemessen wird bis nach dem zweiten Frame nach der Änderung (zwei
`requestAnimationFrame` und eine Task); das sind bei 60 Hz mindestens 17–33 ms, auch ohne Arbeit.

| Messung | median | p95 | max | n |
|---|---|---|---|---|
| `fa_open` allein, erstes Mal (Parsen, Umwandeln, Farben der Code-Blöcke, IPC, JSON) | 216 | – | – | 1 |
| `fa_open` allein, Blöcke schon da (IPC und JSON von 2,7 MB) | 38 | – | – | 1 |
| Facade zeigen, bis die ersten Blöcke gezeichnet sind | 83 | 104 | 104 | 5 |
| Wechsel Facade → Quellansicht (`tab_set_view`, CodeMirror aufbauen) | 29 | 67 | 67 | 5 |
| Wechsel Quellansicht → Facade (`tab_set_view`, `fa_open`, zeichnen) | 69 | 159 | 159 | 5 |
| Eine Bildschirmhöhe weiter scrollen, bis gezeichnet | 34 | 51 | 682 | 319 |

Werte in ms, Lauf nach dem Review F5a: Parsen und Umwandeln laufen jetzt ohne die Sitzung (A-47); die Sitzung ist
für `fa_open` nur für die Momentaufnahme des Texts und das Übernehmen gehalten, der Tick hält sie nur für die
Momentaufnahme (eine Kopie der Rope). Der erste Lauf vor dem Review: `fa_open` 196/37, Öffnen 77, Wechsel 33/67,
Scrollen 33/49/548. Im DOM standen höchstens 61 der 3'697 Blöcke. Ein erster Lauf ergab beim Wechsel zur Facade bis
1'545 ms: Jedes fertige Diagramm baute alle Blöcke neu (unter der Sitzung), und der Wechsel zur Quelle verwarf die
Blöcke samt Farben. Behoben: Ein fertiges Diagramm ändert nur seine Abbildung, die Farben der Code-Blöcke hält die
Sitzung, und die Blöcke bleiben beim Wechsel zur Quelle erhalten. Der Ausreisser beim Scrollen (548 bzw. 682 ms, je einmal) fiel
in die Zeit, in der die Diagramme der Notiz im Hintergrund fertig wurden. Offen für F5b: Die Blöcke gehen beim Öffnen
ganz über IPC (2,7 MB JSON, rund 35 ms); bei Bedarf werden nur die Blöcke um die Sicht geschickt. Nicht gemessen:
WebView2, WKWebView, echte OS-Eingaben.

