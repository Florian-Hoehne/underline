<!--
  UC-024 Edit Note Attributes (FR-093), in the note properties of the tool
  panel, below the text statistics and the tags (step 2): the attributes of
  the active note in the order of the note, each with its name and its value
  as its kind shows it (BR-002): a text or number field, a date with a
  calendar, a switch, a field per item of a list, or the markup of a nested
  value with "Show in source view" (A6). A front matter that cannot be read
  shows as its text with the line and the reason, and "Show in source view"
  (A5).

  - A change is written when it is confirmed (Enter, or leaving the field;
    a switch at once) through the view shown, one undo step (BR-006); a
    refused one says why below its field, marks the field and keeps what was
    typed (A3, A4). Typing again or Escape takes the message away.
  - What is typed and not confirmed stays in its field while the note
    changes (A7), and goes with its attribute when that is gone.
  - "Add attribute" opens a small form: name, kind and value (A1, BR-004);
    the palette's "Add note attribute" opens it with the keyboard in the
    name; a refusal puts the keyboard into the field it is about. Escape
    closes it.
  - BR-007: everything by keyboard: Tab goes from field to field and then to
    the remove button after each attribute; Enter confirms. After a remove
    the keyboard goes to the next attribute (the one before, or "Add
    attribute"), the status says how to undo, and Ctrl+Z (Cmd+Z) in the
    panel undoes it in the note.

  No rule of the attributes lives here: the editor core reads and writes
  them (editor/attributes.ts).
-->
<script lang="ts">
  import { tick, untrack } from 'svelte';
  import type {
    AttributeChangeView,
    AttributeFieldName,
    AttributeKindName,
    NoteAttributeView,
    NoteHeadView,
  } from './bindings';
  import { changeAttribute, kindsFor, showInSource, undoIn, valueOf } from './editor/attributes';
  import type { Shell } from './shell.svelte';

  type Props = {
    shell: Shell;
    note: string;
    head: NoteHeadView;
    /** Asks for the attributes again at once, after a change. */
    onChanged?: () => void;
  };

  let { shell, note, head, onChanged }: Props = $props();

  /** A7: what is typed and not confirmed yet, by field. */
  let drafts = $state<Record<string, string>>({});
  /** A3, A4: why a change of an attribute was not written, by attribute. */
  let problems = $state<Record<string, string>>({});
  /** The form of a new attribute, while open. */
  let adding = $state(false);
  let newName = $state('');
  let newKind = $state<AttributeKindName>('text');
  let newValue = $state('');
  let addProblem = $state<string | null>(null);
  let addAbout = $state<AttributeFieldName | null>(null);
  let nameField = $state<HTMLInputElement | null>(null);
  let valueField = $state<HTMLInputElement | HTMLSelectElement | null>(null);
  let addButton = $state<HTMLButtonElement | null>(null);

  const kinds = $derived(kindsFor(head.asciidoc));
  const mac = $derived(shell.platform === 'macos');

  // Another note: nothing typed or said of the one before stays.
  $effect(() => {
    void note;
    untrack(() => {
      drafts = {};
      problems = {};
      closeForm(false);
    });
  });

  // A7 (app test): the draft and the message of an attribute that is gone
  // go with it, so they do not come back with an attribute of that name.
  // The field the keyboard is in keeps what is being typed: a text changed
  // outside may lack the attribute only for a moment.
  $effect(() => {
    const names = new Set(head.attributes.map((attribute) => attribute.name));
    untrack(() => {
      const typing = (document.activeElement as HTMLElement | null)?.dataset?.key;
      const keep = (key: string) => key === typing || names.has(nameOfKey(key));
      if (Object.keys(drafts).some((key) => !keep(key))) {
        drafts = Object.fromEntries(Object.entries(drafts).filter(([key]) => keep(key)));
      }
      if (Object.keys(problems).some((key) => !names.has(key))) {
        problems = Object.fromEntries(Object.entries(problems).filter(([key]) => names.has(key)));
      }
    });
  });

  // The palette's "Add note attribute" opens the form with the keyboard.
  let asked = untrack(() => shell.attributeAsked);
  $effect(() => {
    const now = shell.attributeAsked;
    if (now !== asked) {
      asked = now;
      void openForm();
    }
  });

  async function openForm() {
    adding = true;
    await tick();
    nameField?.focus();
  }

  function closeForm(focusButton = true) {
    adding = false;
    newName = '';
    newKind = 'text';
    newValue = '';
    addProblem = null;
    addAbout = null;
    if (focusButton) {
      void tick().then(() => addButton?.focus());
    }
  }

  /** `record` without `key`. */
  function without(record: Record<string, string>, key: string): Record<string, string> {
    const rest = { ...record };
    delete rest[key];
    return rest;
  }

  /** The field keys of an attribute's value and of its items. */
  const valueKey = (name: string) => `${name}`;
  const itemKey = (name: string, at: number) => `${name}#${at}`;
  const newItemKey = (name: string) => `${name}+`;
  /** The attribute a field key belongs to. */
  const nameOfKey = (key: string) => key.replace(/(#\d+|\+)$/, '');
  /** The ids of the fields, for the keyboard. */
  const fieldId = (name: string) => `attribute-${name}`;
  const itemId = (name: string, at: number) => `attribute-${name}-item-${at}`;
  const newItemId = (name: string) => `attribute-${name}-new-item`;
  const problemId = (name: string) => `attribute-problem-${name}`;

  /** Puts the keyboard on the element with id `id`, or on "Add attribute". */
  function focusId(id: string | null) {
    const target = id === null ? null : document.getElementById(id);
    (target ?? addButton)?.focus();
  }

  /** The field that takes the keyboard after attribute `at` is removed. */
  function afterRemove(at: number): string | null {
    const next = head.attributes[at + 1] ?? head.attributes[at - 1];
    if (!next) {
      return null;
    }
    if (next.kind === 'list') {
      return next.items.length > 0 ? itemId(next.name, 0) : newItemId(next.name);
    }
    return next.kind === 'nested' ? `remove-${next.name}` : fieldId(next.name);
  }

  /**
   * Writes `change`; `keys` are the fields it came from, whose drafts go
   * when it is written. Resolves whether it was written.
   */
  async function write(change: AttributeChangeView, keys: string[], about: string) {
    const refusal = await changeAttribute(note, change);
    if (refusal === null) {
      const nextDrafts = { ...drafts };
      for (const key of keys) {
        delete nextDrafts[key];
      }
      drafts = nextDrafts;
      problems = without(problems, about);
      onChanged?.();
      return true;
    }
    problems = { ...problems, [about]: refusal.problem };
    return false;
  }

  /** Typing again takes the message of the attribute away. */
  function typed(key: string, value: string) {
    drafts = { ...drafts, [key]: value };
    problems = without(problems, nameOfKey(key));
  }

  /** A text, number or date confirmed: written when it changed. */
  function confirmValue(attribute: NoteAttributeView) {
    const key = valueKey(attribute.name);
    const draft = drafts[key];
    if (draft === undefined) {
      return;
    }
    if (draft === attribute.value) {
      drafts = without(drafts, key);
      problems = without(problems, attribute.name);
      return;
    }
    void write(
      { kind: 'set', name: attribute.name, value: valueOf(attribute.kind, draft) },
      [key],
      attribute.name,
    );
  }

  function setSwitch(attribute: NoteAttributeView, on: boolean) {
    void write(
      { kind: 'set', name: attribute.name, value: { kind: 'yesNo', yes: on } },
      [],
      attribute.name,
    );
  }

  /** A list's items with item `at` replaced by `value` (removed when empty), or `value` added. */
  function itemsWith(attribute: NoteAttributeView, at: number | null, value: string): string[] {
    const items = [...attribute.items];
    if (at === null) {
      if (value.trim() !== '') {
        items.push(value.trim());
      }
    } else if (value.trim() === '') {
      items.splice(at, 1);
    } else {
      items[at] = value.trim();
    }
    return items;
  }

  function confirmItem(attribute: NoteAttributeView, at: number | null) {
    const key = at === null ? newItemKey(attribute.name) : itemKey(attribute.name, at);
    const draft = drafts[key];
    if (draft === undefined || (at !== null && draft === attribute.items[at])) {
      drafts = without(drafts, key);
      return;
    }
    if (at === null && draft.trim() === '') {
      return;
    }
    void write(
      {
        kind: 'set',
        name: attribute.name,
        value: { kind: 'list', items: itemsWith(attribute, at, draft) },
      },
      [key],
      attribute.name,
    );
  }

  async function removeItem(attribute: NoteAttributeView, at: number) {
    const items = attribute.items.filter((_, i) => i !== at);
    // The keyboard goes to the item that takes its place, the one before,
    // or the field of a new item.
    const next =
      at < items.length
        ? itemId(attribute.name, at)
        : at > 0
          ? itemId(attribute.name, at - 1)
          : newItemId(attribute.name);
    const written = await write(
      { kind: 'set', name: attribute.name, value: { kind: 'list', items } },
      [itemKey(attribute.name, at)],
      attribute.name,
    );
    if (written) {
      await tick();
      focusId(next);
      const item = attribute.items[at] ?? '';
      shell.status = `Removed item ${item} of ${attribute.name}. Undo with ${mac ? 'Cmd' : 'Ctrl'}+Z.`;
    }
  }

  async function remove(attribute: NoteAttributeView, at: number) {
    const next = afterRemove(at);
    const written = await write(
      { kind: 'remove', name: attribute.name },
      [valueKey(attribute.name)],
      attribute.name,
    );
    if (written) {
      focusId(next);
      shell.status = `Removed attribute ${attribute.name}. Undo with ${mac ? 'Cmd' : 'Ctrl'}+Z.`;
    }
  }

  async function add(event: SubmitEvent) {
    event.preventDefault();
    const refusal = await changeAttribute(note, {
      kind: 'add',
      name: newName,
      value: valueOf(newKind, newKind === 'yesNo' ? String(newValue === 'true') : newValue),
    });
    if (refusal === null) {
      onChanged?.();
      closeForm();
      return;
    }
    addProblem = refusal.problem;
    addAbout = refusal.about;
    await tick();
    if (refusal.about === 'name') {
      nameField?.focus();
    } else if (refusal.about === 'value') {
      valueField?.focus();
    }
  }

  /** Enter confirms a field; Escape gives up what was typed and its message. */
  function fieldKey(event: KeyboardEvent, key: string, confirm: () => void) {
    if (event.key === 'Enter') {
      event.preventDefault();
      confirm();
    } else if (
      event.key === 'Escape' &&
      (drafts[key] !== undefined || problems[nameOfKey(key)] !== undefined)
    ) {
      event.preventDefault();
      event.stopPropagation();
      drafts = without(drafts, key);
      problems = without(problems, nameOfKey(key));
    }
  }

  function formKey(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      closeForm();
    }
  }

  /**
   * BR-006 (UX review): Ctrl+Z (Cmd+Z) in the panel undoes the last change
   * of the note, unless a field holds text typed and not confirmed, which
   * the field undoes itself.
   */
  function panelKey(event: KeyboardEvent) {
    const command = mac ? event.metaKey : event.ctrlKey;
    if (!command || event.shiftKey || event.altKey || event.key.toLowerCase() !== 'z') {
      return;
    }
    const target = event.target as HTMLElement;
    const key = target.dataset.key;
    const ownText =
      (target instanceof HTMLInputElement && target.type !== 'checkbox' && key === undefined) ||
      (key !== undefined && drafts[key] !== undefined);
    if (ownText) {
      return;
    }
    if (undoIn(note)) {
      event.preventDefault();
      onChanged?.();
    }
  }

  function show(offset: number) {
    void showInSource(note, offset, (n, view) => shell.setView(n, view));
  }

  const UNDO_KEYS = $derived(mac ? 'Meta+Z' : 'Control+Z');
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section class="attributes" aria-labelledby="attributes-title" onkeydown={panelKey}>
  <h3 id="attributes-title">Attributes</h3>
  {#if head.problem}
    <!-- A5: the front matter as its text, never rewritten. -->
    <div class="problem" role="group" aria-label="Front matter that cannot be read">
      <p class="said" aria-live="polite">
        <span class="mark" aria-hidden="true">⚠</span> The front matter has an error in line {head
          .problem.line}: {head.problem.reason}. The note was not changed. Fix it in the source
        view.
      </p>
      <pre class="markup">{head.problem.markup}</pre>
      <button type="button" class="ui-button" onclick={() => show(head.problem?.offset ?? 0)}
        >Show in source view</button
      >
    </div>
  {:else}
    {#if head.attributes.length === 0}
      <p class="weak">No attributes</p>
    {/if}
    <ul class="list" aria-label="Attributes">
      {#each head.attributes as attribute, index (attribute.name)}
        {@const key = valueKey(attribute.name)}
        {@const id = fieldId(attribute.name)}
        {@const said = problems[attribute.name]}
        {@const describedBy = said ? problemId(attribute.name) : undefined}
        <li class="attribute">
          {#if attribute.kind === 'list' || attribute.kind === 'nested'}
            <span class="name">{attribute.name}</span>
          {:else}
            <label class="name" for={id}>{attribute.name}</label>
          {/if}
          <div class="body">
            {#if attribute.kind === 'nested'}
              <pre class="markup" {id} aria-label="{attribute.name}, markup">{attribute.value}</pre>
              <p class="weak hint">Changed only in the source view.</p>
              <button type="button" class="ui-button" onclick={() => show(attribute.offset)}
                >Show in source view</button
              >
            {:else if attribute.kind === 'yesNo'}
              <label class="switch">
                <input
                  {id}
                  type="checkbox"
                  role="switch"
                  checked={attribute.value === 'true'}
                  aria-invalid={said ? 'true' : undefined}
                  aria-describedby={describedBy}
                  onchange={(event) => setSwitch(attribute, event.currentTarget.checked)}
                />
                <span>{attribute.value === 'true' ? 'Yes' : 'No'}</span>
              </label>
            {:else if attribute.kind === 'list'}
              <ul class="items" aria-label="Items of {attribute.name}">
                {#each attribute.items as item, at (`${at}`)}
                  {@const itemField = itemKey(attribute.name, at)}
                  <li class="item">
                    <input
                      id={itemId(attribute.name, at)}
                      class="field"
                      data-key={itemField}
                      aria-label="Item {at + 1} of {attribute.name}"
                      aria-invalid={said ? 'true' : undefined}
                      aria-describedby={describedBy}
                      value={drafts[itemField] ?? item}
                      oninput={(event) => typed(itemField, event.currentTarget.value)}
                      onchange={() => confirmItem(attribute, at)}
                      onkeydown={(event) =>
                        fieldKey(event, itemField, () => confirmItem(attribute, at))}
                    />
                    <button
                      type="button"
                      class="remove"
                      aria-label="Remove item {item} of {attribute.name}"
                      title="Remove item"
                      onclick={() => void removeItem(attribute, at)}>×</button
                    >
                  </li>
                {/each}
                <li class="item">
                  <input
                    id={newItemId(attribute.name)}
                    class="field"
                    data-key={newItemKey(attribute.name)}
                    aria-label="New item of {attribute.name}"
                    aria-invalid={said ? 'true' : undefined}
                    aria-describedby={describedBy}
                    placeholder="Add item"
                    value={drafts[newItemKey(attribute.name)] ?? ''}
                    oninput={(event) =>
                      typed(newItemKey(attribute.name), event.currentTarget.value)}
                    onchange={() => confirmItem(attribute, null)}
                    onkeydown={(event) =>
                      fieldKey(event, newItemKey(attribute.name), () =>
                        confirmItem(attribute, null),
                      )}
                  />
                </li>
              </ul>
            {:else}
              <input
                {id}
                class="field"
                data-key={key}
                type={attribute.kind === 'date' ? 'date' : 'text'}
                inputmode={attribute.kind === 'number' ? 'decimal' : undefined}
                value={drafts[key] ?? attribute.value}
                aria-invalid={said ? 'true' : undefined}
                aria-describedby={[describedBy, attribute.kind === 'date' ? 'date-hint' : undefined]
                  .filter(Boolean)
                  .join(' ') || undefined}
                oninput={(event) => typed(key, event.currentTarget.value)}
                onchange={() => confirmValue(attribute)}
                onkeydown={(event) => fieldKey(event, key, () => confirmValue(attribute))}
              />
            {/if}
            <!-- BR-007: after its field in the order of Tab. -->
            <button
              type="button"
              class="remove"
              id="remove-{attribute.name}"
              aria-label="Remove attribute {attribute.name}"
              aria-keyshortcuts={UNDO_KEYS}
              title="Remove attribute"
              onclick={() => void remove(attribute, index)}>×</button
            >
          </div>
          {#if said}
            <p class="said" id={problemId(attribute.name)} role="alert">
              <span class="mark" aria-hidden="true">⚠</span>
              {said}
            </p>
          {/if}
        </li>
      {/each}
    </ul>
    {#if head.attributes.some((attribute) => attribute.kind === 'date')}
      <p class="weak hint" id="date-hint">Dates are written year-month-day in the file.</p>
    {/if}
    {#if adding}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <form class="add" aria-label="New attribute" onsubmit={add} onkeydown={formKey}>
        <label for="new-attribute-name">Name</label>
        <input
          id="new-attribute-name"
          class="field"
          bind:this={nameField}
          bind:value={newName}
          autocomplete="off"
          spellcheck="false"
          aria-invalid={addProblem && addAbout === 'name' ? 'true' : undefined}
          aria-describedby={addProblem && addAbout === 'name' ? 'new-attribute-problem' : undefined}
          oninput={() => (addProblem = null)}
        />
        <label for="new-attribute-kind">Kind</label>
        <select id="new-attribute-kind" class="field select" bind:value={newKind}>
          {#each kinds as choice (choice.kind)}
            <option value={choice.kind}>{choice.label}</option>
          {/each}
        </select>
        <label for="new-attribute-value">Value</label>
        {#if newKind === 'yesNo'}
          <select
            id="new-attribute-value"
            class="field select"
            bind:this={valueField}
            bind:value={newValue}
          >
            <option value="false">No</option>
            <option value="true">Yes</option>
          </select>
        {:else}
          <input
            id="new-attribute-value"
            class="field"
            bind:this={valueField}
            type={newKind === 'date' ? 'date' : 'text'}
            inputmode={newKind === 'number' ? 'decimal' : undefined}
            bind:value={newValue}
            aria-invalid={addProblem && addAbout === 'value' ? 'true' : undefined}
            aria-describedby={[
              addProblem && addAbout === 'value' ? 'new-attribute-problem' : undefined,
              newKind === 'list' ? 'new-attribute-list-hint' : undefined,
            ]
              .filter(Boolean)
              .join(' ') || undefined}
            oninput={() => (addProblem = null)}
          />
        {/if}
        {#if newKind === 'list'}
          <p class="weak hint wide" id="new-attribute-list-hint">Items, separated by commas.</p>
        {/if}
        {#if addProblem}
          <p class="said wide" id="new-attribute-problem" role="alert">
            <span class="mark" aria-hidden="true">⚠</span>
            {addProblem}
          </p>
        {/if}
        <div class="actions wide">
          <button type="submit" class="ui-button">Add</button>
          <button type="button" class="ui-button" onclick={() => closeForm()}>Cancel</button>
        </div>
      </form>
    {:else}
      <button type="button" class="ui-button" bind:this={addButton} onclick={() => void openForm()}
        >Add attribute</button
      >
    {/if}
  {/if}
</section>

<style>
  .attributes {
    container-type: inline-size;
    margin-top: var(--space-3);
    padding-top: var(--space-2);
    border-top: 1px solid var(--ui-rule);
  }

  h3 {
    margin: 0 0 var(--space-1);
    font-size: 1em;
    font-weight: 700;
  }

  .weak,
  .hint {
    margin: 0 0 var(--space-2);
    color: var(--ui-hint);
  }

  .hint {
    font-size: 0.85em;
  }

  .list,
  .items {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .attribute {
    margin-bottom: var(--space-2);
  }

  /* The names as the markup writes them. */
  .name {
    display: block;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 0.9em;
    color: var(--ui-hint);
    overflow-wrap: anywhere;
  }

  /* The value, then its remove button, side by side. */
  .body {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: var(--space-1);
    align-items: start;
  }

  .body > :not(.remove) {
    grid-column: 1;
  }

  .body > .remove {
    grid-row: 1;
    grid-column: 2;
  }

  .field {
    box-sizing: border-box;
    width: 100%;
    min-width: 0;
    min-height: 28px;
    padding: 0 var(--space-2);
    font: inherit;
    color: var(--ui-text);
    background: var(--ui-field);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
  }

  .select {
    background: var(--ui-button);
  }

  /* A3, A4: a refused value, marked by its border and its message. */
  .field[aria-invalid='true'],
  .switch input[aria-invalid='true'] {
    border: 2px solid var(--ui-overdue, currentColor);
    outline-color: var(--ui-overdue, currentColor);
  }

  .item {
    display: flex;
    gap: var(--space-1);
    align-items: center;
  }

  .item + .item {
    margin-top: var(--space-1);
  }

  .remove {
    flex: none;
    width: 28px;
    height: 28px;
    padding: 0;
    font: inherit;
    font-size: 1.1em;
    line-height: 1;
    color: var(--ui-hint);
    cursor: pointer;
    background: none;
    border: 1px solid transparent;
    border-radius: var(--radius);
  }

  .remove:hover {
    color: var(--ui-text);
    border-color: var(--ui-border);
  }

  .switch {
    display: inline-flex;
    gap: var(--space-2);
    align-items: center;
    min-height: 28px;
    cursor: pointer;
  }

  .switch input {
    width: 18px;
    height: 18px;
    margin: 0;
  }

  .markup {
    box-sizing: border-box;
    max-height: 12em;
    margin: var(--space-1) 0;
    padding: var(--space-1) var(--space-2);
    overflow: auto;
    font-family: var(--font-mono);
    font-size: 0.9em;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    border-left: 2px solid var(--ui-rule);
  }

  /* Why something was not written: the theme's error colour, a sign, and
     the words, so colour is not the only cue. */
  .said {
    margin: var(--space-1) 0;
    color: var(--ui-overdue, inherit);
    overflow-wrap: anywhere;
  }

  .mark {
    font-weight: 700;
  }

  .add {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: var(--space-1) var(--space-2);
    align-items: center;
  }

  .add label {
    color: var(--ui-hint);
  }

  .add .wide {
    grid-column: 1 / -1;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    margin-top: var(--space-1);
  }

  /* A narrow panel: each label above its field. */
  @container (max-width: 200px) {
    .add {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
