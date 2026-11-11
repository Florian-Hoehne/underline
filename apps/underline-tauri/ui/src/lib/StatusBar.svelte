<!--
  M-04.7 (FR-020): the status bar's text: the workspace's root path and the
  status message, joined by "  |  " as in the egui app. The branch, sync,
  agents and todo parts come with their features (M-04.1 to M-04.10).

  UC-007 A4 (M-14.3): after "Deleted {t}" the bar offers "Undo", as long as
  that report shows.

  UC-009 step 5, A2 (M-04.8): after "Exported {t} to {p}" the bar offers
  "Open" (the system's program) and, when blocks were only approximated,
  "Show" ("Approximated blocks"), right of the text as in egui, as long as
  that report shows. Reports of exports that ended together stand on lines
  of their own, and their buttons name the note ("Open {t}", as egui's
  "Open {title}" after agent changes, A-120). A failed export is marked
  as an error by a sign and the word "Error:" for screen readers, not by
  colour alone; while an export runs the bar is busy (review F11).

  Feature F13, left of the text as in egui (M-04.1 to M-04.3):
  - the branch button "Branch {b}" or "Branch {b} · N change(s)" (hover
    "Branches  Ctrl+Shift+B"), which opens the branch menu; when git cannot
    answer, "Changes cannot be shown: {e}", weak (UC-011 A9);
  - "1 conflict"/"N conflicts" in the conflict colour, with "Abort the sync"
    (UC-015 step 1, A5);
  - with a remote, the state of the syncs with its reason at the pointer,
    a warning marked "⚠" (not by colour alone), "N to push", "N to pull"
    and "Sync now", disabled while a sync runs or conflicts wait (UC-014
    BR-004, A10).

  Feature F14 (UC-017, UC-019):
  - M-04.4, M-17.1: after the git parts, the agents entry "Agents: {status}"
    (" in this workspace", " · Agent(s) connected: A, B", " · A asked for
    access"), hover "What agents may do with the notes through Underline's
    MCP server"; it opens a menu with the levels the workspace allows as
    choices, the user's chosen (BR-002), and "Settings of this workspace…".
    Only while a workspace is open.
  - M-04.9: after an agent created or changed notes that are not open, up
    to three buttons "Open" or "Open {title}", the newest first, right of
    the text, as long as that report shows (UC-019 BR-010).

  Feature F15 (UC-020, UC-021):
  - M-04.5: after the agents entry, "1 todo overdue" or "N todos overdue"
    in red (hover "Open the todo list"), which shows the tab "Todos" with
    the filter "Open" (UC-021 A2, BR-004).
  - M-04.6: "Undo" before the text while it reports a deleted todo
    ("Deleted todo {name}", UC-020 A5) or a todo marked in the list
    ("Marked todo {name} done/open", UC-021 step 8), as in egui.
-->
<script lang="ts">
  import type {
    AgentLevelName,
    AgentOpenView,
    AgentsMenuView,
    ExportOfferView,
    GitBarView,
    StatusOffer,
  } from './bindings';
  import ContextMenu, { type MenuEntry } from './ContextMenu.svelte';

  type Props = {
    /** The root of the open workspace, if one is open. */
    root: string | null;
    /** The status message; empty when there is none. */
    status: string;
    /** What the bar offers with the message. */
    offer?: StatusOffer | null;
    /** "Open" and "Show" for each export the message reports. */
    exports?: ExportOfferView[];
    /** The message reports a failed export. */
    error?: boolean;
    /** An export is running. */
    busy?: boolean;
    /** M-04.1 to M-04.3: the branch, the conflicts and the syncs. */
    git?: GitBarView | null;
    /** "Cmd" on macOS, "Ctrl" elsewhere. */
    command?: string;
    /** M-04.1: the branch button opens the branch menu. */
    onBranches?: () => void;
    /** M-04.2: "Abort the sync". */
    onAbort?: () => void;
    /** M-04.3: "Sync now". */
    onSyncNow?: () => void;
    /** "Undo" of a deletion. */
    onUndo?: () => void;
    /** "Open" of the export `index`. */
    onOpenExport?: (index: number) => void;
    /** "Show" of the approximated blocks of the export `index`. */
    onShowApproximated?: (index: number) => void;
    /** M-04.4: the agents entry and its menu; `null` without a workspace. */
    agents?: AgentsMenuView | null;
    /** M-04.9: "Open" for the notes an agent changed. */
    agentOpens?: AgentOpenView[];
    /** M-17.1: a level chosen in the agents menu. */
    onAgentLevel?: (level: AgentLevelName) => void;
    /** M-17.1: "Settings of this workspace…". */
    onAgentSettings?: () => void;
    /** M-04.9: "Open" of a note an agent changed. */
    onOpenAgentNote?: (note: string) => void;
    /** M-04.5: "N todos overdue"; `null` while none is. */
    overdue?: string | null;
    /** M-04.5: the overdue todos show the tab "Todos". */
    onOverdue?: () => void;
    /** M-04.6: "Undo" of a deleted or marked todo. */
    onUndoTodo?: () => void;
  };

  let {
    root,
    status,
    offer = null,
    exports = [],
    error = false,
    busy = false,
    git = null,
    command = 'Ctrl',
    onBranches,
    onAbort,
    onSyncNow,
    onUndo,
    onOpenExport,
    onShowApproximated,
    agents = null,
    agentOpens = [],
    onAgentLevel,
    onAgentSettings,
    onOpenAgentNote,
    overdue = null,
    onOverdue,
    onUndoTodo,
  }: Props = $props();

  /** M-04.6: the undo of a todo stands before the text, as in egui. */
  const undoTodo = $derived(offer === 'undoTodoDelete' || offer === 'undoTodoMark');

  /** M-17.1: where the agents menu opens, while it is open. */
  let agentsMenu = $state<{ x: number; y: number } | null>(null);
  let agentsButton = $state<HTMLButtonElement | null>(null);

  /** M-17.1: the levels as choices, then "Settings of this workspace…". */
  const agentEntries = $derived<MenuEntry[]>(
    agents === null
      ? []
      : [
          ...agents.levels.map((level) => ({
            label: level.label,
            checked: level.chosen,
            action: () => {
              if (!level.chosen) {
                onAgentLevel?.(level.level);
              }
            },
          })),
          {
            label: agents.workspaceSettings,
            separatorBefore: true,
            action: () => onAgentSettings?.(),
          },
        ],
  );

  /**
   * When the menu closed last: a click on the entry closes an open menu
   * (its pointer-down outside the menu closed it already) rather than
   * opening it again.
   */
  let agentsClosedAt = -Infinity;

  function openAgentsMenu() {
    if (agentsMenu !== null || performance.now() - agentsClosedAt < 300) {
      agentsMenu = null;
      return;
    }
    const box = agentsButton?.getBoundingClientRect();
    if (box) {
      agentsMenu = { x: box.left, y: box.top - 2 };
    }
  }

  /** The menu closed; `back` gives the keyboard back to the entry. */
  function closeAgentsMenu(back: boolean) {
    if (agentsMenu === null) {
      // Already closed (an Escape, then the focus leaving): no debounce.
      return;
    }
    agentsMenu = null;
    agentsClosedAt = back ? -Infinity : performance.now();
    if (back) {
      agentsButton?.focus();
    }
  }

  /** M-04.9: "Open" names the note when there are several (for screen readers always). */
  const agentsOffered = $derived(agentOpens.length > 0 ? '. Open available' : '');

  /** For screen readers before a failure (new text, A-118). */
  const ERROR_PREFIX = 'Error: ';

  const several = $derived(exports.length > 1);
  const opens = $derived(exports.some((e) => e.open));
  const shows = $derived(exports.some((e) => e.show));
  /** For screen readers, after the text (new, as "Undo available", A-83). */
  const offered = $derived(
    offer !== null
      ? '. Undo available'
      : opens && shows
        ? '. Open and Show available'
        : opens
          ? '. Open available'
          : shows
            ? '. Show available'
            : '',
  );

  const text = $derived([root ?? '', status].filter((part) => part !== '').join('  |  '));
  const told = $derived(offered || agentsOffered);
</script>

<footer class="status-bar" aria-busy={busy || (git?.sync?.running ?? false)}>
  {#if git?.branch}
    <button
      type="button"
      class="ui-button undo git-part"
      title="Branches  {command}+Shift+B"
      aria-keyshortcuts="{command === 'Cmd' ? 'Meta' : 'Control'}+Shift+B"
      onclick={() => onBranches?.()}>{git.branch}</button
    >
  {:else if git?.problem}
    <span class="weak git-part" title={git.problem}>{git.problem}</span>
  {/if}
  {#if git?.conflicts}
    <span class="conflicts git-part">{git.conflicts}</span>
    <button type="button" class="ui-button undo" onclick={() => onAbort?.()}>Abort the sync</button>
  {/if}
  {#if git?.sync}
    {@const sync = git.sync}
    <span class="git-part" role="status" title={sync.hover ?? undefined}
      >{#if sync.warning}<span aria-hidden="true">⚠&nbsp;</span><span class="hidden"
          >Warning:
        </span>{/if}{sync.text}{#if sync.hover}<span class="hidden">: {sync.hover}</span>{/if}</span
    >
    {#if sync.toPush}
      <span class="weak git-part">{sync.toPush}</span>
    {/if}
    {#if sync.toPull}
      <span class="weak git-part">{sync.toPull}</span>
    {/if}
    <!-- UX review F13 (new texts, Q-97): why "Sync now" cannot be chosen. -->
    <button
      type="button"
      class="ui-button undo"
      disabled={!sync.canSync}
      title={sync.running
        ? 'A sync is running'
        : sync.canSync
          ? undefined
          : 'Resolve the conflicts first'}
      onclick={() => onSyncNow?.()}>Sync now</button
    >
  {/if}
  {#if agents !== null}
    <button
      type="button"
      class="ui-button undo git-part agents"
      title={agents.hover}
      aria-haspopup="menu"
      aria-expanded={agentsMenu !== null}
      bind:this={agentsButton}
      onclick={openAgentsMenu}
      >{#if agents.asked}<span aria-hidden="true">⚠&nbsp;</span
        >{`${agents.asked} · `}{/if}{agents.text}</button
    >
    {#if agentsMenu !== null}
      <ContextMenu
        label="Agents"
        entries={agentEntries}
        x={agentsMenu.x}
        y={agentsMenu.y}
        above
        onClose={closeAgentsMenu}
      />
    {/if}
  {/if}
  {#if overdue}
    <button type="button" class="overdue" title="Open the todo list" onclick={() => onOverdue?.()}
      >{overdue}</button
    >
  {/if}
  {#if undoTodo}
    <!-- egui: before the report, so a long report does not push it away. -->
    <button type="button" class="ui-button undo" onclick={() => onUndoTodo?.()}>Undo</button>
  {/if}
  {#if error}
    <span class="sign" aria-hidden="true">⚠</span>
  {/if}
  <p class="text" role={error ? 'alert' : 'status'} title={text}>
    {#if error}<span class="hidden">{ERROR_PREFIX}</span>{/if}{text}{#if told}<span class="hidden"
        >{told}</span
      >{/if}
  </p>
  {#if offer === 'undoDelete'}
    <button type="button" class="ui-button undo" onclick={() => onUndo?.()}>Undo</button>
  {/if}
  {#each exports as report, index (index)}
    {#if report.open}
      {@const label = several ? `Open exported file ${report.title}` : 'Open exported file'}
      <button
        type="button"
        class="ui-button undo"
        aria-label={label}
        title={label}
        onclick={() => onOpenExport?.(index)}>{several ? `Open ${report.title}` : 'Open'}</button
      >
    {/if}
    {#if report.show}
      {@const label = several
        ? `Show approximated blocks of ${report.title}`
        : 'Show approximated blocks'}
      <button
        type="button"
        class="ui-button undo"
        aria-label={label}
        title={label}
        onclick={() => onShowApproximated?.(index)}
        >{several ? `Show ${report.title}` : 'Show'}</button
      >
    {/if}
  {/each}
  {#if agentOpens.length > 0}
    <!-- App-tester F14: one line, which shrinks and scrolls rather than wraps. -->
    <span class="opens">
      {#each agentOpens as open (open.note)}
        {@const label = `Open ${open.note}, ${open.created ? 'created' : 'changed'} by an agent`}
        <button
          type="button"
          class="ui-button undo open-note"
          aria-label={label}
          title={label}
          onclick={() => onOpenAgentNote?.(open.note)}>{open.label}</button
        >
      {/each}
    </span>
  {/if}
</footer>

<style>
  .status-bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    min-height: 28px;
    padding: var(--space-1) var(--space-2);
    border-top: 1px solid var(--ui-rule);
  }

  /* UC-009 A4: the sign of a failed export, in the text colour. */
  .sign {
    flex: none;
    font-weight: 700;
  }

  /* For screen readers only (UX review F8). */
  .hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  /* egui's small button. */
  .undo {
    flex: none;
    min-height: 24px;
    padding: 0 var(--space-2);
  }

  /* M-04.1 to M-04.3: the git parts keep their width; the text wraps. */
  .git-part {
    flex: none;
    max-width: 40ch;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .weak {
    color: var(--ui-hint);
  }

  /* M-04.4, app-tester F14: the agents entry is one line that gives way
     to the report text in a small window, cut at its end (what an agent
     asked stands first, so it stays); screen readers get it whole. */
  .agents {
    flex: 0 1 auto;
    min-width: 8ch;
    max-width: min(60ch, 40vw);
  }

  /* M-04.9: the "Open" buttons on one line beside the text; it shrinks
     and scrolls sideways when the window is narrow. */
  .opens {
    display: flex;
    flex: 0 1 auto;
    gap: var(--space-2);
    min-width: 0;
    overflow-x: auto;
  }

  /* M-04.9: "Open {title}" at most 18 characters wide, cut at the end. */
  .open-note {
    max-width: 18ch;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* M-04.5: egui's small button in the error colour; without the button
     fill, which Nord's red would not stand out from (A-178), and with an
     outline in the same colour. */
  .overdue {
    flex: none;
    min-height: 24px;
    padding: 0 var(--space-2);
    font: inherit;
    color: var(--ui-overdue);
    cursor: pointer;
    background: transparent;
    border: 1px solid var(--ui-overdue);
    border-radius: var(--radius);
  }

  .overdue:hover {
    background: color-mix(in srgb, var(--ui-overdue) 15%, transparent);
  }

  /* UC-015 BR-005: egui's conflict orange, as the letter C (Q-91). */
  .conflicts {
    font-weight: 700;
    color: rgb(255 110 40);
  }

  /* UX review F13: a darker orange on light themes reaches 4.5:1. */
  :global(:root[data-scheme='light']) .conflicts {
    color: rgb(178 60 0);
  }

  .ui-button:disabled {
    color: var(--ui-hint);
    cursor: default;
  }

  .ui-button:disabled:hover {
    color: var(--ui-hint);
    background: var(--ui-button);
  }

  /* A long message wraps to two lines, cut at the end (app-tester F14: the
     editor keeps its height); the whole text is at the pointer (`title`)
     and for screen readers. */
  /* UX review F13: in a narrow window the text goes to a line of its own
     rather than getting thin. */
  .text {
    display: -webkit-box;
    flex: 1 1 16em;
    min-width: 0;
    overflow: hidden;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    margin: 0;
    /* "  |  " keeps its two spaces; a long message wraps instead of being cut. */
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
