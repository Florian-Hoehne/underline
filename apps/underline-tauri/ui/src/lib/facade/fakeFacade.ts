// The facade of the mock backend (plan section 10): a rough, line-based
// stand-in for the facade engine's blocks, so the facade can be tried in a
// plain browser. Pictures are inline SVG (`data:`), which only the mock
// gives; the backend serves them through `ul-asset` (ADR-023). The real
// rules are tested in Rust (`underline-session`, `facade/tests.rs`).

import type { BlockKindView, BlockView, FigureView, RunView } from '../bindings';

/** The note of the mock that shows every kind of block. */
export const TOUR_NOTE = 'Facade-Tour';

/** The text of the tour note. */
export const TOUR_TEXT = `# Facade tour

Some **bold**, *italic*, ~~gone~~ and \`code\`, with [a link](https://example.com) 🙂.

## Lists

- one
- [x] done
  1. inner

> A quote

\`\`\`rust
fn main() {
    println!("hi");
}
\`\`\`

| Name | Size |
|:-----|-----:|
| **a** | 1 |
| b | 22 |

![The plan](img/plan.svg)

![Gone](img/gone.png)

\`\`\`mermaid
graph TD
A-->B
\`\`\`

---

The end.
`;

function svg(body: string, width: number, height: number): string {
  const doc = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">${body}</svg>`;
  return `data:image/svg+xml,${encodeURIComponent(doc)}`;
}

const PLAN = svg(
  '<rect width="240" height="120" fill="#cde"/><circle cx="60" cy="60" r="36" fill="#58a"/><rect x="120" y="30" width="90" height="60" fill="#e96"/>',
  240,
  120,
);
const DIAGRAM = svg(
  '<rect x="70" y="10" width="60" height="30" rx="4" fill="#fff" stroke="#333"/><text x="100" y="30" font-size="14" text-anchor="middle">A</text><line x1="100" y1="40" x2="100" y2="80" stroke="#333"/><rect x="70" y="80" width="60" height="30" rx="4" fill="#fff" stroke="#333"/><text x="100" y="100" font-size="14" text-anchor="middle">B</text>',
  200,
  120,
);

