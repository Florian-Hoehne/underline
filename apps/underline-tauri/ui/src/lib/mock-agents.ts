// Feature F14 in the mock backend (UC-016 to UC-019): rough stand-ins for
// `Session::agents_view`, the section "Agents" (`agents_section_start`,
// `agents_section_done`) and the agent cursor; the real rules are tested in
// Rust (`underline-session/src/session/agents/tests.rs`).
//
// The agent tools' configurations are a fake per tool: found or not,
// installed for this workspace, for another place, or unreadable. No file
// of this computer is read or written. The hooks of `window.__ulMock`
// (`agentConnect`, `agentRead`, `agentChange` …) act as agents do.

import type {
  AgentAction,
  AgentButtonView,
  AgentLevelName,
  AgentMarkView,
  AgentOpenView,
  AgentToolName,
  AgentToolRowView,
  AgentsSectionView,
  AgentsView,
} from './bindings';

/** The levels in their order (`AgentAccess::ALL`). */
export const ACCESS_LEVELS: readonly { level: AgentLevelName; label: string; status: string }[] = [
  { level: 'off', label: 'Off', status: 'off' },
  { level: 'readOnly', label: 'Read only', status: 'read only' },
  { level: 'readWrite', label: 'Read and write', status: 'read and write' },
];

/** The agent tools in their order (`Tool::ALL`) with their names. */
export const AGENT_TOOLS: readonly { tool: AgentToolName; name: string }[] = [
  { tool: 'claudeCode', name: 'Claude Code' },
  { tool: 'claudeDesktop', name: 'Claude Desktop' },
  { tool: 'cursor', name: 'Cursor' },
  { tool: 'vsCode', name: 'VS Code (Copilot)' },
  { tool: 'codex', name: 'Codex' },
  { tool: 'geminiCli', name: 'Gemini CLI' },
];

/** How a fake tool stands. */
export type MockToolState = 'notFound' | 'notInstalled' | 'installed' | 'otherPlace' | 'unreadable';

/** An agent connected to the fake workspace. */
export type MockAgent = { name: string; refused: boolean };

/** The fake program MCP install writes into the configurations. */
export const MOCK_PROGRAM = '/home/user/.local/bin/underline';

/** The levels of `label` ("Off", "Read only", "Read and write"). */
export function levelOf(label: string): number {
  return Math.max(
    0,
    ACCESS_LEVELS.findIndex((l) => l.label === label),
  );
}

/** "underline-" and the folder name, as `base_name`. */
function entryName(root: string): string {
  const folder = root.split('/').filter(Boolean).at(-1) ?? 'workspace';
  return `underline-${folder.replace(/[^\p{L}\p{N}_-]/gu, '-')}`;
}

function folderName(root: string): string {
  return root.split('/').filter(Boolean).at(-1) ?? '';
}

function button(label: string, action: AgentAction, enabled = true): AgentButtonView {
  return { label, enabled, action };
}

/** The agents of the fake workspace and the section "Agents". */
export class MockAgents {
  connected: MockAgent[] = [];
  /** M-17.6: the cursor of each agent: note, lines from 1, written, until when (ms). */
  readonly cursors = new Map<
    string,
    { note: string; lines: [number, number]; wrote: boolean; until: number }
  >();
  /** UC-019 BR-010: the report of the notes not open an agent changed. */
  report: { status: string; agent: string; items: { verb: string; note: string }[] } | null = null;
  /** The fake tools: Claude Code and Cursor are there, the others not. */
  readonly tools: Record<AgentToolName, MockToolState> = {
    claudeCode: 'notInstalled',
    claudeDesktop: 'notFound',
    cursor: 'notInstalled',
    vsCode: 'notFound',
    codex: 'notFound',
    geminiCli: 'notFound',
  };
  /** The section's report and question. */
  sectionReport: string | null = null;
  asking: { kind: 'switchOn' | 'replace'; tool: AgentToolName } | null = null;

