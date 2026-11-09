# Offene Fragen der Migration

Fragen, die bei der Migration aufkommen und die der egui-Code oder die Spezifikation nicht beantworten. Statt zu
raten, steht jede Frage hier, bis der Benutzer entscheidet. Fragen zur Produktspezifikation selbst gehören nach
`docs/open_questions.md` (`/open-questions`).

| ID | Feature | Frage | Bis zur Antwort | Status |
|---|---|---|---|---|
| Q-01 | F1 | Der Shortcut-Hinweis „Ctrl+,“ steht wie in egui in der Farbe `weak`. Das erreicht 3:1, aber in Solarized Light/Dark und Nord nicht 4.5:1. Gilt er als Fliesstext (dann Farbe `foreground`) oder als Hinweis? | bleibt `weak` (egui-Parität) | offen |
| Q-02 | F1 | In egui wird eine lange Statusmeldung abgeschnitten, in Tauri bricht sie um. Ist das so in Ordnung? | bricht um | offen |
| Q-03 | F2 | egui setzt die Panelbreiten bei jedem Neulesen der Einstellungsdatei zurück (`width_generation`). Soll das Event `settings` das auslösen oder ein eigenes Signal? | Gelöst in F2: `SettingsView` trägt `widthGeneration`. Der Zähler wächst wie in egui, wenn die Benutzerdatei (Poll) oder die Workspace-Datei (Watcher) neu gelesen wird; das Einstellungsfenster (F10) zählt ihn beim Ändern einer Breite hoch. Das Event `settings` geht auch dann raus, wenn sich nur der Zähler ändert. Das Fenster setzt die Übersicht dann auf `overviewWidth` zurück. Eine Schnelländerung (Theme, gezogene Breite) lässt die Breite stehen. | erledigt |
| Q-04 | F2 | Solange `ws_start` den Workspace vom Start öffnet, bleibt der Mittelbereich leer (`aria-busy`), ohne Text. egui öffnet vor dem ersten Frame und hat dafür keinen Text. Soll bei langem Öffnen (> 1 s) ein Hinweis erscheinen, und mit welchem Wortlaut? | leer, ohne Text | offen |
| Q-05 | F2 | egui zeigt Probleme in Dialogen (z. B. „… does not exist. Choose another directory …“) in seiner eigenen Warnfarbe (Orange, nicht aus der Palette, auf hellen Themes unter 4.5:1). Welche Farbe sollen sie haben? | Textfarbe mit einem Balken links in `weak` | offen |
| Q-06 | F2 | Die Übersicht lässt sich wie in egui nur mit der Maus verbreitern. NFR-011 nennt Menüs, Toolbar, Werkzeugpanel und Git, nicht die Übersicht. Soll der Rand per Tastatur (Pfeiltasten) bedienbar sein? | nur Maus | offen |
| Q-07 | F2 | Der Button „×“ eines Tabs hat in egui keinen Namen ausser „×“. In Tauri heisst er für Screenreader „Close {Titel}“ (nicht sichtbar). Ist der Wortlaut recht? | „Close {Titel}“ | offen |
