// M-11.4, M-12: what the parts around the source view show of it: the place
// of the cursor and the status of the input mode for the status line, the
// view itself for the minimap, and a toolbar function that asks first (the
// Link and Image dialogs of feature F6). The source view fills it in.

import type { KeysStatusView, ToolName } from '../bindings';
import type { SourceView } from './sourceView';

export class EditorInfo {
  /** "Ln 3, Col 5 · 4 chars"; empty while the text loads. */
  place = $state('');
  /** The status of the Vim or Nano keys; `null` with the usual keys. */
  keys = $state.raw<KeysStatusView | null>(null);
  /** The source view shown, once its text is in. */
  view = $state.raw<SourceView | null>(null);
  /**
   * UC-005 A2: the toolbar function waiting for its dialog (feature F6
   * opens the Link and Image dialogs from here).
   */
  request = $state<ToolName | null>(null);
}