  /** M-04.4, M-17.1, M-17.2: as `Session::agents_view`. */
  view(
    root: string | null,
    access: { user: string; effective: string; cap: string | null; lowered: boolean },
  ): AgentsView {
    if (root === null) {
      return { menu: null, head: ['Open a workspace to connect agents to it.'], workspace: false };
    }
    const effective = levelOf(access.effective);
    const reads = effective >= 1;
    let text = `Agents: ${ACCESS_LEVELS[effective].status}`;
    let asked: string | null = null;
    if (access.lowered) {
      text += ' in this workspace';
    }
    if (reads && this.connected.length > 0) {
      const lead = this.connected.length === 1 ? 'Agent connected' : 'Agents connected';
      text += ` · ${lead}: ${this.connected.map((a) => a.name).join(', ')}`;
    } else if (!reads) {
      const names = this.connected.filter((a) => a.refused).map((a) => a.name);
      if (names.length > 0) {
        asked = `${names.join(', ')} asked for access`;
      }
    }
    const cap = access.cap === null ? 2 : levelOf(access.cap);
    const user = levelOf(access.user);
    const line = [
      'Agent access is off; agents reach the notes only while it is on.',
      'Agent access is read only.',
      'Agent access is read and write.',
    ][effective];
    return {
      menu: {
        text,
        asked,
        hover: "What agents may do with the notes through Underline's MCP server",
        levels: ACCESS_LEVELS.slice(0, cap + 1).map((l, at) => ({
          level: l.level,
          label: l.label,
          chosen: at === user,
        })),
        workspaceSettings: 'Settings of this workspace…',
      },
      head: [`Let an agent tool reach the notes of ${root} through Underline's MCP server.`, line],
      workspace: true,
    };
  }

  /** M-17.6: the cursors in `notes` (open) at `now`, as access allows. */
  marks(notes: readonly string[], effective: string, now: number) {
    const level = levelOf(effective);
    const names = new Set(this.connected.map((a) => a.name));
    for (const [name, cursor] of this.cursors) {
      if (!names.has(name) || cursor.until < now) {
        this.cursors.delete(name);
      }
    }
    return notes
      .map((note) => ({
        note,
        marks: [...this.cursors]
          .filter(([, c]) => c.note === note && (c.wrote ? level >= 2 : level >= 1))
          .map(([name, c]): AgentMarkView => ({
            from: Math.max(0, c.lines[0] - 1),
            to: Math.max(c.lines[1], c.lines[0]),
            name,
            wrote: c.wrote,
          }))
          .sort((a, b) => a.from - b.from),
      }))
      .filter((view) => view.marks.length > 0);
  }

  /** UC-019 BR-010: the report after `agent` changed `note` (`verb`); its text. */
  reportChange(agent: string, verb: 'created' | 'changed', note: string, status: string): string {
    let items =
      this.report && this.report.status === status && this.report.agent === agent
        ? this.report.items.filter((i) => i.note !== note)
        : [];
    items = [...items, { verb, note }];
    const title = (n: string) => (n.split('/').at(-1) ?? n).replace(/-/g, ' ');
    const parts = (['created', 'changed'] as const)
      .map((v) => {
        const notes = items.filter((i) => i.verb === v).map((i) => title(i.note));
        return notes.length > 0 ? `${v} ${notes.join(', ')}` : null;
      })
      .filter((p) => p !== null);
    const text = `${agent} ${parts.join(' and ')}`;
    this.report = { status: text, agent, items };
    return text;
  }

  /** M-04.9: "Open" for the notes the report names while it shows. */
  opens(status: string): AgentOpenView[] {
    const report = this.report;
    if (!report || report.status !== status) {
      return [];
    }
    const title = (n: string) => (n.split('/').at(-1) ?? n).replace(/-/g, ' ');
    return [...report.items]
      .reverse()
      .slice(0, 3)
      .map((item) => ({
        note: item.note,
        created: item.verb === 'created',
        label: report.items.length === 1 ? 'Open' : `Open ${title(item.note)}`,
      }));
  }

