<!--
  M-20.1 to M-20.5, UC-010: the window "Settings", as egui's `show_window`
  draws it: 460 px wide, resizable, not modal, closed with "×", Escape or
  Ctrl/Cmd+, again.

  - Two parts (M-20.2): "For me", and "This workspace" only while a
    workspace is open (A9). The part shown stays while Underline runs.
  - Each part says where it applies; "This workspace" of a repository
    that shares its settings file says so (BR-007).
  - One row per setting (M-20.3): the name, the control with the value in
    effect, and the third column (M-20.4): "As for me" (A2), "own",
    "as for me", "this workspace uses its own value", "this workspace asks
    for more; not followed".
  - A change applies and is stored at once (M-20.5); the backend answers
    with what the window shows now, so a control never keeps a value that
    did not take, and with what went wrong (A6, A7), which the window shows
    until the next change applies (ux review F10).
  - Numbers are sent when the field is left, with Enter, or when the
    window closes; the arrow and page keys step them and send the last
    step 250 ms after the keys rest. While a step waits or a number is
    typed, answers do not overwrite the field.
  - Escape closes the window. A native list that is open keeps its keys
    (Chromium, WebKitGTK), so the Escape that closes it never reaches the
    window; no flag guesses whether a list is open (app-tester F10: a
    guess left set swallowed the next Escape).

  The rows, their texts and choices come from the backend
  (`SettingsWindowView`); nothing is decided here.
