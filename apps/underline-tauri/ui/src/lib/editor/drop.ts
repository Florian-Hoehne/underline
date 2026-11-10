// M-10.5, UC-005 A9 to A13: files dragged onto the window. Tauri reports
// them with the paths the system gives (`onDragDropEvent`, no plugin); the
// webview's own drop events are refused by both views. Outside Tauri (the
// dev server's mock backend, the tests) the mock delivers them
// (`mockDrop`, `window.__ulMock.drop`).

/** What happens with files dragged over the window. */
export type DropEvent =
  /** Files are dragged into the window. */
  | { type: 'enter' }
  /** They left it without a drop. */
  | { type: 'leave' }
  /** They were dropped at `x`, `y` (CSS pixels of the window). */
  | { type: 'drop'; paths: string[]; x: number; y: number };

type Handler = (event: DropEvent) => void;

const mockHandlers = new Set<Handler>();
let mocked = false;

/** The mock backend delivers the drops from now on. */
export function useMockDrops(): void {
  mocked = true;
}

/** The mock backend: `event` happens, as Tauri would report it. */
export function mockDrop(event: DropEvent): void {
  for (const handler of [...mockHandlers]) {
    handler(event);
  }
}

/** Whether this page runs in a Tauri webview that reports drops. */
function inWebview(): boolean {
  const internals = (
    globalThis as { __TAURI_INTERNALS__?: { metadata?: { currentWebview?: unknown } } }
  ).__TAURI_INTERNALS__;
  return !mocked && internals?.metadata?.currentWebview !== undefined;
}

/** Calls `handler` for every drag and drop of files; returns the way to stop. */
export async function listenDrops(handler: Handler): Promise<() => void> {
  if (!inWebview()) {
    mockHandlers.add(handler);
    return () => mockHandlers.delete(handler);
  }
  const { getCurrentWebview } = await import('@tauri-apps/api/webview');
  const unlisten = await getCurrentWebview().onDragDropEvent((event) => {
    const payload = event.payload;
    if (payload.type === 'enter') {
      handler({ type: 'enter' });
    } else if (payload.type === 'leave') {
      handler({ type: 'leave' });
    } else if (payload.type === 'drop') {
      // Tauri gives the place in physical pixels of the webview (Q-49).
      const ratio = window.devicePixelRatio || 1;
      handler({
        type: 'drop',
        paths: payload.paths,
        x: payload.position.x / ratio,
        y: payload.position.y / ratio,
      });
    }
  });
  return unlisten;
}
