// Feature F10: a stand-in for the settings of the Rust backend (UC-010),
// so the settings window can be tried in a plain browser and in Vitest.
//
// The user settings file and one settings file per fake workspace are kept
// as maps of their keys; the settings in effect, the rows of the settings
// window and where a change goes follow `SettingsState` and
// `settings/window.rs` roughly. The real rules are tested in Rust.
//
// For trying the edge cases, the mock's hooks (`window.__ulMock`) change a
// settings file as another program would (A5), make one unreadable (A6) or
// unwritable (A7). `/home/user/Work` is a git repository that shares its
// settings file (BR-007).

import type {
  PaletteView,
  Scope,
  SettingChoiceView,
  SettingControlView,
  SettingName,
  SettingRowView,
  SettingsView,
  SettingsWindowView,
} from './bindings';

/** A settings file: the keys it sets, as written in it. */
export type MockSettingsFile = Partial<Record<SettingName, string>>;

/** The file a hook speaks of: the user's, or the open workspace's. */
export type MockSettingsPlace = 'user' | 'workspace';

/** The fake workspaces whose settings file git shares (BR-007). */
export const SHARED_WORKSPACES: readonly string[] = ['/home/user/Work'];

/** Where the mock keeps the user settings file. */
export const MOCK_USER_SETTINGS = '/home/user/underline.yaml';

const THEMES_FALLBACK = ['Light'];
const INPUT_MODES = ['Normal', 'Vim', 'Nano'];
const SIDES = ['Left', 'Right'];
const ACCESS = ['Off', 'Read only', 'Read and write'];
const FORMATS = ['Markdown', 'AsciiDoc'];

/** The rows in the order of `SettingKey::ALL`, with their names as in egui. */
const ROWS: readonly { key: SettingName; label: string }[] = [
  { key: 'default_format', label: 'Default format' },
  { key: 'theme', label: 'Theme' },
  { key: 'input_mode', label: 'Input mode' },
  { key: 'show_line_numbers', label: 'Line numbers' },
  { key: 'line_number_side', label: 'Line number side' },
  { key: 'show_minimap', label: 'Minimap' },
  { key: 'copy_on_select', label: 'Copy on select' },
  { key: 'git_sync_interval', label: 'Git sync every (minutes)' },
  { key: 'agent_access', label: 'Agent access' },
  { key: 'overview_width', label: 'Note overview width' },
  { key: 'tool_panel_width', label: 'Tool panel width' },
  { key: 'show_menu_bar', label: 'Menu bar' },
  { key: 'blink_found_in_minimap', label: 'Search hits in the minimap' },
];

const CHECK_TEXTS: Partial<Record<SettingName, string>> = {
  show_line_numbers: 'Shown, in the source view',
  show_minimap: 'Shown',
  copy_on_select: 'On',
  show_menu_bar: 'Shown',
  blink_found_in_minimap: 'Blinking',
};

const RANGES: Partial<Record<SettingName, [number, number]>> = {
  git_sync_interval: [1, 60],
  overview_width: [100, 1000],
  tool_panel_width: [100, 1000],
};

/** The built-in defaults (UC-010 BR-001), as written in a file. */
const DEFAULTS: Record<Exclude<SettingName, 'default_format'>, string> = {
  theme: 'Light',
  input_mode: 'Normal',
  show_line_numbers: 'true',
  line_number_side: 'Left',
  show_minimap: 'true',
  copy_on_select: 'true',
  git_sync_interval: '5',
  agent_access: 'Off',
  overview_width: '200',
  tool_panel_width: '240',
  show_menu_bar: 'false',
  blink_found_in_minimap: 'true',
};

/** `SettingsView` fields by the key they show, to seed the user file. */
const VIEW_FIELDS: Partial<Record<keyof SettingsView, SettingName>> = {
  theme: 'theme',
  inputMode: 'input_mode',
  showLineNumbers: 'show_line_numbers',
  lineNumberSide: 'line_number_side',
  showMinimap: 'show_minimap',
  copyOnSelect: 'copy_on_select',
  gitSyncInterval: 'git_sync_interval',
  agentAccess: 'agent_access',
  overviewWidth: 'overview_width',
  toolPanelWidth: 'tool_panel_width',
  showMenuBar: 'show_menu_bar',
  blinkFoundInMinimap: 'blink_found_in_minimap',
};

