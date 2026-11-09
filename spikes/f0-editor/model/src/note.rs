//! The test note of the spike: 10,000 lines of Markdown, generated
//! deterministically, with the mix a long real note has.

/// The number of lines of the test note.
pub const LINES: usize = 10_000;

/// A small deterministic random source (xorshift), so every run sees the
/// same note.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

const WORDS: &[&str] = &[
    "the",
    "note",
    "editor",
    "keeps",
    "every",
    "byte",
    "of",
    "markup",
    "where",
    "it",
    "was",
    "written",
    "and",
    "only",
    "changes",
    "what",
    "you",
    "type",
    "facade",
    "block",
    "source",
    "view",
    "workspace",
    "agent",
    "tab",
    "rope",
    "undo",
    "history",
    "Über",
    "Grösse",
    "naïve",
    "café",
    "fast",
    "plain",
    "text",
    "list",
    "quote",
    "table",
    "diagram",
    "image",
    "link",
    "heading",
    "paragraph",
    "with",
    "a",
    "an",
    "to",
    "for",
    "on",
    "in",
    "is",
    "are",
    "be",
];

const LANGS: &[(&str, &[&str])] = &[
    (
        "rust",
        &[
            "fn main() {",
            "    let notes = Workspace::open(\"~/notes\")?;",
            "    for note in notes.iter() {",
            "        println!(\"{}\", note.title());",
            "    }",
            "}",
        ],
    ),
    (
        "python",
        &[
            "def count_words(text: str) -> int:",
            "    \"\"\"Counts the words of a note.\"\"\"",
            "    return len([w for w in text.split() if w])",
            "",
            "print(count_words(open('note.md').read()))",
        ],
    ),
    (
        "typescript",
        &[
            "export function debounce<T>(fn: (v: T) => void, ms: number) {",
            "  let timer: number | undefined;",
            "  return (v: T) => {",
            "    clearTimeout(timer);",
            "    timer = setTimeout(() => fn(v), ms);",
            "  };",
            "}",
        ],
    ),
    (
        "bash",
        &[
            "#!/usr/bin/env bash",
            "set -euo pipefail",
            "for f in *.md; do",
            "  wc -l \"$f\"",
            "done",
        ],
    ),
    (
        "json",
        &[
            "{",
            "  \"theme\": \"dark\",",
            "  \"autosave\": 500,",
            "  \"lineNumbers\": \"left\"",
            "}",
        ],
    ),
];

fn sentence(rng: &mut Rng) -> String {
    let len = 6 + rng.below(10);
    let mut words: Vec<String> = (0..len).map(|_| rng.pick(WORDS).to_owned()).collect();
    // Inline formatting now and then.
    match rng.below(8) {
        0 => words[1] = format!("**{}**", words[1]),
        1 => words[2] = format!("*{}*", words[2]),
        2 => words[3] = format!("`{}`", words[3]),
        3 => words[1] = format!("[{}](https://example.org/{})", words[1], words[1]),
        4 => words[2] = format!("~~{}~~", words[2]),
        _ => {}
    }
    let mut s = words.join(" ");
    if let Some(first) = s.get(..1) {
        let upper = first.to_uppercase();
        s.replace_range(..1, &upper);
    }
    s.push('.');
    s
}

fn paragraph(rng: &mut Rng, out: &mut Vec<String>) {
    // Soft-wrapped lines of 60 to 100 chars, like a hand-written note.
    let lines = 1 + rng.below(5);
    for _ in 0..lines {
        let mut line = sentence(rng);
        while line.len() < 60 {
            line.push(' ');
            line.push_str(&sentence(rng));
        }
        out.push(line);
    }
}

fn list(rng: &mut Rng, out: &mut Vec<String>) {
    let ordered = rng.below(3) == 0;
    let tasks = !ordered && rng.below(3) == 0;
    let items = 2 + rng.below(5);
    for i in 0..items {
        let marker = if ordered {
            format!("{}.", i + 1)
        } else if tasks {
            if rng.below(2) == 0 { "- [ ]" } else { "- [x]" }.to_owned()
        } else {
            "-".to_owned()
        };
        out.push(format!("{marker} {}", sentence(rng)));
        if rng.below(3) == 0 {
            for _ in 0..1 + rng.below(3) {
                out.push(format!("  - {}", sentence(rng)));
                if rng.below(4) == 0 {
                    out.push(format!("    - {}", sentence(rng)));
                }
            }
        }
    }
}

