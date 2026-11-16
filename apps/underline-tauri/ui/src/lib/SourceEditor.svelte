<!--
  M-08 (UC-005): the source view of one note. Its text lives in the
  window's editor core (ADR-025, editor/wasmBackend.ts), which answers every
  key and sends the edits to the backend after the frame; CodeMirror shows
  it (editor/sourceView.ts). The backend's colours (`ed-highlight`) reach it
  through the shell, which listens for the whole window; the sections and
  Nano hits come from the core. While the text loads the area stays empty;
  a failure is said in the status bar.

  M-17.6: the agent cursors in the note (`agentMarks`) show behind its
  lines (editor/agentMarks.ts).

  FR-099, UC-005 BR-023, A28: the words the spell check marks
  (`spellMarks`) are underlined with a wave (editor/spellMarks.ts); the
  context menu on one (the right mouse button, Shift+F10, the menu key)
  offers its corrections and "Add to dictionary" (SpellMenu.svelte).

  It follows the input mode and the line numbers of the settings (M-11,
  M-08.7), the editor zoom and soft wrap (UC-010 BR-009, BR-011) and tells `info` the place of the cursor and the keys' status for
  the status line (M-11.4).

  Escape leaves the text for the note's tab in the Normal input mode
  (M-08.10, ux review F3). A Ctrl+click (Cmd+click) on a link follows it
  (M-07.3, `onFollow`); a jump a link asked for while the view was opening
  is made once it shows.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import type { AgentMarkView, EditorCommandName, PaneName, SpellMarksView } from './bindings';
  import { setActiveSourceView } from './editor/active';
  import { runJump, takeJump } from './editor/jump';
  import type { EditorInfo } from './editor/info.svelte';
  import type { InputModeName } from './editor/keys';
  import type { LineNumberSide } from './editor/numbers';
  import { sourceMinimap } from './editor/sourceMinimap';
  import { SourceView, type EditorBackend, type SpellMenuRequest } from './editor/sourceView';
  import SpellMenu, { CHANGED_SINCE } from './SpellMenu.svelte';
  import { wasmBackend } from './editor/wasmBackend';
  import { linkCompletion } from './editor/linkCompletion.svelte';
  import { tabId } from './tabs';

  type Props = {
    /** The note of the tab. */
    note: string;
    /** Its title, the name of the text field. */
    label: string;
    /** Cmd is the command key. */
    mac: boolean;
    /** The input mode of the settings. */
    mode?: InputModeName;
    /** M-08.7: line numbers on or off, and their side. */
    lineNumbers?: boolean;
    lineNumberSide?: LineNumberSide;
    /** UC-010 BR-009: the editor zoom in percent. */
    zoom?: number;
    /** UC-010 BR-011: long lines wrap. */
    softWrap?: boolean;
    /** What the status line and the minimap show of the view. */
    info?: EditorInfo;
    /** The keyboard goes into the text once it is shown. */
    focus?: boolean;
    onError: (message: string) => void;
    /** M-10.9: the keys ask to store the note or close its tab. */
    onCommand?: (command: EditorCommandName) => void;
    /** M-07.3: a Ctrl+click (Cmd+click) at `at` (web units) follows the link there. */
    onFollow?: (at: number, side: boolean) => void;
    /** M-17.6: the agent cursors in this note. */
    agentMarks?: AgentMarkView[];
    /** FR-099: the last marks of the spell check in this note. */
    spellMarks?: SpellMarksView | null;
    /** UC-005 A28: a correction was made ("Corrected to ferry"). */
    onCorrected?: (suggestion: string) => void;
    backend?: EditorBackend;
    /** UC-004 BR-018: the pane of the split view the view is in. */
    pane?: PaneName;
  };

  let {
    note,
    label,
    mac,
    mode = 'Normal',
    lineNumbers = false,
    lineNumberSide = 'Left',
    zoom = 100,
    softWrap = true,
    info,
    focus = false,
    onError,
    onCommand,
    onFollow,
    agentMarks = [],
    spellMarks = null,
    onCorrected,
    pane = 'left',
    backend = wasmBackend(mac, pane),
  }: Props = $props();

  let host = $state<HTMLElement | null>(null);
  let loading = $state(true);
  let view = $state.raw<SourceView | null>(null);

  /** Escape: the keyboard goes to the note's tab. */
  function leave() {
    document.getElementById(tabId(note, pane))?.focus();
  }

  $effect(() => {
    view?.setMode(mode);
  });

  $effect(() => {
    view?.setLineNumbers(lineNumbers, lineNumberSide);
  });

  $effect(() => {
    view?.setAgentMarks(agentMarks);
  });

  $effect(() => {
    view?.setZoom(zoom);
  });

  $effect(() => {
    if (spellMarks) {
      view?.takeSpell(spellMarks);
    }
  });

  /** UC-005 A28: the spell menu open now, and for which word. */
  let spellMenu = $state<SpellMenuRequest | null>(null);

  /**
   * UC-005 A28 step 2: the correction is made when the text is still the
   * one the word was marked in; otherwise the status bar says so.
   */
  async function correct(request: SpellMenuRequest, suggestion: string) {
    const made = (await view?.correct(request, suggestion)) ?? false;
    if (made) {
      onCorrected?.(suggestion);
    } else {
      onError(CHANGED_SINCE);
    }
  }

  /**
   * The menu closes before the entry chosen acts; the request stays until
   * then, so a correction still knows its word.
   */
  function closeSpellMenu(back: boolean) {
    const closing = spellMenu;
    queueMicrotask(() => {
      if (spellMenu === closing) spellMenu = null;
    });
    if (back) {
      view?.focus();
    }
  }

  $effect(() => {
    view?.setSoftWrap(softWrap);
  });

  /** FR-080: the text names the list of link completion while it shows. */
  const linkAria = $derived(linkCompletion.aria(note));
  $effect(() => {
    view?.setLinkAria(linkAria);
  });

  onMount(() => {
    let opened: SourceView | null = null;
    let gone = false;
    SourceView.open(host!, note, backend, {
      mac,
      place: `${pane}:${note}`,
      label,
      onError,
      onLeave: leave,
      mode,
      lineNumbers: { shown: lineNumbers, side: lineNumberSide },
      zoom,
      softWrap,
      onPlace: (place) => {
        if (info) info.place = place;
      },
      onKeys: (status) => {
        if (info) info.keys = status;
      },
      onFound: () => {
        if (info) info.found++;
      },
      onCommand: (command) => onCommand?.(command),
      onRequest: (tool) => {
        if (info) info.request = tool;
      },
      onFollow: (at, side) => onFollow?.(at, side),
      onSpellMenu: (request) => {
        spellMenu = request;
      },
    })
      .then((made) => {
        if (gone) {
          made.destroy();
          return;
        }
        opened = made;
        view = made;
        setActiveSourceView(made, pane);
        if (info) {
          info.view = made;
          info.minimap = sourceMinimap(made);
        }
        loading = false;
        // M-07.3: a link led here, to a heading; M-13.3: a found place.
        const waiting = takeJump(note);
        if (waiting !== null) {
          runJump(made, waiting);
        } else if (focus) {
          made.focus();
        }
      })
      .catch((error: unknown) => {
        loading = false;
        onError(
          typeof error === 'object' && error !== null && 'message' in error
            ? String(error.message)
            : String(error),
        );
      });
    return () => {
      gone = true;
      if (opened) {
        setActiveSourceView(null, pane, opened);
        if (info?.view === opened) {
          info.view = null;
          info.minimap = null;
          info.keys = null;
          info.place = '';
        }
        opened.destroy();
      }
    };
  });
</script>

<div class="source" aria-busy={loading} bind:this={host}></div>
{#if spellMenu}
  {@const request = spellMenu}
  <SpellMenu
    word={request.shown.word}
    {note}
    x={request.x}
    y={request.y}
    onChoose={(suggestion) => void correct(request, suggestion)}
    {onError}
    onClose={closeSpellMenu}
  />
{/if}

<style>
  .source {
    height: 100%;
    min-height: 0;
    background: var(--palette-background, var(--ui-field));
  }
</style>
