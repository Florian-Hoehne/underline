# Paritätsbericht: egui-App → Tauri 2

Wird je Feature fortgeschrieben. Grundlage ist [feature-inventar.md](feature-inventar.md). Die endgültige Fassung
entsteht in Phase 4.

## Stand je Feature

| Feature | Inventar | Stand | Commit |
|---|---|---|---|
| F0 Spike | – | entschieden: Variante A ([ADR-024](../architecture/decisions/ADR-024-the-backend-owns-the-note-text.md)) | `8181d28` |
| F1 Shell | M-01.2–6, M-03.1/3/5/6, M-04.7 ✔; M-01.1, M-02.1 und M-03.4 folgen mit F2, M-03.2 mit F9, M-03.7 mit F10 | fertig | siehe git log „F1“ |
| F2 Workspace | M-01.1, M-02.1, M-03.4, M-05.1, M-05.2, M-06.1–3 (ohne Editor-Teile), M-06.5 (nur Zustand), M-06.7, M-21.1–3, M-22.1, M-22.2 ✔ (M-06.1–3 und M-21.3 werden mit F3 bzw. den Dienst-Features abgehakt). Folgen: M-05.3 mit F12, M-05.4–6 und „New note…“ mit F8, M-06.4 mit F8, M-06.6 mit F16, Tab-Marken „ ⚠“/„ •“ und die Rückfrage beim Schliessen mit F3, Tab „Todos (N)“ mit F15, Syncer/Searcher/ChangeTracker mit F13/F9/F12, D-05 mit F14 | fertig | siehe git log „F2“ |

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

Nicht testbar war in F1 der Screenshot des echten Tauri-Fensters: Es ist ein natives Wayland-Fenster, und `xdotool`,
`grim` und `import` fehlen. Geprüft wurde im Mock-Modus mit Playwright. Das echte Fenster lief ohne Fehler im
Rust-Log.

## Messwerte

| Messung | egui | Tauri | Quelle |
|---|---|---|---|
| Tippen Quellansicht, p99 (10'000 Zeilen) | 1,2 ms (Verarbeitung) | 7 ms (Verarbeitung, Spike) | ADR-012, ADR-024 |
| Tippen Facade, p99 (10'000 Zeilen) | 1,4 ms (Verarbeitung) | 9–10 ms (Verarbeitung, Spike) | ADR-012, ADR-024 |
