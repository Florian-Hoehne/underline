# Offene Fragen der Migration

Fragen, die bei der Migration aufkommen und die der egui-Code oder die Spezifikation nicht beantworten. Statt zu
raten, steht jede Frage hier, bis der Benutzer entscheidet. Fragen zur Produktspezifikation selbst gehören nach
`docs/open_questions.md` (`/open-questions`).

| ID | Feature | Frage | Bis zur Antwort | Status |
|---|---|---|---|---|
| Q-01 | F1 | Der Shortcut-Hinweis „Ctrl+,“ steht wie in egui in der Farbe `weak`. Das erreicht 3:1, aber in Solarized Light/Dark und Nord nicht 4.5:1. Gilt er als Fliesstext (dann Farbe `foreground`) oder als Hinweis? | bleibt `weak` (egui-Parität) | offen |
| Q-02 | F1 | In egui wird eine lange Statusmeldung abgeschnitten, in Tauri bricht sie um. Ist das so in Ordnung? | bricht um | offen |
| Q-03 | F2 | egui setzt die Panelbreiten bei jedem Neulesen der Einstellungsdatei zurück (`width_generation`). Soll das Event `settings` das auslösen oder ein eigenes Signal? | in F2 klären, egui-Verhalten nachbauen | offen |
