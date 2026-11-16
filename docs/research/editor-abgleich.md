# Abgleich: Underline gegen Obsidian, VS Code und GNOME Text Editor

Stand 2026-11-11. Verglichen wird, was ein Benutzer in den drei Editoren täglich nutzt, mit dem, was Underline nach
der Migration kann (`docs/requirements.md`, `docs/migration/feature-inventar.md`). Der Vergleich gilt dem Verhalten,
nicht der Technik. „Ubuntu Text Editor“ ist der GNOME Text Editor (gnome-text-editor), der Standard-Editor von
Ubuntu.

Legende: ✔ vorhanden · ◐ teilweise · ✘ fehlt · – passt nicht zum Produkt (Vision, „Out of scope“)

## Navigation und Verknüpfung

| Funktion | Obsidian | VS Code | GNOME TE | Underline | Bemerkung |
|---|---|---|---|---|---|
| Notiz per Name schnell öffnen (Quick Switcher, Ctrl+O / Ctrl+P) | ✔ | ✔ | ✘ | ✘ | Übersicht hat Filter? nein; nur Volltextsuche (Ctrl+Shift+F) |
| Befehlspalette (alle Befehle per Tastatur suchen) | ✔ | ✔ | ✘ | ✘ | Menüs sind per Tastatur erreichbar (NFR-011), aber ohne Suche |
| Wiki-Links `[[Notiz]]` mit Vervollständigung | ✔ | ◐ (Erweiterung) | ✘ | ✘ | Underline kennt nur Markdown-/AsciiDoc-Links; GitHub rendert `[[…]]` nicht (C-003) |
| Link-Vervollständigung beim Tippen eines Links | ✔ | ◐ | ✘ | ◐ | nur über den Dialog „Insert link“ (M-10.3) |
| Backlinks („welche Notizen verlinken hierher“) | ✔ | ◐ | ✘ | ◐ | die Zahl erscheint nur beim Löschen (M-14.3) |
| Ausgehende Links, unverlinkte Erwähnungen | ✔ | ✘ | ✘ | ✘ | |
| Tags (`#tag`) und Suche danach | ✔ | ✘ | ✘ | ✘ | |
| Eigenschaften / Frontmatter (YAML-Kopf) als Felder | ✔ | ◐ | ✘ | ✘ | Markdown-Frontmatter wird heute als Text gezeigt |
| Graph-Ansicht | ✔ | ✘ | ✘ | ✘ | |
| Ordnerbaum mit Verschieben per Drag-and-drop | ✔ | ✔ | ✘ | ◐ | Übersicht ist eine Liste „zuletzt geöffnet“, Ordner nur im Namen |
| Lesezeichen / angeheftete Notizen bzw. Tabs | ✔ | ✔ | ✘ | ✘ | |
| Zurück / Vorwärts durch besuchte Stellen | ✔ | ✔ | ✘ | ✘ | |
| Gehe zu Zeile (Ctrl+G / Ctrl+I) | ✘ | ✔ | ✔ | ◐ | nur Vim `:N` / `NG` |

## Bearbeiten