/** The user file that gives the settings `seed` (the mock's `MockStart.settings`). */
export function userFileOf(seed: Partial<SettingsView>): MockSettingsFile {
  const file: MockSettingsFile = {};
  for (const [field, value] of Object.entries(seed)) {
    const key = VIEW_FIELDS[field as keyof SettingsView];
    if (key !== undefined && value !== undefined && value !== null) {
      file[key] = String(value);
    }
  }
  return file;
}

/** The settings of the mock: the files, what is in effect, the window. */
export class MockSettings {
  user: MockSettingsFile;
  /** The settings file of each fake workspace, by its root. */
  readonly workspaces: Record<string, MockSettingsFile> = {};
  root: string | null = null;
  widthGeneration = 0;
  /** Grows with every change of a file, as `SettingsState::revision`. */
  revision = 0;
  /** A6: files that cannot be read now, by place. */
  readonly broken = new Set<MockSettingsPlace>();
  /** A7: the files cannot be written. */
  unwritable = false;
  /** Whether agents' access was refused last (said once, UC-017 A1). */
  #refused = false;

  constructor(
    user: MockSettingsFile,
    private readonly palettes: readonly PaletteView[],
  ) {
    this.user = { ...user };
  }

  get #workspace(): MockSettingsFile | null {
    return this.root === null ? null : (this.workspaces[this.root] ??= {});
  }

  /** The value in effect of `key` in `scope` ("For me" leaves the workspace out). */
  #value(key: Exclude<SettingName, 'default_format'>, scope: Scope = 'workspace'): string {
    const workspace = scope === 'workspace' ? this.#workspace : null;
    if (key === 'agent_access') {
      const user = this.user.agent_access ?? DEFAULTS.agent_access;
      const own = workspace?.agent_access;
      return own !== undefined && ACCESS.indexOf(own) < ACCESS.indexOf(user) ? own : user;
    }
    return workspace?.[key] ?? this.user[key] ?? DEFAULTS[key];
  }

  /** BR-008: the workspace asks for more agent access than the user allows. */
  get refused(): boolean {
    const own = this.#workspace?.agent_access;
    const user = this.user.agent_access ?? DEFAULTS.agent_access;
    return own !== undefined && ACCESS.indexOf(own) > ACCESS.indexOf(user);
  }

  /** The settings in effect, as the `settings` event carries them. */
  effective(): SettingsView {
    const theme = this.#value('theme');
    const user = this.user.agent_access ?? DEFAULTS.agent_access;
    const access = this.#value('agent_access');
    return {
      theme,
      palette: this.palettes.find((p) => p.theme === theme) ?? this.palettes[0],
      inputMode: this.#value('input_mode'),
      showLineNumbers: this.#value('show_line_numbers') === 'true',
      lineNumberSide: this.#value('line_number_side'),
      showMinimap: this.#value('show_minimap') === 'true',
      copyOnSelect: this.#value('copy_on_select') === 'true',
      gitSyncInterval: Number(this.#value('git_sync_interval')),
      agentAccess: access,
      agentAccessLowered: ACCESS.indexOf(access) < ACCESS.indexOf(user),
      agentAccessRefused: this.refused,
      overviewWidth: Number(this.#value('overview_width')),
      toolPanelWidth: Number(this.#value('tool_panel_width')),
      showMenuBar: this.#value('show_menu_bar') === 'true',
      blinkFoundInMinimap: this.#value('blink_found_in_minimap') === 'true',
      widthGeneration: this.widthGeneration,
    };
  }

  /** UC-003, A8: the format new notes get: the workspace's own, else the notes'. */
  defaultFormat(derived: string): string {
    const own = this.#workspace?.default_format;
    return own === 'asciidoc' ? 'AsciiDoc' : own === 'markdown' ? 'Markdown' : derived;
  }

  /** UC-002: the workspace at `root` opened; returns what to say, if anything. */
  openWorkspace(root: string): string | null {
    this.root = root;
    this.revision += 1;
    this.#refused = false;
    return this.#refusedNow();
  }

  #refusedNow(): string | null {
    const now = this.refused;
    const said = now && !this.#refused;
    this.#refused = now;
    return said
      ? "This workspace's settings ask for more agent access than your own settings allow; that is not followed (only your own settings can raise it)"
      : null;
  }

  /** Why `value` is not allowed for `key`, roughly as `Settings::set` says it. */
  #refusal(key: SettingName, value: string): string | null {
    const among = (names: readonly string[]) =>
      names.includes(value) ? null : `${JSON.stringify(value)} is none of ${names.join(', ')}`;
    switch (key) {
      case 'theme':
        return among(this.palettes.map((p) => p.theme).concat(THEMES_FALLBACK));
      case 'input_mode':
        return among(INPUT_MODES);
      case 'line_number_side':
        return among(SIDES);
      case 'agent_access':
        return among(ACCESS);
      case 'default_format':
        return among(['markdown', 'asciidoc']);
      case 'git_sync_interval':
      case 'overview_width':
      case 'tool_panel_width': {
        const [min, max] = RANGES[key] ?? [0, 0];
        const number = Number(value);
        if (!/^\d+$/.test(value)) {
          return `underline.yaml: ${key}: invalid type: string ${JSON.stringify(value)}, expected u32`;
        }
        return number < min || number > max
          ? `${key} is ${number}, but must be from ${min} to ${max}`
          : null;
      }
      default:
        return value === 'true' || value === 'false'
          ? null
          : `underline.yaml: ${key}: invalid type: string ${JSON.stringify(value)}, expected a boolean`;
    }
  }

  /**
   * Changes `key` in the file of `scope` (`SettingsState::change`); returns
   * the status message, if there is one.
   */
  change(scope: Scope, key: SettingName, value: string | null): string | null {
    const file = scope === 'me' ? this.user : this.#workspace;
    if (file === null) {
      return null;
    }
    if (value !== null) {
      const refusal = this.#refusal(key, value);
      if (refusal !== null) {
        return refusal;
      }
      file[key] = value;
    } else {
      delete file[key];
    }
    this.revision += 1;
    const said = this.#refusedNow();
    const place: MockSettingsPlace = scope === 'me' ? 'user' : 'workspace';
    if (this.broken.has(place)) {
      const path = scope === 'me' ? MOCK_USER_SETTINGS : `${this.root}/underline.yaml`;
      return `${path} has an error; the setting applies until Underline is closed and is stored once the file is corrected`;
    }
    if (this.unwritable) {
      return 'The setting could not be stored and applies until Underline is closed: Permission denied (os error 13)';
    }
    return said;
  }

  /** A3, A4, BR-003: a quick switch goes where the setting comes from. */
  quick(key: SettingName, value: string): string | null {
    const scope: Scope = this.#workspace?.[key] !== undefined ? 'workspace' : 'me';
    return this.change(scope, key, value);
  }

  /**
   * M-20.5: a change in the settings window (`window_change`): numbers kept
   * within their range, the value shown is no change, a width sets the
   * panels.
   */
  windowChange(scope: Scope, key: SettingName, value: string | null): string | null {
    if (scope === 'workspace' && this.#workspace === null) {
      return null;
    }
    let wanted = value === null ? null : value.trim();
    if (scope === 'workspace' && key === 'agent_access' && wanted !== null) {
      const user = this.user.agent_access ?? DEFAULTS.agent_access;
      if (ACCESS.indexOf(wanted) > ACCESS.indexOf(user)) {
        return "This workspace's settings ask for more agent access than your own settings allow; that is not followed (only your own settings can raise it)";
      }
    }
    const range = RANGES[key];
    if (wanted !== null && range && /^\d+$/.test(wanted.trim())) {
      wanted = String(Math.min(range[1], Math.max(range[0], Number(wanted.trim()))));
    }
    if (key !== 'default_format' && wanted !== null && this.#value(key, scope) === wanted) {
      return null;
    }
    const said = this.change(scope, key, wanted);
    if (key === 'overview_width' || key === 'tool_panel_width') {
      this.widthGeneration += 1;
    }
    return said;
  }

  /** A5: another program set `key` in a file; returns what to say, if anything. */
  outside(place: MockSettingsPlace, key: SettingName, value: string | null): string | null {
    const file = place === 'user' ? this.user : this.#workspace;
    if (file === null) {
      return null;
    }
    if (value === null) {
      delete file[key];
    } else {
      file[key] = value;
    }
    this.broken.delete(place);
    this.revision += 1;
    this.widthGeneration += 1;
    return this.#refusedNow();
  }

  /** M-20.2 to M-20.4: what the settings window shows (`window_view`). */
  windowView(derived: string | null): SettingsWindowView {
    const workspace = this.#workspace;
    const rows = (scope: Scope): SettingRowView[] =>
      ROWS.filter((row) => scope === 'workspace' || row.key !== 'default_format').map((row) => ({
        key: row.key,
        label: row.label,
        control: this.#control(row.key, scope, derived),
        note: this.#note(row.key, scope),
      }));
    return {
      revision: this.revision,
      workspace: workspace !== null,
      shared: this.root !== null && SHARED_WORKSPACES.includes(this.root),
      me: rows('me'),
      thisWorkspace: workspace === null ? [] : rows('workspace'),
    };
  }

  #control(key: SettingName, scope: Scope, derived: string | null): SettingControlView {
    if (key === 'default_format') {
      const own = this.#workspace?.default_format;
      const ownName = own === 'asciidoc' ? 'AsciiDoc' : own === 'markdown' ? 'Markdown' : null;
      const shown =
        ownName ?? (derived !== null ? `${derived} (from the notes)` : 'Follow the notes');
      const choices: SettingChoiceView[] = FORMATS.map((format) => ({
        label: format,
        value: format.toLowerCase(),
        selected: ownName === format,
      }));
      choices.push({ label: 'Follow the notes', value: null, selected: ownName === null });
      return { kind: 'choice', shown, choices };
    }
    const value = this.#value(key, scope);
    const text = CHECK_TEXTS[key];
    if (text !== undefined) {
      return { kind: 'check', on: value === 'true', text };
    }
    const range = RANGES[key];
    if (range !== undefined) {
      return { kind: 'number', value: Number(value), min: range[0], max: range[1] };
    }
    let names: readonly string[];
    switch (key) {
      case 'theme':
        names = this.palettes.map((p) => p.theme);
        break;
      case 'input_mode':
        names = INPUT_MODES;
        break;
      case 'line_number_side':
        names = SIDES;
        break;
      default: {
        const user = this.user.agent_access ?? DEFAULTS.agent_access;
        names = scope === 'me' ? ACCESS : ACCESS.slice(0, ACCESS.indexOf(user) + 1);
      }
    }
    return {
      kind: 'choice',
      shown: value,
      choices: names.map((name) => ({ label: name, value: name, selected: name === value })),
    };
  }

  #note(key: SettingName, scope: Scope): SettingRowView['note'] {
    const workspace = this.#workspace;
    if (workspace === null) {
      return null;
    }
    const sets = workspace[key] !== undefined;
    if (scope === 'workspace') {
      if (!sets) {
        return { kind: 'text', text: 'as for me' };
      }
      return key === 'default_format'
        ? { kind: 'text', text: 'own' }
        : { kind: 'asForMe', label: 'As for me', hint: 'Use the setting from "For me" again' };
    }
    if (key === 'agent_access' && this.refused) {
      return { kind: 'text', text: 'this workspace asks for more; not followed' };
    }
    return sets ? { kind: 'text', text: 'this workspace uses its own value' } : null;
  }
}
