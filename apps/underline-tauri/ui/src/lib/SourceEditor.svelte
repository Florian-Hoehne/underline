<!--
  M-08 (UC-005): the source view of one note. It asks the backend for the
  text and shows it in CodeMirror (editor/sourceView.ts). The backend's
  colours (`ed-highlight`) and replaced texts (`ed-replaced`) reach it
  through the shell, which listens for the whole window. While the text
  loads the area stays empty; a failure is said in the status bar.

  Escape leaves the text for the note's tab (M-08.10, ux review F3).
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { setActiveSourceView } from './editor/active';
  import { SourceView, tauriBackend, type EditorBackend } from './editor/sourceView';
  import { tabId } from './tabs';

  type Props = {
    /** The note of the tab. */
    note: string;
    /** Its title, the name of the text field. */
    label: string;
    /** Cmd is the command key. */
    mac: boolean;
    /** The keyboard goes into the text once it is shown. */
    focus?: boolean;
    onError: (message: string) => void;
    backend?: EditorBackend;
  };

  let { note, label, mac, focus = false, onError, backend = tauriBackend }: Props = $props();

  let host = $state<HTMLElement | null>(null);
  let loading = $state(true);

  /** Escape: the keyboard goes to the note's tab. */
  function leave() {
    document.getElementById(tabId(note))?.focus();
  }

  onMount(() => {
    let view: SourceView | null = null;
    let gone = false;
    SourceView.open(host!, note, backend, { mac, label, onError, onLeave: leave })
      .then((opened) => {
        if (gone) {
          opened.destroy();
          return;
        }
        view = opened;
        setActiveSourceView(view);
        loading = false;
        if (focus) {
          view.focus();
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
      if (view) {
        setActiveSourceView(null);
        view.destroy();
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