| Funktion | Obsidian | VS Code | GNOME TE | Underline | Bemerkung |
|---|---|---|---|---|---|
| Suchen in der Notiz | ✔ | ✔ | ✔ | ✔ | Suchleiste (M-13.4) |
| **Ersetzen** in der Notiz (einzeln / alle) | ✔ | ✔ | ✔ | ✘ | häufigste Lücke |
| Suchen mit Gross/Klein, ganzes Wort, regulärem Ausdruck | ◐ | ✔ | ✔ | ✘ | |
| Suchen und Ersetzen im ganzen Workspace | ✘ | ✔ | ✘ | ◐ | Suchen ja (M-13.1), Ersetzen nein |
| Mehrere Cursor / Spaltenauswahl | ✔ | ✔ | ✘ | ◐ | nur Vim-Blockauswahl in der Quellansicht |
| Rechtschreibprüfung | ✔ | ◐ | ✔ | ✘ | |
| Wörter, Zeichen, Lesezeit | ✔ | ◐ | ✔ | ◐ | Grösse und Zeilen in den Properties |
| Zeile verschieben / duplizieren (Alt+↑/↓, Ctrl+Shift+D) | ✔ | ✔ | ✔ | ◐ | nur mit Vim |
| Automatisches Schliessen von Klammern und Markup (`**`, `` ` ``) | ✔ | ✔ | ✔ | ✘ | |
| Listen fortsetzen, Ein-/Ausrücken mit Tab | ✔ | ◐ | ◐ | ◐ | Enter setzt Listen fort (M-09.11), Tab rückt nicht ein |
| Tabelle bearbeiten (Spalte/Zeile einfügen, ausrichten) | ✔ | ◐ | ✘ | ◐ | Tabelle einfügen ja, bearbeiten im Markup |
| Vorlagen (Templates) für neue Notizen | ✔ | ◐ (Snippets) | ✘ | ✘ | |
| Tagesnotiz (Daily note) | ✔ | ✘ | ✘ | ✘ | |
| Einfügen von Datum/Uhrzeit | ✔ | ◐ | ✘ | ✘ | |
| Tastenkürzel selbst belegen | ✔ | ✔ | ◐ | ✘ | |

## Ansicht und Arbeitsfluss

| Funktion | Obsidian | VS Code | GNOME TE | Underline | Bemerkung |
|---|---|---|---|---|---|
| Geteilte Ansicht: zwei Notizen oder Quelle und Facade nebeneinander | ✔ | ✔ | ✘ | ✘ | |
| Zoom der Schrift (Ctrl+/Ctrl−/Ctrl+0) | ✔ | ✔ | ✔ | ✘ | Bilder und Diagramme lassen sich vergrössern |
| Konzentrationsmodus (Zen, Seitenleisten ausblenden) | ◐ | ✔ | ✔ | ◐ | Panels lassen sich verkleinern, nicht ausblenden per Taste |
| Aktuelle Zeile hervorheben, passende Klammer zeigen | ◐ | ✔ | ✔ | ✘ | |
| Zeilenumbruch umschalten | ✔ | ✔ | ✔ | ✘ | immer Umbruch |
| Lokale Versionsgeschichte / Dateiwiederherstellung | ✔ | ✔ | ◐ | ◐ | Git-Diff und Papierkorb (30 Tage), keine Schnappschüsse ohne Git |
| Wiederherstellen nach Absturz | ✔ | ✔ | ✔ | ✔ | Autosave (NFR-005) |
| Drucken | ✔ | ✘ | ✔ | ◐ | über PDF-Export |
| Mehrere Fenster | ✔ | ✔ | ✔ | ✘ | |

## Darstellung (Markdown/AsciiDoc)

| Funktion | Obsidian | VS Code | GNOME TE | Underline | Bemerkung |
|---|---|---|---|---|---|
| Mathematik (LaTeX `$…$`, `$$…$$`) | ✔ | ✔ (Vorschau) | ✘ | ✘ | GitHub rendert `$…$` (C-003) |
| Callouts / Alerts (`> [!NOTE]`) | ✔ | ◐ | ✘ | ◐ | GitHub-Alerts: zu prüfen; AsciiDoc-Admonitions ja |
| Fussnoten | ✔ | ✔ | ✘ | ◐ | zu prüfen |
| Hervorhebung `==text==` | ✔ | ✘ | ✘ | – | GitHub kennt es nicht (C-003) |
| Einbetten einer Notiz in eine andere | ✔ | ✘ | ✘ | ✘ | kein GitHub-Standard |
| Mermaid, PlantUML, Code-Farben | ✔ (Mermaid) | ◐ | ◐ | ✔ | Underline stärker |
| Canvas / Whiteboard | ✔ | ✘ | ✘ | – | |

## Bereits stärker als die Vergleichs-Editoren

Facade für Markdown und AsciiDoc gleichermassen, PlantUML offline, Git-Workflow mit Konfliktlösung, Agenten über
MCP mit sichtbarem Cursor, Todos mit Notiz-Verknüpfung, Export nach AsciiDoc/HTML/PDF, Vim und Nano.

## Passt nicht zum Produkt (Vision „Out of scope“ oder Einschränkungen)

Plugin-System und Erweiterungsmarktplatz, eingebaute KI, eigener Cloud-Sync, Echtzeit-Zusammenarbeit, Canvas,
proprietäre Syntax, die GitHub nicht darstellt (`==highlight==`, Obsidian-Embeds), integriertes Terminal.

## Vorschlag zur Reihenfolge

1. **Suchen und Ersetzen** (Notiz und Workspace, Gross/Klein, ganzes Wort, regulärer Ausdruck) — grösste Lücke,
   Standard in allen drei Editoren.
2. **Notiz schnell öffnen und Befehlspalette** — Tastatur-Arbeitsfluss (Zielgruppe „keyboard-friendly“).
3. **Backlinks und ausgehende Links im Werkzeugpanel**, Link-Vervollständigung beim Tippen — der Kern einer
   vernetzten Notiz-App, mit normalen Markdown-/AsciiDoc-Links (bleibt GitHub-kompatibel).
4. **Editier-Komfort**: Klammern/Markup automatisch schliessen, Zeilen verschieben/duplizieren, Tab rückt Listen
   ein, aktuelle Zeile und Klammerpaar hervorheben, Gehe zu Zeile.
5. **Zoom und Konzentrationsmodus**, Zeilenumbruch umschalten.
6. **Wortzahl und Statistik** in den Properties.
7. **Tags und Frontmatter-Eigenschaften**, Tagesnotiz und Vorlagen.
8. **Geteilte Ansicht**, Mathematik, Rechtschreibprüfung, lokale Versionsgeschichte, mehrere Fenster.