fn quote(rng: &mut Rng, out: &mut Vec<String>) {
    for _ in 0..1 + rng.below(3) {
        out.push(format!("> {}", sentence(rng)));
    }
}

fn code(rng: &mut Rng, out: &mut Vec<String>) {
    let (lang, lines) = LANGS[rng.below(LANGS.len())];
    out.push(format!("```{lang}"));
    let repeat = 1 + rng.below(3);
    for _ in 0..repeat {
        out.extend(lines.iter().map(|l| (*l).to_owned()));
    }
    out.push("```".to_owned());
}

fn table(rng: &mut Rng, out: &mut Vec<String>) {
    let cols = 3 + rng.below(2);
    let header: Vec<&str> = (0..cols).map(|_| rng.pick(WORDS)).collect();
    out.push(format!("| {} |", header.join(" | ")));
    out.push(format!("|{}", "---|".repeat(cols)));
    for _ in 0..2 + rng.below(5) {
        let row: Vec<String> = (0..cols)
            .map(|_| format!("{} {}", rng.pick(WORDS), rng.below(1000)))
            .collect();
        out.push(format!("| {} |", row.join(" | ")));
    }
}

fn figure(rng: &mut Rng, out: &mut Vec<String>) {
    match rng.below(3) {
        0 => out.push(format!(
            "![{} diagram](images/figure-{}.png)",
            rng.pick(WORDS),
            rng.below(100)
        )),
        1 => {
            out.push("```mermaid".to_owned());
            out.push("flowchart LR".to_owned());
            out.push("  A[Type] --> B{Owner}".to_owned());
            out.push("  B -->|A| C[Backend]".to_owned());
            out.push("  B -->|B| D[WASM]".to_owned());
            out.push("```".to_owned());
        }
        _ => {
            out.push("```plantuml".to_owned());
            out.push("@startuml".to_owned());
            out.push("Alice -> Bob: keystroke".to_owned());
            out.push("Bob --> Alice: patch".to_owned());
            out.push("@enduml".to_owned());
            out.push("```".to_owned());
        }
    }
}

/// The test note: exactly [`LINES`] lines, each ending with `\n`.
pub fn generate() -> String {
    let mut rng = Rng(0x00F0_5EED_1234_5678);
    let mut out: Vec<String> = vec![
        "# Spike F0 test note".to_owned(),
        String::new(),
        "This note is generated: it mixes headings, paragraphs, lists, task lists, quotes, code, \
         tables and figures."
            .to_owned(),
        String::new(),
    ];
    let mut section = 0;
    let mut misses = 0;
    // Whole sections while they fit; a few tries with smaller ones at the end.
    while misses < 200 {
        let mut part = Vec::new();
        part.push(format!(
            "## Section {}: {}",
            section + 1,
            sentence(&mut rng)
        ));
        part.push(String::new());
        let items = if misses > 0 { 1 } else { 3 + rng.below(6) };
        for _ in 0..items {
            match rng.below(12) {
                0..=4 => paragraph(&mut rng, &mut part),
                5 | 6 => list(&mut rng, &mut part),
                7 => quote(&mut rng, &mut part),
                8 => code(&mut rng, &mut part),
                9 => table(&mut rng, &mut part),
                10 => figure(&mut rng, &mut part),
                _ => part.push(format!("### {}", sentence(&mut rng))),
            }
            part.push(String::new());
        }
        if rng.below(10) == 0 {
            part.push("---".to_owned());
            part.push(String::new());
        }
        if out.len() + part.len() <= LINES {
            out.extend(part);
            section += 1;
        } else {
            misses += 1;
        }
    }
    // Fill up with paragraphs of one line each, separated by blank lines.
    while out.len() < LINES {
        if out.len() + 2 <= LINES {
            let mut line = sentence(&mut rng);
            line.push_str(" The end is near.");
            out.push(line);
        }
        out.push(String::new());
    }
    out.truncate(LINES);
    let mut text = out.join("\n");
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_note_has_ten_thousand_lines_and_never_changes() {
        let note = generate();
        assert_eq!(note.lines().count(), LINES);
        assert_eq!(note, generate());
    }
}
