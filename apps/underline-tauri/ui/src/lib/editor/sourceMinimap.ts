// M-12: the minimap's view of the source view (CodeMirror): its lines laid
// out by `geometryOf`, its marked hits, its scrolling.

import { foldedLines, headingLines } from './folding';
import { geometryOf, hitsOf, visibleOf, type MinimapSource } from './minimap';
import type { SourceView } from './sourceView';

/** What the minimap shows of `source`. */
export function sourceMinimap(source: SourceView): MinimapSource {
  const view = source.view;
  return {
    geometry: () => geometryOf(view, headingLines(view), foldedLines(view)),
    hits: () => hitsOf(view, source.hits()),
    visible: () => visibleOf(view),
    contentHeight: () => view.scrollDOM.scrollHeight,
    scrollTo: (offset) => {
      view.scrollDOM.scrollTop = offset;
    },
    subscribe: (listener) => source.subscribe(listener),
  };
}
