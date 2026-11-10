// M-11.4, M-12: what the parts around the source view (or the facade) show of it: the place
// of the cursor and the status of the input mode for the status line, the
// view itself for the minimap, and a toolbar function that asks first (the
// Link and Image dialogs of feature F6). The source view fills it in.

import type { KeysStatusView, ToolName } from '../bindings';
import type { ActiveFacade } from '../facade/active';
import type { MinimapSource } from './minimap';
import type { SourceView } from './sourceView';

export class EditorInfo {
  /** "Ln 3, Col 5 · 4 chars"; empty while the text loads. */
  place = $state('');
  /** The status of the Vim or Nano keys; `null` with the usual keys. */
  keys = $state.raw<KeysStatusView | null>(null);
  /** The source view shown, once its text is in. */
  view = $state.raw<SourceView | null>(null);
  /** M-09: the facade shown, with the toolbar's functions. */
  facade = $state.raw<
    (ActiveFacade & { tool(name: ToolName): void; settled(): Promise<unknown> }) | null
  >(null);
  /** M-12: what the minimap shows: the source view or the facade. */
  minimap = $state.raw<MinimapSource | null>(null);
  /**
   * UC-005 A2: the toolbar function waiting for its dialog (feature F6
   * opens the Link and Image dialogs from here).
   */
  request = $state<ToolName | null>(null);
}
