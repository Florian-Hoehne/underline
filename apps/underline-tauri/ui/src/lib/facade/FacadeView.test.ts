import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../../App.svelte';
import { connectedShell } from '../../test/shell';
import type { BlockView, EditorPatch, FacadePatch, FacadeState } from '../bindings';
import { EditorInfo } from '../editor/info.svelte';
import type { CoreListener } from '../editor/sync';
import { activeFacade } from './active';
import type { FacadeBackend } from './backend';
import { assetUrl } from './backend';
import { fakeBlocks, TOUR_NOTE, TOUR_TEXT } from './fakeFacade';
import FacadeView from './FacadeView.svelte';
import { sourceAt } from './model';

/** Images load at once (jsdom loads none). */
class LoadingImage {
  /** As WebKit reports a Mermaid SVG with `width="100%"` and a view box. */
  naturalWidth = 168;
  naturalHeight = 152;
  onload: (() => void) | null = null;
  onerror: (() => void) | null = null;
  set src(url: string) {
    setTimeout(() => (url.includes('broken') ? this.onerror?.() : this.onload?.()), 0);
  }
}

beforeEach(() => {
  vi.stubGlobal('Image', LoadingImage);
});

afterEach(() => {
  cleanup();
  clearMocks();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const PATCH: EditorPatch = {
  base: 1,
  rev: 1,
  changes: [],
  anchor: 0,
  head: 0,
  scroll: false,
  unstored: false,
  place: 'Ln 1, Col 1',
  keys: null,
  commands: [],
  request: null,
};

/** More blocks of the kinds the tour note lacks. */
function extraBlocks(at: number, line: number): BlockView[] {
  const base = {
    lines: 1,
    colors: null,
    figure: null,
  };
  const run = (text: string) => ({
    text,
    bold: false,
    italic: false,
    strike: false,
    code: false,
    link: null,
    offsets: [[0, 0]] as [number, number][],
    end: text.length,
  });
  return [
    {
      ...base,
      kind: { type: 'admonition', label: 'NOTE' },
      start: at,
      line,
      length: 18,
      prefix: '    NOTE  ',
      runs: [run('Mind this.')],
    },
    {
      ...base,
      kind: { type: 'markup' },
      start: at + 20,
      line: line + 2,
      length: 12,
      prefix: '',
      runs: [run('<div>x</div>')],
    },
    {
      ...base,
      kind: { type: 'code', language: 'plantuml' },
      start: at + 34,
      line: line + 4,
      length: 20,
      prefix: '',
      runs: [run('@startuml\nboom')],
      figure: {
        picture: null,
        message: 'This PlantUML diagram cannot be shown: Line 2: Syntax Error?',
        failed: true,
        sourceShown: true,
        loading: null,
        unreadable: null,
        caption: null,
      },
    },
    {
      ...base,
      kind: { type: 'code', language: 'rust' },
      start: at + 56,
      line: line + 8,
      length: 16,
      prefix: '',
      runs: [run('fn main() {}')],
      colors: {
        spans: [[0, 2, 0]],
        styles: [{ color: '#aa0000', bold: false, italic: false }],
      },
    },
  ];
}

function state(blocks: BlockView[], head = 0): FacadeState {
  return {
    note: 'Home',
    rev: 1,
    version: 1,
    format: 'Markdown',
    blocks,
    anchor: head,
    head,
    place: 'Ln 1, Col 1',
    gap: null,
    keys: null,
  };
}

/** A facade patch moving nothing, of revision 1. */
const FACADE_PATCH: FacadePatch = {
  base: 1,
  rev: 1,
  blocks: [],
  anchor: 0,
  head: 0,
  gap: null,
  scroll: false,
  unstored: false,
  place: 'Ln 1, Col 1',
  keys: null,
  commands: [],
  request: null,
  copied: null,
  wantsClipboard: false,
};

function backendWith(opened: FacadeState): FacadeBackend & {
  open: ReturnType<typeof vi.fn>;
  select: ReturnType<typeof vi.fn>;
  fold: ReturnType<typeof vi.fn>;
  find: ReturnType<typeof vi.fn>;
  input: ReturnType<typeof vi.fn>;
  clipboard: ReturnType<typeof vi.fn>;
  pasteImage: ReturnType<typeof vi.fn>;
  insert: ReturnType<typeof vi.fn>;
  copySelection: ReturnType<typeof vi.fn>;
  keys: ReturnType<typeof vi.fn>;
  readClipboard: ReturnType<typeof vi.fn>;
  writeClipboard: ReturnType<typeof vi.fn>;
  refresh: ReturnType<typeof vi.fn>;
} {
  return {
    open: vi.fn(async () => opened),
    select: vi.fn(async () => 'Ln 3, Col 2'),
    fold: vi.fn(async () => PATCH),
    find: vi.fn(async () => null),
    input: vi.fn(async () => FACADE_PATCH),
    clipboard: vi.fn(async () => ({ patch: FACADE_PATCH, image: null })),
    pasteImage: vi.fn(async () => FACADE_PATCH),
    insert: vi.fn(async () => FACADE_PATCH),
    copySelection: vi.fn(async () => {}),
    textHash: () => null,
    keys: vi.fn(async () => null),
    readClipboard: vi.fn(async () => 'from the clipboard'),
    writeClipboard: vi.fn(async () => {}),
    // ADR-025: the window's editor core, which the test speaks for (`core()`).
    attach: (_note: string, listener: CoreListener) => {
      attached = listener;
      return () => {
        if (attached === listener) attached = null;
      };
    },
    refresh: vi.fn(async () => FACADE_PATCH),
    setMode: () => {},
    leave: () => {},
  };
}

/** The editor core's side of the facade shown (ADR-025). */
let attached: CoreListener | null = null;

function core(): CoreListener {
  if (!attached) throw new Error('No facade is attached');
  return attached;
}

async function show(opened: FacadeState, info = new EditorInfo()) {
  const backend = backendWith(opened);
  const errors: string[] = [];
  render(FacadeView, {
    props: {
      note: 'Home',
      label: 'Home',
      platform: 'linux',
      info,
      backend,
      onError: (message: string) => errors.push(message),
    },
  });
  await waitFor(() => expect(document.querySelector('[data-b="0"]')).not.toBeNull());
  return { backend, errors, info };
}

const tour = () => {
  const blocks = fakeBlocks(TOUR_TEXT);
  const last = blocks[blocks.length - 1];
  return [...blocks, ...extraBlocks(last.start + last.length + 2, last.line + 2)];
};

const block = (index: number) => document.querySelector<HTMLElement>(`[data-b="${index}"]`)!;

describe('M-09.1, M-09.2 the facade draws every kind of block', () => {
  it('headings, text with its styles, list markers, quotes, code and breaks', async () => {
    const { info } = await show(state(tour()));

    expect(screen.getByRole('heading', { level: 1, name: 'Facade tour' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { level: 2, name: 'Lists' })).toBeInTheDocument();
    const paragraph = block(1);
    expect(within(paragraph).getByText('bold')).toHaveClass('bold');
    expect(within(paragraph).getByText('italic')).toHaveClass('italic');
    expect(within(paragraph).getByText('gone')).toHaveClass('strike');
    expect(within(paragraph).getByText('code')).toHaveClass('code');
    const link = within(paragraph).getByText('a link');
    expect(link).toHaveClass('link');
    expect(link).toHaveAttribute('data-link', 'https://example.com');
    expect(block(3).querySelector('.prefix')).toHaveTextContent('•');
    expect(block(4).querySelector('.prefix')?.textContent).toBe('•  ☑ ');
    expect(block(5).querySelector('.prefix')?.textContent).toBe('    1.  ');
    expect(block(6).querySelectorAll('.bar')).toHaveLength(1);
    expect(block(7).querySelector('.code')).toHaveTextContent('println!("hi");');
    expect(document.querySelector('[role="separator"]')).not.toBeNull();
    // M-09.3: the format indicator.
    expect(screen.getByText('Markdown')).toHaveClass('indicator');
    expect(info.place).toBe('Ln 1, Col 1');
  });

  it('admonitions, markup, a failed diagram with its source and coloured code', async () => {
    const blocks = extraBlocks(0, 0);
    await show(state(blocks));
    const at = (type: string) => blocks.findIndex((b) => b.kind.type === type);

    const admonition = block(at('admonition'));
    expect(admonition.querySelector('.prefix')?.textContent).toBe('    NOTE  ');
    expect(admonition.querySelector('.admonition')).not.toBeNull();
    expect(block(at('markup')).querySelector('.code')).toHaveTextContent('<div>x</div>');
    const failed = blocks.findIndex((b) => b.figure?.failed && b.kind.type === 'code');
    await waitFor(() =>
      expect(
        within(block(failed)).getByText(
          'This PlantUML diagram cannot be shown: Line 2: Syntax Error?',
        ),
      ).toHaveClass('failure'),
    );
    expect(block(failed).querySelector('.frame')).toHaveTextContent('@startuml');
    const rust = blocks.length - 1;
    expect(within(block(rust)).getByText('fn')).toHaveStyle({ color: '#aa0000' });
  });
});

describe('M-09.5 to M-09.7 figures', () => {
  it('a table with its header and alignment; its markup only with the cursor in it', async () => {
    const blocks = tour();
    const index = blocks.findIndex((b) => b.kind.type === 'table');
    const { backend } = await show(state(blocks));
    const table = within(block(index)).getByRole('table');

    expect(
      within(table)
        .getAllByRole('columnheader')
        .map((c) => c.textContent),
    ).toEqual(['Name', 'Size']);
    expect(within(table).getAllByRole('cell')[1]).toHaveStyle({ textAlign: 'right' });
    expect(within(table).getByText('a')).toHaveClass('bold');
    expect(block(index).querySelector('.frame')).toBeNull();

    // M-09.8: a click on the table puts the cursor into the block.
    await fireEvent.click(table);

    await waitFor(() => expect(block(index).querySelector('.frame')).not.toBeNull());
    const start = sourceAt(blocks[index], blocks[index].runs[0], 0);
    expect(backend.select).toHaveBeenCalledWith('Home', 1, start, start);
    expect(block(index).querySelector('.frame')).toHaveTextContent('| Name | Size |');
  });

  it('M-09.8 the figure holding the cursor shows its markup', async () => {
    const blocks = tour();
    const index = blocks.findIndex((b) => b.kind.type === 'image');
    await show(state(blocks, blocks[index].start + 2));

    expect(block(index).querySelector('.frame')).toHaveTextContent('![The plan](img/plan.svg)');
  });

  it('images and diagrams come through the own protocol; a missing image says so', async () => {
    const blocks = fakeBlocks(TOUR_TEXT);
    blocks[blocks.findIndex((b) => b.kind.type === 'image')].figure!.picture =
      'file/img/plan.svg?v=1';
    await show(state(blocks));
    const images = () => [...document.querySelectorAll<HTMLImageElement>('.picture img')];

    await waitFor(() => expect(images()).toHaveLength(2));
    expect(images()[0].getAttribute('src')).toBe('ul-asset://localhost/file/img/plan.svg?v=1');
    expect(images()[1].getAttribute('src')).toMatch(/^data:image\/svg\+xml,/);
    expect(
      screen.getByText('The image Gone cannot be shown: img/gone.png was not found'),
    ).toHaveClass('failure');
    expect(assetUrl('diagram/ab.svg', 'windows')).toBe('http://ul-asset.localhost/diagram/ab.svg');
  });

  it('MUSS 1: a picture is laid out at its own size, with its alt text', async () => {
    await show(state(fakeBlocks(TOUR_TEXT)));

    const images = await waitFor(() => {
      const found = [...document.querySelectorAll<HTMLImageElement>('.picture img')];
      expect(found).toHaveLength(2);
      return found;
    });
    expect(images[0].style.width).toBe('168px');
    expect(images[0].style.aspectRatio).toBe('168 / 152');
    expect(images[0]).toHaveAttribute('alt', 'The plan');
    expect(images[1]).toHaveAttribute('alt', 'Mermaid diagram');
    expect(screen.getAllByRole('button', { name: 'Enlarge picture' })[0]).toHaveAttribute(
      'title',
      'Enlarge (double click)',
    );
  });

  it('M-09.9, NFR-011 Ctrl+Enter enlarges the picture the cursor is in', async () => {
    const blocks = fakeBlocks(TOUR_TEXT);
    const index = blocks.findIndex((b) => b.kind.type === 'image');
    await show(state(blocks, blocks[index].start + 1));
    await waitFor(() => expect(document.querySelector('.picture img')).not.toBeNull());

    await fireEvent.keyDown(document.querySelector('.facade-text')!, {
      key: 'Enter',
      ctrlKey: true,
    });

    expect(screen.getByRole('dialog', { name: 'Enlarged picture' })).toBeInTheDocument();
  });

  it('a picture the window cannot read says so; it says "Loading" before', async () => {
    vi.stubGlobal(
      'Image',
      class {
        onerror: (() => void) | null = null;
        set src(_url: string) {
          setTimeout(() => this.onerror?.(), 20);
        }
      },
    );
    const blocks = fakeBlocks('![The plan](img/plan.png)\n');
    blocks[0].figure = {
      ...blocks[0].figure!,
      picture: 'file/img/plan.png?v=1',
      loading: 'Loading img/plan.png…',
      unreadable: 'The image The plan cannot be shown: the image cannot be read',
    };
    await show(state(blocks));

    expect(screen.getByText('Loading img/plan.png…')).not.toHaveClass('failure');
    await waitFor(() =>
      expect(
        screen.getByText('The image The plan cannot be shown: the image cannot be read'),
      ).toHaveClass('failure'),
    );
  });

  it('M-09.9 the magnifier or a double click enlarges a picture; Escape closes it', async () => {
    await show(state(fakeBlocks(TOUR_TEXT)));
    const magnifier = await waitFor(
      () => screen.getAllByRole('button', { name: 'Enlarge picture' })[0],
    );

    await fireEvent.click(magnifier);

    const dialog = screen.getByRole('dialog', { name: 'Enlarged picture' });
    expect(dialog.querySelector('.hint')?.textContent).toMatch(
      /^\d+ % {3}· {3}wheel or \+\/− to zoom, drag to move, 0 to fit, Esc to close$/,
    );
    await fireEvent.keyDown(dialog, { key: 'Escape' });
    expect(screen.queryByRole('dialog')).toBeNull();

    await fireEvent.dblClick(document.querySelector('.picture img')!);
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});

describe('M-07.4, M-08.8 links and folding', () => {
  it('pointing at a link shows its address and "Ctrl+click to follow"', async () => {
    await show(state(fakeBlocks(TOUR_TEXT)));
    const link = screen.getByText('a link');

    await fireEvent.pointerMove(link, { clientX: 10, clientY: 10 });

    expect(screen.getByRole('tooltip').textContent).toBe(
      'https://example.com\nCtrl+click to follow',
    );
    await fireEvent.pointerLeave(document.querySelector('.facade-text')!);
    expect(screen.queryByRole('tooltip')).toBeNull();
  });

  it('a heading with a section has its arrow; a folded one hides its body and ends on "…"', async () => {
    const blocks = fakeBlocks('# A\n\ntext\n\n## B\n\nmore\n');
    const { backend, info } = await show(state(blocks));

    core().views({
      sections: {
        note: 'Home',
        rev: 1,
        sections: [
          { line: 0, last: 6, folded: false },
          { line: 4, last: 6, folded: true },
        ],
      },
      found: null,
    });
    await tick();

    expect(block(0).querySelector('.fold')).not.toBeNull();
    expect(block(3)).toBeNull();
    expect(screen.getByRole('img', { name: 'folded' })).toHaveTextContent('…');
    backend.fold.mockResolvedValueOnce({ ...PATCH, head: 7, anchor: 7, place: 'Ln 5, Col 1' });
    await fireEvent.mouseDown(block(2).querySelector('.fold')!);
    expect(backend.fold).toHaveBeenCalledWith('Home', 4);
    // MUSS 2: the cursor and the status line follow the backend.
    await waitFor(() => expect(info.place).toBe('Ln 5, Col 1'));
  });
});

const many = (count: number) =>
  fakeBlocks(Array.from({ length: count }, (_, i) => `Paragraph ${i}`).join('\n\n') + '\n');

describe('ADR-024 the facade is virtualized', () => {
  it('only the blocks near the part in view are in the DOM', async () => {
    await show(state(many(2000)));

    const drawn = document.querySelectorAll('[data-b]').length;
    expect(drawn).toBeGreaterThan(5);
    expect(drawn).toBeLessThan(200);
    const spaces = document.querySelectorAll<HTMLElement>('.space');
    expect(parseFloat(spaces[1].style.height)).toBeGreaterThan(10_000);
  });

  it('A-25, UX 13: the place shown comes back when the facade is shown again', async () => {
    const blocks = many(2000);
    await show(state(blocks));
    const scroller = () => document.querySelector<HTMLElement>('.scroller')!;
    scroller().scrollTop = 20_000;
    scroller().dispatchEvent(new Event('scroll'));
    cleanup();

    await show(state(blocks));

    await waitFor(() => expect(Math.abs(scroller().scrollTop - 20_000)).toBeLessThan(2));
  });

  it('M-09.16 reveal scrolls a block to the top without moving the cursor', async () => {
    const blocks = many(2000);
    const { backend } = await show(state(blocks));
    const scroller = document.querySelector<HTMLElement>('.scroller')!;

    activeFacade()!.reveal(blocks[1500].start + 1);

    expect(scroller.scrollTop).toBeGreaterThan(10_000);
    expect(backend.select).not.toHaveBeenCalled();
  });

  it('a change of blocks replaces them and moves the rest; another version asks again', async () => {
    const blocks = many(3);
    const { backend } = await show(state(blocks));
    const changed = fakeBlocks('Paragraph 1 and more\n')[0];

    core().blocks!([
      {
        note: 'Home',
        rev: 2,
        base: 1,
        version: 2,
        from: 1,
        removed: 1,
        blocks: [{ ...changed, start: blocks[1].start, line: 2 }],
        shift: 9,
        lineShift: 0,
      },
    ]);
    await tick();

    expect(block(1)).toHaveTextContent('Paragraph 1 and more');
    expect(block(2)).toHaveTextContent('Paragraph 2');
    core().blocks!([
      {
        note: 'Home',
        rev: 3,
        base: 7,
        version: 8,
        from: 0,
        removed: 0,
        blocks: [],
        shift: 0,
        lineShift: 0,
      },
    ]);
    await waitFor(() => expect(backend.open).toHaveBeenCalledTimes(2));
  });
});

describe('F5b editing in the facade (ADR-024, UC-005)', () => {
  const text = () => document.querySelector<HTMLElement>('.facade-text')!;
  const beforeInput = (inputType: string, data: string | null = null) => {
    const event = new InputEvent('beforeinput', {
      inputType,
      data,
      bubbles: true,
      cancelable: true,
    });
    text().dispatchEvent(event);
    return event;
  };
  const keyDown = (key: string, init: KeyboardEventInit = {}) => {
    const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init });
    text().dispatchEvent(event);
    return event;
  };

  it('M-09.10 typing goes to the backend with the selection; its blocks and caret come back', async () => {
    const blocks = fakeBlocks('Hello\n\nWorld\n');
    const { backend, info } = await show(state(blocks, 5));
    const typed = fakeBlocks('Hello!\n\nWorld\n');
    backend.input.mockResolvedValueOnce({
      ...FACADE_PATCH,
      base: 1,
      rev: 2,
      anchor: 6,
      head: 6,
      place: 'Ln 1, Col 7',
      blocks: [
        {
          note: 'Home',
          rev: 2,
          base: 1,
          version: 2,
          from: 0,
          removed: 1,
          blocks: [typed[0]],
          shift: 1,
          lineShift: 0,
        },
      ],
    });

    const event = beforeInput('insertText', '!');

    expect(event.defaultPrevented).toBe(true);
    await waitFor(() => expect(block(0)).toHaveTextContent('Hello!'));
    expect(backend.input).toHaveBeenCalledWith(
      'Home',
      { rev: 1, anchor: 5, head: 5 },
      { kind: 'text', text: '!' },
      null,
      null,
    );
    expect(info.place).toBe('Ln 1, Col 7');
    expect(block(1)).toHaveTextContent('World');
    // The facade is no longer read only (A-40 resolved).
    expect(text()).not.toHaveAttribute('aria-readonly');
    expect(text()).toHaveAttribute('spellcheck', 'false');
  });

  it('M-09.11, M-09.12, M-10.1 Enter, Backspace, Delete, undo and redo of the webview', async () => {
    const { backend } = await show(state(fakeBlocks(TOUR_TEXT)));
    const key = (name: string, ctrl = false) => ({
      kind: 'key',
      key: name,
      ctrl,
      alt: false,
      shift: false,
      meta: false,
    });

    beforeInput('insertParagraph');
    beforeInput('deleteContentBackward');
    beforeInput('deleteContentForward');
    beforeInput('historyUndo');
    beforeInput('historyRedo');
    const bold = beforeInput('formatBold');

    await waitFor(() => expect(backend.input).toHaveBeenCalledTimes(5));
    const sent = backend.input.mock.calls.map((call) => call[2]);
    expect(sent).toEqual([
      key('Enter'),
      key('Backspace'),
      key('Delete'),
      key('Z', true),
      key('Y', true),
    ]);
    // Anything else changes nothing.
    expect(bold.defaultPrevented).toBe(true);
  });

  it('M-09.13 Tab and Page Up do nothing; Escape leaves the text in the Normal mode', async () => {
    const { backend } = await show(state(fakeBlocks(TOUR_TEXT)));
    const tab = document.createElement('button');
    tab.id = 'tab-Home';
    document.body.append(tab);

    expect(keyDown('Tab').defaultPrevented).toBe(true);
    expect(keyDown('PageUp').defaultPrevented).toBe(true);
    keyDown('Escape');

    expect(backend.input).not.toHaveBeenCalled();
    tab.remove();
  });

  it('M-09.13 arrows go to the backend; Up and Down ask the layout first', async () => {
    const { backend } = await show(state(fakeBlocks(TOUR_TEXT)));

    keyDown('ArrowRight');
    keyDown('ArrowDown', { shiftKey: true });

    await waitFor(() => expect(backend.input).toHaveBeenCalledTimes(2));
    expect(backend.input.mock.calls[0][2]).toMatchObject({ key: 'ArrowRight' });
    expect(backend.input.mock.calls[1][2]).toMatchObject({ key: 'ArrowDown', shift: true });
  });

  it('M-09.11 the empty lines after Enter are drawn before their block', async () => {
    const blocks = fakeBlocks('First.\n\nSecond.\n');
    const { backend } = await show(state(blocks, 6));
    backend.input.mockResolvedValueOnce({
      ...FACADE_PATCH,
      rev: 1,
      anchor: 8,
      head: 8,
      gap: { before: 1, lines: 2 },
    });

    keyDown('Enter');

    await waitFor(() => expect(document.querySelectorAll('[data-gap] .gap-line')).toHaveLength(2));
    const gap = document.querySelector('[data-gap]')!;
    expect(gap.nextElementSibling?.querySelector('[data-b="1"]')).not.toBeNull();
  });

  it('M-08.5 an input method composes in the text; its finished text is sent at the end', async () => {
    const { backend } = await show(state(fakeBlocks(TOUR_TEXT)));

    text().dispatchEvent(new CompositionEvent('compositionstart', { data: '' }));
    const composing = beforeInput('insertCompositionText', '日');
    text().dispatchEvent(new CompositionEvent('compositionend', { data: '日本' }));

    expect(composing.defaultPrevented).toBe(false);
    await waitFor(() => expect(backend.input).toHaveBeenCalledTimes(1));
    expect(backend.input.mock.calls[0][2]).toEqual({ kind: 'text', text: '日本' });
  });

  it('M-09.14 copy, cut and paste go through the backend; Ctrl+Shift+V pastes plain text', async () => {
    const { backend } = await show(state(fakeBlocks(TOUR_TEXT)));

    const paste = new Event('paste', { bubbles: true, cancelable: true });
    text().dispatchEvent(paste);
    keyDown('V', { ctrlKey: true, shiftKey: true });
    text().dispatchEvent(new Event('copy', { bubbles: true, cancelable: true }));

    expect(paste.defaultPrevented).toBe(true);
    await waitFor(() => expect(backend.clipboard).toHaveBeenCalledTimes(3));
    expect(backend.clipboard.mock.calls.map((call) => [call[2], call[3]])).toEqual([
      ['paste', false],
      ['paste', true],
      ['copy', false],
    ]);
  });

  it('M-11.2, M-11.4 the keys status, the commands and a yank to the clipboard follow the patch', async () => {
    const info = new EditorInfo();
    const commands: string[] = [];
    const status = {
      keymap: 'VIM',
      mode: 'NORMAL',
      color: 0,
      pending: '',
      hints: [],
      prompting: false,
    };
    const backend = backendWith({ ...state(fakeBlocks(TOUR_TEXT)), keys: status });
    backend.input.mockResolvedValueOnce({
      ...FACADE_PATCH,
      keys: { ...status, mode: 'INSERT', color: 1 },
      commands: ['store'],
      copied: 'yanked',
      wantsClipboard: true,
    });
    render(FacadeView, {
      props: {
        note: 'Home',
        label: 'Home',
        platform: 'linux',
        mode: 'Vim',
        info,
        backend,
        onError: () => {},
        onCommand: (name: string) => commands.push(name),
      },
    });
    await waitFor(() => expect(info.keys?.mode).toBe('NORMAL'));

    beforeInput('insertText', 'i');

    await waitFor(() => expect(info.keys?.mode).toBe('INSERT'));
    expect(commands).toEqual(['store']);
    expect(backend.writeClipboard).toHaveBeenCalledWith('yanked');
    // The next input brings the clipboard's text along (`"+p`).
    beforeInput('insertText', 'p');
    await waitFor(() => expect(backend.input).toHaveBeenCalledTimes(2));
    expect(backend.input.mock.calls[1][4]).toBe('from the clipboard');
  });

  it('ADR-024 a change of blocks that comes early waits for the one before it', async () => {
    const blocks = many(3);
    const { backend } = await show(state(blocks));
    const changed = (name: string) => ({ ...fakeBlocks(`${name}\n`)[0], start: 0, line: 0 });
    const change = (base: number, name: string) => ({
      note: 'Home',
      rev: base + 1,
      base,
      version: base + 1,
      from: 0,
      removed: 1,
      blocks: [changed(name)],
      shift: 0,
      lineShift: 0,
    });

    core().blocks!([change(2, 'Second change')]);
    await tick();
    expect(block(0)).not.toHaveTextContent('Second change');
    core().blocks!([change(1, 'First change')]);
    await tick();

    expect(block(0)).toHaveTextContent('Second change');
    expect(backend.open).toHaveBeenCalledTimes(1);
  });
});

describe('M-07.2, M-09 with the mock backend', () => {
  it('the tour note shows a table, an image and a diagram in the facade', async () => {
    const shell = await connectedShell({ givenDir: '/home/user/Work' });
    render(App, { props: { shell } });
    await shell.startWorkspace();

    await shell.openNote(TOUR_NOTE);

    const text = await waitFor(() => {
      const found = document.querySelector<HTMLElement>('.facade-text');
      expect(found).not.toBeNull();
      return found!;
    });
    await waitFor(() => expect(within(text).getByRole('table')).toBeInTheDocument());
    await waitFor(() => expect(text.querySelectorAll('.picture img')).toHaveLength(2));
    expect(
      within(text).getByText('The image Gone cannot be shown: img/gone.png was not found'),
    ).toBeInTheDocument();
    expect(screen.getByRole('tabpanel', { name: 'Facade Tour' })).toHaveAttribute(
      'data-view',
      'facade',
    );
  });

  it('F5b the mock backend types in the facade', async () => {
    const shell = await connectedShell({ givenDir: '/home/user/Work' });
    render(App, { props: { shell } });
    await shell.startWorkspace();
    await shell.openNote(TOUR_NOTE);
    const text = await waitFor(() => {
      const found = document.querySelector<HTMLElement>('.facade-text');
      expect(found?.querySelector('[data-b="0"]')).toBeTruthy();
      return found!;
    });

    for (const data of ['N', 'e', 'w']) {
      text.dispatchEvent(
        new InputEvent('beforeinput', {
          inputType: 'insertText',
          data,
          bubbles: true,
          cancelable: true,
        }),
      );
    }

    await waitFor(() => expect(text.querySelector('[data-b="0"]')).toHaveTextContent('New'));
  });
});