-->
<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte';
  import DialogWindow from './DialogWindow.svelte';
  import type {
    Scope,
    SettingChoiceView,
    SettingControlView,
    SettingName,
    SettingRowView,
    SettingsWindowView,
  } from './bindings';
  import type { Shell } from './shell.svelte';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  /** How long arrow and page steps wait for the keys to rest (ux review F10). */
  const STEP_DELAY_MS = 250;

  const view = $derived<SettingsWindowView | null>(shell.settingsWindow);
  const scope = $derived<Scope>(view?.workspace ? shell.settingsScope : 'me');
  const rows = $derived<SettingRowView[]>(
    view === null ? [] : scope === 'me' ? view.me : view.thisWorkspace,
  );
  const scopes = $derived<{ id: Scope; label: string }[]>(
    view?.workspace
      ? [
          { id: 'me', label: 'For me' },
          { id: 'workspace', label: 'This workspace' },
        ]
      : [{ id: 'me', label: 'For me' }],
  );

  let tabs = $state<Record<string, HTMLButtonElement | null>>({});
  let frame = $state<HTMLElement | null>(null);
  let destroyed = false;

  // Bookkeeping of the handlers, read by no template or effect: plain
  // collections on purpose, not reactive ones.
  /** Per setting and part, the number of the last change sent. */
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  const sent = new Map<string, number>();
  /** Numbers typed but not sent, and steps waiting for the keys to rest. */
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  const dirty = new Set<string>();
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  const steps = new Map<string, ReturnType<typeof setTimeout>>();

  const panelId = 'settings-panel';
  const tabId = (id: Scope) => `settings-tab-${id}`;
  const controlId = (part: Scope, key: SettingName) => `setting-${part}-${key}`;

  function choose(id: Scope) {
    shell.settingsScope = id;
  }

  /** The tabs: arrows, Home and End move and choose (selection follows focus). */
  function onTabKey(event: KeyboardEvent) {
    const at = scopes.findIndex((s) => s.id === scope);
    let next: number;
    if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
      next = (at + 1) % scopes.length;
    } else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
      next = (at - 1 + scopes.length) % scopes.length;
    } else if (event.key === 'Home') {
      next = 0;
    } else if (event.key === 'End') {
      next = scopes.length - 1;
    } else {
      return;
    }
    event.preventDefault();
    const id = scopes[next].id;
    choose(id);
    void tick().then(() => tabs[id]?.focus());
  }

  /** The row of `key` in the part `part` of the view shown now. */
  function rowOf(part: Scope, key: SettingName): SettingRowView | undefined {
    const now = shell.settingsWindow;
    return (part === 'me' ? now?.me : now?.thisWorkspace)?.find((r) => r.key === key);
  }

  /**
   * Sends a change; when its answer is the newest for the setting, the
   * control shows the value in effect again, and the keyboard, if the
   * control it was on went away ("As for me"), goes to the row's control.
   */
  async function send(part: Scope, key: SettingName, value: string | null) {
    const id = controlId(part, key);
    const number = (sent.get(id) ?? 0) + 1;
    sent.set(id, number);
    await shell.changeSetting(part, key, value);
    if (destroyed || sent.get(id) !== number) {
      return;
    }
    await tick();
    if (destroyed || sent.get(id) !== number) {
      return;
    }
    const element = document.getElementById(id);
    const control = rowOf(part, key)?.control;
    if (!element || !control) {
      return;
    }
    show(element, control);
    const active = document.activeElement;
    if (active === null || active === document.body || !frame?.contains(active)) {
      element.focus();
    }
  }

  /** `element` shows the value of `control`, unless the user is changing it. */
  function show(element: HTMLElement, control: SettingControlView) {
    if (element instanceof HTMLSelectElement && control.kind === 'choice') {
      element.value = choiceValue(control);
    } else if (element instanceof HTMLInputElement && control.kind === 'check') {
      element.checked = control.on;
    } else if (
      element instanceof HTMLInputElement &&
      control.kind === 'number' &&
      !dirty.has(element.id) &&
      !steps.has(element.id)
    ) {
      element.value = String(control.value);
    }
  }

  // The numbers follow the view, except while one is typed or stepped.
  $effect(() => {
    for (const row of rows) {
      if (row.control.kind === 'number') {
        const element = document.getElementById(controlId(scope, row.key));
        if (element) {
          show(element, row.control);
        }
      }
    }
  });

  /**
   * The option the closed box shows: the selected choice, or `shown` where
   * its text is none of the choices' ("Markdown (from the notes)").
   */
  function choiceValue(control: Extract<SettingControlView, { kind: 'choice' }>): string {
    const at = control.choices.findIndex((c) => c.selected && c.label === control.shown);
    return at < 0 ? 'shown' : String(at);
  }

  function onChoice(row: SettingRowView, choices: SettingChoiceView[], event: Event) {
    const select = event.currentTarget as HTMLSelectElement;
    const choice = choices[Number(select.value)];
    if (choice !== undefined) {
      void send(scope, row.key, choice.value);
    }
  }

  function onCheck(row: SettingRowView, event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    void send(scope, row.key, String(input.checked));
  }

  /** A number typed: sent when the field is left, with Enter, or on closing. */
  function commitNumber(input: HTMLInputElement, part: Scope, key: SettingName) {
    const waiting = steps.get(input.id);
    if (waiting !== undefined) {
      clearTimeout(waiting);
      steps.delete(input.id);
    }
    dirty.delete(input.id);
    const control = rowOf(part, key)?.control;
    const current = control?.kind === 'number' ? control.value : null;
    const typed = input.value.trim();
    if (!/^\d+$/.test(typed)) {
      // Nothing that could be a number: the value in effect stays.
      if (current !== null) {
        input.value = String(current);
      }
      return;
    }
    if (Number(typed) !== current) {
      void send(part, key, typed);
    }
  }

  /** Enter sends; arrow and page keys step and send once the keys rest. */
  function onNumberKey(row: SettingRowView, min: number, max: number, event: KeyboardEvent) {
    const input = event.currentTarget as HTMLInputElement;
    const part = scope;
    if (event.key === 'Enter') {
      event.preventDefault();
      commitNumber(input, part, row.key);
      return;
    }
    const step = (
      { ArrowUp: 1, ArrowDown: -1, PageUp: 10, PageDown: -10 } as Record<string, number>
    )[event.key];
    if (step === undefined || event.altKey || event.ctrlKey || event.metaKey) {
      return;
    }
    event.preventDefault();
    const now = Number(input.value);
    const from = Number.isFinite(now) && input.value.trim() !== '' ? now : min;
    const next = Math.min(max, Math.max(min, Math.round(from) + step));
    input.value = String(next);
    const waiting = steps.get(input.id);
    if (waiting !== undefined) {
      clearTimeout(waiting);
    }
    steps.set(
      input.id,
      setTimeout(() => commitNumber(input, part, row.key), STEP_DELAY_MS),
    );
  }

  /** On closing: numbers typed or stepped but not sent are sent now. */
  function flushNumbers() {
    for (const id of new Set([...dirty, ...steps.keys()])) {
      const input = document.getElementById(id);
      const [, part, ...key] = id.split('-');
      if (input instanceof HTMLInputElement) {
        commitNumber(input, part as Scope, key.join('-') as SettingName);
      }
    }
  }

  onMount(() => {
    shell.settingsBeforeClose = flushNumbers;
  });

  onDestroy(() => {
    flushNumbers();
    destroyed = true;
    if (shell.settingsBeforeClose === flushNumbers) {
      shell.settingsBeforeClose = null;
    }
  });
