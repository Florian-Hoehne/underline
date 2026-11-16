<!--
  FR-075, UC-008 A7, BR-011: the three search switches of a search field,
  match case, whole word and regular expression, each a button that stays
  pressed while its switch is on (shown by its frame and fill, and told to
  screen readers by `aria-pressed`, not by colour alone). Alt+C, Alt+W and
  Alt+R switch them from the field (`switchKey`); a click keeps the keyboard
  in the field.
-->
<script lang="ts">
  import type { SearchSwitches } from './bindings';
  import type { SwitchName } from './search.svelte';

  type Props = {
    switches: SearchSwitches;
    onToggle: (name: SwitchName) => void;
  };

  let { switches, onToggle }: Props = $props();

  const SWITCHES: { name: SwitchName; label: string; key: string; glyph: string }[] = [
    { name: 'matchCase', label: 'Match case', key: 'C', glyph: 'Aa' },
    { name: 'wholeWord', label: 'Whole word', key: 'W', glyph: 'ab' },
    { name: 'regex', label: 'Regular expression', key: 'R', glyph: '.*' },
  ];
</script>

<div class="switches" role="group" aria-label="Search switches">
  {#each SWITCHES as each (each.name)}
    <button
      type="button"
      class="switch {each.name}"
      aria-pressed={switches[each.name]}
      aria-label={each.label}
      aria-keyshortcuts="Alt+{each.key}"
      title="{each.label} (Alt+{each.key})"
      onmousedown={(event) => event.preventDefault()}
      onclick={() => onToggle(each.name)}><span aria-hidden="true">{each.glyph}</span></button
    >
  {/each}
</div>

<style>
  .switches {
    display: inline-flex;
    flex: 0 0 auto;
    gap: 2px;
  }

  .switch {
    box-sizing: border-box;
    min-width: 26px;
    min-height: 24px;
    padding: 0 var(--space-1);
    font: inherit;
    font-family: var(--font-mono);
    font-size: 0.85em;
    color: var(--ui-hint);
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius);
    cursor: pointer;
  }

  /* "ab" underlined as a whole word stands on its own. */
  .wholeWord span {
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .switch:hover {
    color: var(--ui-text);
    border-color: var(--ui-border);
  }

  /* On: framed and filled, the glyph in the text colour. */
  .switch[aria-pressed='true'] {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
    border-color: var(--ui-focus);
  }

  .switch:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: 1px;
  }

  @media (forced-colors: active) {
    .switch[aria-pressed='true'] {
      border-color: Highlight;
      outline: 1px solid Highlight;
    }
  }
</style>