  /** M-17.2 to M-17.5: the section as checked last. */
  section(root: string | null): AgentsSectionView {
    if (root === null) {
      return {
        tools: [],
        question: null,
        report: null,
        copy: null,
        copyHover: 'For any other agent tool',
      };
    }
    const entry = entryName(root);
    const tools = AGENT_TOOLS.map(({ tool, name }): AgentToolRowView => {
      const row = { tool, name, weak: false, warn: false, hover: null };
      const install = (enabled: boolean) =>
        button('Install', { kind: 'install', tool, replace: false }, enabled);
      switch (this.tools[tool]) {
        case 'notFound':
          return { ...row, state: 'not found', weak: true, button: install(false) };
        case 'notInstalled':
          return { ...row, state: 'not installed', button: install(true) };
        case 'installed':
          return {
            ...row,
            state: `installed as ${entry}`,
            button: button('Remove', { kind: 'remove', tool }),
          };
        case 'otherPlace':
          return {
            ...row,
            state: `${entry}: installed for another place`,
            warn: true,
            button: install(true),
          };
        case 'unreadable':
          return {
            ...row,
            state: 'cannot be read',
            warn: true,
            hover: `/home/user/.config/${tool}.json cannot be read: expected value at line 1 column 3`,
            button: null,
          };
      }
    });
    const asking = this.asking;
    const question =
      asking === null
        ? null
        : asking.kind === 'switchOn'
          ? {
              text: 'Agents reach the notes only while agent access is on. Allow agents to read, or to read and write, for you?',
              buttons: [
                button('Allow reading and install', {
                  kind: 'allowAndInstall',
                  tool: asking.tool,
                  level: 'readOnly',
                }),
                button('Allow reading and writing and install', {
                  kind: 'allowAndInstall',
                  tool: asking.tool,
                  level: 'readWrite',
                }),
                button('Install only', { kind: 'installOnly', tool: asking.tool }),
                button('Cancel', { kind: 'cancel' }),
              ],
            }
          : {
              // As the backend: A2's question stands above its buttons.
              text: this.sectionReport,
              buttons: [
                button('Replace', { kind: 'install', tool: asking.tool, replace: true }),
                button('Cancel', { kind: 'cancel' }),
              ],
            };
    return {
      tools,
      question,
      report: asking?.kind === 'replace' ? null : this.sectionReport,
      copy: button('Copy configuration', { kind: 'copy' }),
      copyHover: 'For any other agent tool',
    };
  }

  /**
   * UC-016 steps 3–5, A1–A4: `action` in the section of `root`, where the
   * user's access is `user` and the access in effect reads (`reads`).
   * Returns the level to set first (A1) and the configuration copied (A4).
   */
  act(
    action: AgentAction,
    root: string,
    user: string,
    reads: () => boolean,
  ): { level: AgentLevelName | null; copied: string | null } {
    this.asking = null;
    const name = (tool: AgentToolName) => AGENT_TOOLS.find((t) => t.tool === tool)?.name ?? tool;
    const folder = folderName(root);
    const install = (tool: AgentToolName, replace: boolean) => {
      const state = this.tools[tool];
      if (state === 'unreadable') {
        this.sectionReport = `${name(tool)} was not changed: /home/user/.config/${tool}.json cannot be read: expected value at line 1 column 3`;
        return;
      }
      if (state === 'otherPlace' && !replace) {
        this.asking = { kind: 'replace', tool };
        this.sectionReport = 'An entry for this workspace starts another Underline. Replace it?';
        return;
      }
      this.tools[tool] = 'installed';
      this.sectionReport = `Installed ${folder} in ${name(tool)} as ${entryName(root)}; restart ${name(tool)} if it does not show it yet.`;
      if (!reads()) {
        this.sectionReport += ' Agent access is off, so the agent cannot reach the notes yet.';
      }
    };
    switch (action.kind) {
      case 'cancel':
        // As the backend: the question goes with the report that asked it.
        this.sectionReport = null;
        return { level: null, copied: null };
      case 'copy':
        this.sectionReport = `Copied the configuration of ${folder}. Its command: ${MOCK_PROGRAM} mcp ${root}`;
        return {
          level: null,
          copied: JSON.stringify(
            { mcpServers: { [entryName(root)]: { command: MOCK_PROGRAM, args: ['mcp', root] } } },
            null,
            2,
          ),
        };
      case 'remove':
        this.tools[action.tool] = 'notInstalled';
        this.sectionReport = `Removed ${folder} from ${name(action.tool)}.`;
        return { level: null, copied: null };
      case 'allowAndInstall':
        return { level: action.level, copied: null };
      case 'install':
        if (!action.replace && levelOf(user) === 0) {
          this.asking = { kind: 'switchOn', tool: action.tool };
          this.sectionReport = null;
          return { level: null, copied: null };
        }
        install(action.tool, action.replace);
        return { level: null, copied: null };
      case 'installOnly':
        install(action.tool, false);
        return { level: null, copied: null };
    }
  }

  /** A1: after the level was set, the install of "Allow … and install". */
  installAfterAllowing(tool: AgentToolName, root: string, reads: () => boolean): void {
    this.act({ kind: 'installOnly', tool }, root, 'Read only', reads);
  }

  /** Another workspace: nothing of the agents of the one left is kept. */
  leave(): void {
    this.connected = [];
    this.cursors.clear();
    this.asking = null;
    this.sectionReport = null;
  }
}