</script>

<DialogWindow
  title="Settings"
  onClose={() => shell.closeSettings()}
  busy={view === null && shell.settingsProblem === null}
  step={view === null ? 'loading' : 'shown'}
  width={460}
  resizable
  closeButton
>
  <div class="frame" bind:this={frame}>
    {#if shell.settingsProblem !== null}
      <p class="problem" role="alert">{shell.settingsProblem}</p>
    {/if}
    {#if view !== null}
      {#if scopes.length > 1}
        <div class="scopes" role="tablist" aria-label="Settings">
          {#each scopes as part (part.id)}
            <button
              type="button"
              role="tab"
              id={tabId(part.id)}
              aria-selected={scope === part.id}
              aria-controls={panelId}
              tabindex={scope === part.id ? 0 : -1}
              data-autofocus={scope === part.id ? 'true' : undefined}
              bind:this={tabs[part.id]}
              onclick={() => choose(part.id)}
              onkeydown={onTabKey}>{part.label}</button
            >
          {/each}
        </div>
      {/if}
      <div
        class="panel"
        role={scopes.length > 1 ? 'tabpanel' : 'group'}
        id={panelId}
        aria-labelledby={scopes.length > 1 ? tabId(scope) : undefined}
        aria-label={scopes.length > 1 ? undefined : 'For me'}
      >
        {#if scope === 'me'}
          <p class="weak">These settings apply in every workspace.</p>
        {:else}
          <p class="weak">These settings apply in this workspace only.</p>
          {#if view.shared}
            <p class="problem">
              These values are in underline.yaml of the repository and apply to everyone who uses
              it.
            </p>
          {/if}
        {/if}
        <div class="grid" class:with-notes={rows.some((row) => row.note !== null)}>
          {#each rows as row, at (row.key)}
            {@const id = controlId(scope, row.key)}
            {@const control = row.control}
            {@const noteId = row.note === null ? undefined : `${id}-note`}
            {@const first = at === 0 && scopes.length === 1 ? 'true' : undefined}
            {#if control.kind === 'check'}
              <span class="name" id={`${id}-name`}>{row.label}</span>
              <label class="check">
                <input
                  {id}
                  type="checkbox"
                  checked={control.on}
                  aria-labelledby={`${id}-name ${id}-text`}
                  aria-describedby={noteId}
                  data-autofocus={first}
                  onchange={(event) => onCheck(row, event)}
                />
                <span id={`${id}-text`}>{control.text}</span>
              </label>
            {:else}
              <label class="name" for={id}>{row.label}</label>
              {#if control.kind === 'choice'}
                <select
                  {id}
                  value={choiceValue(control)}
                  aria-describedby={noteId}
                  data-autofocus={first}
                  onchange={(event) => onChoice(row, control.choices, event)}
                >
                  {#if choiceValue(control) === 'shown'}
                    <option value="shown" disabled hidden>{control.shown}</option>
                  {/if}
                  {#each control.choices as choice, n (choice.label)}
                    <option value={String(n)}>{choice.label}</option>
                  {/each}
                </select>
              {:else}
                <input
                  {id}
                  class="number"
                  type="number"
                  inputmode="numeric"
                  min={control.min}
                  max={control.max}
                  step="1"
                  title={`${control.min}–${control.max}`}
                  aria-describedby={noteId}
                  data-autofocus={first}
                  oninput={() => dirty.add(id)}
                  onchange={(event) => commitNumber(event.currentTarget, scope, row.key)}
                  onkeydown={(event) => onNumberKey(row, control.min, control.max, event)}
                />
              {/if}
            {/if}
            <span class="note" id={noteId}>
              {#if row.note?.kind === 'asForMe'}
                <button
                  type="button"
                  class="ui-button small"
                  title={row.note.hint}
                  aria-label={`${row.note.label}: ${row.label}`}
                  aria-describedby={`${id}-hint`}
                  onclick={() => void send(scope, row.key, null)}>{row.note.label}</button
                >
                <span class="hidden" id={`${id}-hint`}>{row.note.hint}</span>
              {:else if row.note?.kind === 'text'}
                <span class="weak">{row.note.text}</span>
              {/if}
            </span>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</DialogWindow>

<style>
  /* egui's selectable values: the part shown in the chosen colour, with a
     line below too, so it is not told by colour alone (NFR-010). */
  .scopes {
    display: flex;
    gap: var(--space-1);
    padding-bottom: var(--space-2);
    border-bottom: 1px solid var(--ui-rule);
  }

  [role='tab'] {
    min-height: 28px;
    padding: 0 var(--space-3);
    font: inherit;
    color: var(--ui-text);
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius);
    cursor: pointer;
  }

  [role='tab']:hover {
    background: color-mix(in srgb, var(--ui-chosen) 40%, transparent);
  }

  [role='tab'][aria-selected='true'] {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
    border-color: var(--ui-chosen-text);
  }

  .frame,
  .panel {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  /* The palette's weak colour reaches 4.5:1 on the bars in only eight of
     the eleven themes (Solarized 4.1–4.2, Nord 4.35); mixed with the text
     colour it reaches 4.57:1 in all (ux review F10, A-105). */
  .weak {
    color: color-mix(in srgb, var(--ui-hint) 40%, var(--ui-text));
  }

  .hidden {
    display: none;
  }

  /* Name, control, note: egui's grid of three columns, 12 px apart, rows
     8 px apart; the note takes what is left and wraps. */
  .grid {
    display: grid;
    grid-template-columns: minmax(6em, max-content) max-content minmax(0, max-content);
    gap: var(--space-2) var(--space-3);
    align-items: center;
    margin-top: var(--space-1);
  }

  /* A long note wraps at its spaces beside the controls. */
  .grid.with-notes {
    grid-template-columns: minmax(6em, max-content) max-content minmax(10em, 16em);
  }

  .name {
    overflow-wrap: break-word;
  }

  .check {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    min-height: 24px;
    cursor: pointer;
  }

  .check input {
    width: 16px;
    height: 16px;
    margin: 0;
    accent-color: var(--ui-chosen);
  }

  select,
  .number {
    box-sizing: border-box;
    min-height: 28px;
    font: inherit;
    color: var(--ui-text);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
  }

  select {
    max-width: 100%;
    background: var(--ui-button);
  }

  .number {
    width: 8ch;
    padding: 0 var(--space-1);
    background: var(--ui-field);
  }

  .note {
    min-width: 0;
    overflow-wrap: break-word;
  }

  .small {
    min-height: 24px;
    padding: 0 var(--space-2);
  }

  /* A narrow window: one column, each name above its control and note. */
  @media (max-width: 520px) {
    .grid,
    .grid.with-notes {
      grid-template-columns: minmax(0, 1fr);
      row-gap: 2px;
    }

    .name:not(:first-child) {
      margin-top: var(--space-2);
    }

    select {
      width: 100%;
    }
  }
</style>
