# Paritätsbericht: egui-App → Tauri 2

Wird je Feature fortgeschrieben. Grundlage ist [feature-inventar.md](feature-inventar.md). Die endgültige Fassung
entsteht in Phase 4.

## Stand je Feature

| Feature | Inventar | Stand | Commit |
|---|---|---|---|
| F0 Spike | – | entschieden: Variante A ([ADR-024](../architecture/decisions/ADR-024-the-backend-owns-the-note-text.md)) | `8181d28` |
| F1 Shell | M-01.2–6, M-03.1/3/5/6, M-04.7 ✔; M-01.1, M-02.1 und M-03.4 folgen mit F2, M-03.2 mit F9, M-03.7 mit F10 | fertig | siehe git log „F1“ |

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

Nicht testbar war in F1 der Screenshot des echten Tauri-Fensters: Es ist ein natives Wayland-Fenster, und `xdotool`,
`grim` und `import` fehlen. Geprüft wurde im Mock-Modus mit Playwright. Das echte Fenster lief ohne Fehler im
Rust-Log.

## Messwerte

| Messung | egui | Tauri | Quelle |
|---|---|---|---|
| Tippen Quellansicht, p99 (10'000 Zeilen) | 1,2 ms (Verarbeitung) | 7 ms (Verarbeitung, Spike) | ADR-012, ADR-024 |
| Tippen Facade, p99 (10'000 Zeilen) | 1,4 ms (Verarbeitung) | 9–10 ms (Verarbeitung, Spike) | ADR-012, ADR-024 |
