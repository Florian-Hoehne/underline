<!--
  M-18.3 (UC-020 BR-003): "Choose a day", the calendar of "Pick", as egui's
  `calendar_ui`: ◀ and ▶ step a month, the month's days with Monday first
  (Mo … Su), today in bold, and "No day". A day chosen is stored at once
  and the window closes; "×" or Escape closes it without a choice. It
  floats above the central area, not modal, like egui's window.

  New (A-182): the days are one stop of Tab; the arrow keys move among them
  (Left/Right a day, Up/Down a week), Home and End to the first and last
  day; the keyboard starts on the day set, else on today, else on the 1st.
  The day set is marked for screen readers ("chosen").

  Review F15: the arrows go on into the month before or after, PageUp and
  PageDown go a month back or on; the keyboard stays on the days. Screen
  readers hear a day as "Saturday, 21 November 2026" (new text, Q-112).
-->
<script lang="ts">
  import { tick } from 'svelte';
  import DialogWindow from './DialogWindow.svelte';
  import type { Shell, TodoCalendar } from './shell.svelte';

  type Props = { shell: Shell; calendar: TodoCalendar };

  let { shell, calendar }: Props = $props();

  const WEEKDAYS = ['Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa', 'Su'];

  const month = $derived(calendar.month);
  /** The day the field holds now. */
  const chosen = $derived.by(() => {
    const todo = shell.todos?.todos.find((t) => t.number === calendar.todo);
    return (calendar.field === 'due' ? todo?.due : todo?.scheduled) ?? null;
  });
  /** The day in the Tab sequence. */
  let focusDay = $state<string | null>(null);
  const reachable = $derived(
    month.days.find((d) => d.day === focusDay)?.day ??
      month.days.find((d) => d.day === chosen)?.day ??
      month.days.find((d) => d.today)?.day ??
      month.days[0]?.day ??
      null,
  );
  let grid = $state<HTMLElement | null>(null);

  const pad = (n: number) => String(n).padStart(2, '0');
  const dateOf = (day: string) =>
    new Date(+day.slice(0, 4), +day.slice(5, 7) - 1, +day.slice(8, 10));
  const textOf = (date: Date) =>
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;

  /** "Saturday, 21 November 2026". */
  function spoken(day: string): string {
    return dateOf(day).toLocaleDateString('en-GB', {
      weekday: 'long',
      day: 'numeric',
      month: 'long',
      year: 'numeric',
    });
  }

  /** `day` moved by `days` days, or by `months` months (the last day of a shorter month). */
  function moved(day: string, days: number, months = 0): string {
    const date = dateOf(day);
    if (months === 0) {
      date.setDate(date.getDate() + days);
      return textOf(date);
    }
    const last = new Date(date.getFullYear(), date.getMonth() + months + 1, 0).getDate();
    return textOf(
      new Date(date.getFullYear(), date.getMonth() + months, Math.min(date.getDate(), last)),
    );
  }

  /** The keyboard goes to `day`, showing its month first when it is another. */
  async function goTo(day: string) {
    if (!month.days.some((d) => d.day === day)) {
      await shell.moveTodoCalendar(day < month.month ? -1 : 1);
    }
    focusDay = day;
    await tick();
    grid?.querySelector<HTMLElement>(`[data-day="${day}"]`)?.focus();
  }

  function dayKey(event: KeyboardEvent, day: string) {
    const by: Record<string, [number, number]> = {
      ArrowLeft: [-1, 0],
      ArrowRight: [1, 0],
      ArrowUp: [-7, 0],
      ArrowDown: [7, 0],
      PageUp: [0, -1],
      PageDown: [0, 1],
    };
    const step = by[event.key];
    if (step) {
      event.preventDefault();
      void goTo(moved(day, step[0], step[1]));
    } else if (event.key === 'Home') {
      event.preventDefault();
      void goTo(month.days[0]?.day ?? day);
    } else if (event.key === 'End') {
      event.preventDefault();
      void goTo(month.days.at(-1)?.day ?? day);
    }
  }

  async function step(by: number) {
    focusDay = null;
    await shell.moveTodoCalendar(by);
  }
</script>

<DialogWindow title="Choose a day" onClose={() => shell.closeTodoCalendar()} closeButton>
  <div class="month">
    <button
      type="button"
      class="ui-button small"
      aria-label="Previous month"
      title="Previous month"
      onclick={() => void step(-1)}>◀</button
    >
    <strong class="title" aria-live="polite">{month.title}</strong>
    <button
      type="button"
      class="ui-button small"
      aria-label="Next month"
      title="Next month"
      onclick={() => void step(1)}>▶</button
    >
  </div>
  <div class="grid" role="group" aria-label={month.title} bind:this={grid}>
    {#each WEEKDAYS as name (name)}
      <span class="weekday" aria-hidden="true">{name}</span>
    {/each}
    {#each { length: month.lead }, at (at)}
      <span aria-hidden="true"></span>
    {/each}
    {#each month.days as day (day.day)}
      <button
        type="button"
        class="day"
        class:today={day.today}
        class:chosen={day.day === chosen}
        data-day={day.day}
        data-autofocus={day.day === reachable ? 'true' : undefined}
        tabindex={day.day === reachable ? 0 : -1}
        aria-label="{spoken(day.day)}{day.today ? ', today' : ''}{day.day === chosen
          ? ', chosen'
          : ''}"
        aria-keyshortcuts="ArrowLeft ArrowRight ArrowUp ArrowDown PageUp PageDown Home End"
        onkeydown={(event) => dayKey(event, day.day)}
        onclick={() => void shell.chooseTodoDay(day.day)}>{day.label}</button
      >
    {/each}
  </div>
  <button type="button" class="ui-button small" onclick={() => void shell.chooseTodoDay(null)}
    >No day</button
  >
</DialogWindow>

<style>
  .month {
    display: flex;
    gap: var(--space-2);
    align-items: center;
    justify-content: space-between;
    margin-bottom: var(--space-2);
  }

  .title {
    text-align: center;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(7, minmax(28px, auto));
    gap: 2px;
    margin-bottom: var(--space-2);
  }

  .weekday {
    color: var(--todo-weak, color-mix(in srgb, var(--ui-hint) 35%, var(--ui-text)));
    text-align: center;
  }

  /* egui's small buttons, 28 px for the pointer (WCAG 2.5.8). */
  .day {
    min-width: 28px;
    min-height: 28px;
    padding: 0;
    font: inherit;
    color: var(--ui-text);
    background: var(--ui-button);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
    cursor: pointer;
  }

  .day:hover {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
  }

  /* egui: today in bold. */
  .today {
    font-weight: 700;
  }

  .chosen {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
    box-shadow: inset 0 -2px 0 var(--ui-text);
  }

  .small {
    min-height: 24px;
    padding: 0 var(--space-2);
  }
</style>
