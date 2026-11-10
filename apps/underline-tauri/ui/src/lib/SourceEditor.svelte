<!--
  M-08 (UC-005): the source view of one note. Its text lives in the
  window's editor core (ADR-025, editor/wasmBackend.ts), which answers every
  key and sends the edits to the backend after the frame; CodeMirror shows
  it (editor/sourceView.ts). The backend's colours (`ed-highlight`) reach it
  through the shell, which listens for the whole window; the sections and
  Nano hits come from the core. While the text loads the area stays empty;
  a failure is said in the status bar.

  It follows the input mode and the line numbers of the settings (M-11,
  M-08.7) and tells `info` the place of the cursor and the keys' status for
  the status line (M-11.4).

  Escape leaves the text for the note's tab in the Normal input mode
  (M-08.10, ux review F3).
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import type { EditorCommandName } from './bindings';
  import { setActiveSourceView } from './editor/active';
  import type { EditorInfo } from './editor/info.svelte';
  import type { InputModeName } from './editor/keys';
  import type { LineNumberSide } from './editor/numbers';
  import { sourceMinimap } from './editor/sourceMinimap';
  import { SourceView, type EditorBackend } from './editor/sourceView';
  import { wasmBackend } from './editor/wasmBackend';
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
    /** What the status line and the minimap show of the view. */
    info?: EditorInfo;
    /** The keyboard goes into the text once it is shown. */
    focus?: boolean;
    onError: (message: string) => void;
    /** M-10.9: the keys ask to store the note or close its tab. */
    onCommand?: (command: EditorCommandName) => void;
    backend?: EditorBackend;
  };

  let {
    note,
    label,
    mac,
    mode = 'Normal',
    lineNumbers = false,
    lineNumberSide = 'Left',
    info,
    focus = false,
    onError,
    onCommand,
    backend = wasmBackend(mac),
  }: Props = $props();

  let host = $state<HTMLElement | null>(null);
  let loading = $state(true);
  let view = $state.raw<SourceView | null>(null);

  /** Escape: the keyboard goes to the note's tab. */
  function leave() {
    document.getElementById(tabId(note))?.focus();
  }

  $effect(() => {
    view?.setMode(mode);
  });

  $effect(() => {
    view?.setLineNumbers(lineNumbers, lineNumberSide);
  });

  onMount(() => {
    let opened: SourceView | null = null;
    let gone = false;
    SourceView.open(host!, note, backend, {
      mac,
      label,
      onError,
      onLeave: leave,
      mode,
      lineNumbers: { shown: lineNumbers, side: lineNumberSide },
      onPlace: (place) => {
        if (info) info.place = place;
      },
      onKeys: (status) => {
        if (info) info.keys = status;
      },
      onCommand: (command) => onCommand?.(command),
      onRequest: (tool) => {
        if (info) info.request = tool;
      },
    })
      .then((made) => {
        if (gone) {
          made.destroy();
          return;
        }
        opened = made;
        view = made;
        setActiveSourceView(made);
        if (info) {
          info.view = made;
          info.minimap = sourceMinimap(made);
        }
        loading = false;
        if (focus) {
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
        setActiveSourceView(null);
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

<style>
  .source {
    height: 100%;
    min-height: 0;
    background: var(--palette-background, var(--ui-field));
  }
</style>
