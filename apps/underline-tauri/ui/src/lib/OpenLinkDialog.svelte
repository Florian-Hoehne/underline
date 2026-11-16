<!--
  M-07.3, UC-004 A4, BR-007 (Q-61, Q-62): the question before a followed
  link opens something that may start another program: an address of
  another scheme than http, https and mailto ("Open ssh: link"), a file
  that may run as a program ("Open program"), or a file by a `file://`
  address or on another computer ("Open file"). The session words the
  question, naming the scheme or the file and its kind, and shows the
  whole address below it, with invisible characters written out; "Copy"
  puts that on the clipboard. The button names what it does ("Open link",
  "Open program", "Open file"); "Cancel" opens nothing. It is asked every
  time; nothing is remembered, and the answer names its question.

  A program carries a calm warning: a sign and the sentence in bold, no
  colour of alarm. An `alertdialog`, not modal like egui's windows; Escape
  cancels (D-02). The keyboard starts on "Cancel", which keeps a visible
  ring as the default, so Enter opens nothing by mistake.
-->
<script lang="ts">
  import DialogWindow from './DialogWindow.svelte';
  import type { Shell, OpenQuestion } from './shell.svelte';

  type Props = { shell: Shell; question: OpenQuestion };

  let { shell, question }: Props = $props();

  const textId = `open-link-${Math.random().toString(36).slice(2)}`;
  const OPEN_LABEL = { link: 'Open link', program: 'Open program', file: 'Open file' } as const;

  let copied = $state(false);

  async function copy(): Promise<void> {
    copied = await shell.copyOpenAddress();
  }
</script>

<DialogWindow
  title={question.title}
  role="alertdialog"
  onClose={() => void shell.answerOpen(false)}
  step={String(question.id)}
  describedBy={textId}
>
  <div class="text" id={textId}>
    <p>
      {question.question}
      {#if question.warning !== null}
        <span class="warning">
          <span aria-hidden="true">⚠</span>
          <span class="visually-hidden">Warning:</span>
          <strong>{question.warning}</strong>
        </span>
      {/if}
      Open it?
    </p>
    <p class="target">
      <span class="visually-hidden">Address:</span>
      {question.target}
    </p>
  </div>
  <div class="actions">
    <button type="button" class="ui-button copy" onclick={() => void copy()}
      >{copied ? 'Copied' : 'Copy'}</button
    >
    <span class="gap"></span>
    <button type="button" class="ui-button" onclick={() => void shell.answerOpen(true)}
      >{OPEN_LABEL[question.about]}</button
    >
    <button
      type="button"
      class="ui-button default"
      data-autofocus="true"
      onclick={() => void shell.answerOpen(false)}>Cancel</button
    >
  </div>
</DialogWindow>

<style>
  .text {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    max-width: 36rem;
  }

  /* The whole address, so the user sees what opens; a long one breaks
     anywhere instead of widening the dialog. */
  .target {
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }

  .warning {
    white-space: normal;
  }

  /* "Copy" stands apart from the answers. */
  .gap {
    flex: 1;
  }

  /* The default answer keeps its ring, focused or not, so it is clear
     what Enter does. */
  .default {
    outline: 2px solid var(--ui-focus);
    outline-offset: 1px;
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    margin: -1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
