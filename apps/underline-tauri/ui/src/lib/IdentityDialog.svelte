<!--
  M-19.4, UC-012 A3: "Your name for commits", when git does not know who
  makes the commit. Name and email are kept in the user's own git settings
  for every repository (BR-005); "Commit" is enabled once the name is not
  empty and the email holds an '@', as in egui, and then commits with the
  message and choice of the commit box. "Cancel" or Escape commits nothing
  and keeps the box (A3 step 4).

  UX review F12 (A-137): a field left empty or wrong says why below it
  ("Enter your name.", "The email needs an @.", new texts) once the user
  left it; when git cannot keep the name, the dialog stays with what was
  typed and says why ("Your name could not be kept: …").
-->
<script lang="ts">
  import DialogWindow from './DialogWindow.svelte';
  import type { IdentityDialog, Shell } from './shell.svelte';

  type Props = { shell: Shell; dialog: IdentityDialog };

  let { shell, dialog }: Props = $props();

  const nameWrong = $derived(dialog.name.trim() === '');
  const emailWrong = $derived(!dialog.email.includes('@'));
  const ready = $derived(!nameWrong && !emailWrong);
  const busy = $derived(shell.commit.busy);
  /** The fields the user left, whose problems show. */
  let left = $state({ name: false, email: false });
  const id = Math.random().toString(36).slice(2);

  function submit(event: SubmitEvent) {
    event.preventDefault();
    left = { name: true, email: true };
    if (ready && !busy) {
      void shell.answerIdentity(dialog.name, dialog.email);
    }
  }
</script>

<DialogWindow
  title="Your name for commits"
  onClose={() => shell.closeIdentity()}
  {busy}
  describedBy="identity-text-{id}"
>
  <form class="form" onsubmit={submit} novalidate>
    <p id="identity-text-{id}">
      git does not know who makes the commit. Your name and email are kept in your git settings for
      every repository and show in the history.
    </p>
    <div class="fields">
      <label for="identity-name-{id}">Name</label>
      <div class="field">
        <input
          id="identity-name-{id}"
          type="text"
          autocomplete="name"
          spellcheck="false"
          data-autofocus="true"
          value={dialog.name}
          aria-invalid={left.name && nameWrong}
          aria-describedby={left.name && nameWrong ? `identity-name-why-${id}` : undefined}
          oninput={(event) => shell.editIdentity(dialog, { name: event.currentTarget.value })}
          onblur={() => (left = { ...left, name: true })}
        />
        {#if left.name && nameWrong}
          <p class="why" id="identity-name-why-{id}">Enter your name.</p>
        {/if}
      </div>
      <label for="identity-email-{id}">Email</label>
      <div class="field">
        <input
          id="identity-email-{id}"
          type="text"
          inputmode="email"
          autocomplete="email"
          spellcheck="false"
          value={dialog.email}
          aria-invalid={left.email && emailWrong}
          aria-describedby={left.email && emailWrong ? `identity-email-why-${id}` : undefined}
          oninput={(event) => shell.editIdentity(dialog, { email: event.currentTarget.value })}
          onblur={() => (left = { ...left, email: true })}
        />
        {#if left.email && emailWrong}
          <p class="why" id="identity-email-why-{id}">The email needs an @.</p>
        {/if}
      </div>
    </div>
    {#if dialog.problem !== null}
      <p class="problem" role="alert">
        <span aria-hidden="true">⚠ </span><span class="visually-hidden"
          >Error:
        </span>{dialog.problem}
      </p>
    {/if}
    <div class="actions">
      <button type="submit" class="ui-button" disabled={!ready || busy} aria-busy={busy}
        >Commit</button
      >
      <button type="button" class="ui-button" onclick={() => shell.closeIdentity()}>Cancel</button>
    </div>
  </form>
</DialogWindow>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    max-width: 420px;
  }

  /* egui's grid: the names beside their fields. */
  .fields {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: var(--space-2);
    align-items: start;
  }

  .fields label {
    padding-top: 4px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .why {
    color: var(--ui-hint);
  }

  .ui-button:disabled {
    color: var(--ui-hint);
    cursor: default;
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