const INLINE = /\*\*(.+?)\*\*|\*(.+?)\*|~~(.+?)~~|`([^`]+)`|\[([^\]]+)\]\(([^)]+)\)/g;

/** The runs of `text`, which stands at `at` in its block. */
function inlineRuns(text: string, at: number): RunView[] {
  const runs: RunView[] = [];
  const plain = (from: number, to: number) => {
    if (to > from) runs.push(run(text.slice(from, to), at + from));
  };
  let last = 0;
  for (const match of text.matchAll(INLINE)) {
    const index = match.index ?? 0;
    plain(last, index);
    const [whole, bold, italic, strike, code, label, link] = match;
    const inner = bold ?? italic ?? strike ?? code ?? label ?? '';
    const offset = index + whole.indexOf(inner);
    runs.push({
      ...run(inner, at + offset),
      bold: bold !== undefined,
      italic: italic !== undefined,
      strike: strike !== undefined,
      code: code !== undefined,
      link: link ?? null,
    });
    last = index + whole.length;
  }
  plain(last, text.length);
  return runs;
}

/** A plain run of `text` standing at `at`, one to one. */
function run(text: string, at: number): RunView {
  return {
    text,
    bold: false,
    italic: false,
    strike: false,
    code: false,
    link: null,
    offsets: text.length > 0 ? [[0, at]] : [],
    end: at + text.length,
  };
}

function block(
  kind: BlockKindView,
  start: number,
  line: number,
  markup: string,
  runs: RunView[],
  prefix = '',
  figure: FigureView | null = null,
): BlockView {
  return {
    kind,
    start,
    line,
    length: markup.length + 1,
    lines: markup.split('\n').length,
    prefix,
    runs,
    colors: null,
    figure,
  };
}

function figure(fields: Partial<FigureView>): FigureView {
  return {
    picture: null,
    message: null,
    failed: false,
    sourceShown: false,
    loading: null,
    unreadable: null,
    caption: null,
    ...fields,
  };
}

/** The blocks of `text`, roughly as the facade engine makes them. */
export function fakeBlocks(text: string): BlockView[] {
  const lines = text.split('\n');
  const starts: number[] = [];
  let offset = 0;
  for (const line of lines) {
    starts.push(offset);
    offset += line.length + 1;
  }
  const blocks: BlockView[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    const start = starts[i];
    if (line.trim() === '') {
      i++;
      continue;
    }
    const heading = /^(#{1,6}) (.*)$/.exec(line);
    const item = /^( *)(- \[( |x)\] |- |\d+\. )(.*)$/.exec(line);
    const image = /^!\[([^\]]*)\]\(([^)]+)\)$/.exec(line);
    if (heading) {
      const level = heading[1].length;
      blocks.push(
        block({ type: 'heading', level }, start, i, line, inlineRuns(heading[2], level + 1)),
      );
      i++;
    } else if (line.startsWith('```')) {
      let end = i + 1;
      while (end < lines.length && !lines[end].startsWith('```')) end++;
      const language = line.slice(3).trim();
      const code = lines.slice(i + 1, end).join('\n');
      const markup = lines.slice(i, end + 1).join('\n');
      const diagram = ['mermaid', 'plantuml', 'puml'].includes(language)
        ? figure({ picture: DIAGRAM })
        : null;
      blocks.push(
        block(
          { type: 'code', language },
          start,
          i,
          markup,
          [run(code, line.length + 1)],
          '',
          diagram,
        ),
      );
      i = end + 1;
    } else if (line.startsWith('|')) {
      let end = i;
      while (end < lines.length && lines[end].startsWith('|')) end++;
      const rows = lines
        .slice(i, end)
        .filter((row) => !/^\|[\s:|-]+\|$/.test(row))
        .map((row) =>
          row
            .slice(1, -1)
            .split('|')
            .map((cell) => inlineRuns(cell.trim(), 0)),
        );
      const markup = lines.slice(i, end).join('\n');
      blocks.push(
        block(
          {
            type: 'table',
            table: { header: true, align: ['left', 'right'], rows, caption: null },
          },
          start,
          i,
          markup,
          [run(markup, 0)],
        ),
      );
      i = end;
    } else if (image) {
      const [, alt, target] = image;
      const known = target === 'img/plan.svg';
      blocks.push(
        block({ type: 'image', target, alt }, start, i, line, [run(line, 0)], '', {
          ...figure(
            known
              ? { picture: PLAN, loading: `Loading ${target}…` }
              : {
                  message: `The image ${alt} cannot be shown: ${target} was not found`,
                  failed: true,
                },
          ),
        }),
      );
      i++;
    } else if (/^-{3,}$/.test(line)) {
      blocks.push(block({ type: 'thematicBreak' }, start, i, line, []));
      i++;
    } else if (item) {
      const [, indent, marker, check, rest] = item;
      const depth = Math.floor(indent.length / 2);
      const bullet = marker.startsWith('-') ? '•  ' : `${marker.trim()}  `;
      const box = check === undefined ? '' : check === 'x' ? '☑ ' : '☐ ';
      blocks.push(
        block(
          { type: 'listItem', depth },
          start,
          i,
          line,
          inlineRuns(rest, indent.length + marker.length),
          `${'    '.repeat(depth)}${bullet}${box}`,
        ),
      );
      i++;
    } else if (line.startsWith('> ')) {
      blocks.push(
        block({ type: 'quote', depth: 1 }, start, i, line, inlineRuns(line.slice(2), 2), '    '),
      );
      i++;
    } else {
      let end = i;
      while (end < lines.length && lines[end].trim() !== '' && !lines[end].startsWith('#')) end++;
      const markup = lines.slice(i, end).join('\n');
      // A line break shows as a space, as in the facade.
      blocks.push(
        block({ type: 'paragraph' }, start, i, markup, inlineRuns(markup.replace(/\n/g, ' '), 0)),
      );
      i = end;
    }
  }
  return blocks;
}
