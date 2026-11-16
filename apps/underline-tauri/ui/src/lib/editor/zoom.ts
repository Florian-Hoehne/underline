// UC-010 A11 (FR-087): the mouse wheel turned over a note with the Ctrl key
// held zooms the editor, one step of 10 percent per notch of the wheel. A
// touchpad sends many small turns (and a pinch comes as a wheel with Ctrl):
// they are added up until they make a notch.

/** How far the wheel turns for one step of the zoom, in pixels (a notch is about 50 to 120). */
export const WHEEL_NOTCH_PX = 50;

/** The parts of a wheel event the zoom reads. */
export type WheelLike = Pick<
  WheelEvent,
  'deltaY' | 'deltaMode' | 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey'
>;

/** Adds up the turns of the wheel with Ctrl held into steps of the zoom. */
export class WheelZoom {
  #turned = 0;

  /** Whether `event` zooms: the wheel with Ctrl held and no other modifier. */
  static zooms(event: WheelLike): boolean {
    return event.ctrlKey && !event.altKey && !event.shiftKey && !event.metaKey;
  }

  /**
   * The step `event` makes: `1` larger (the wheel turned up), `-1` smaller,
   * `0` none yet.
   */
  step(event: WheelLike): -1 | 0 | 1 {
    // `deltaMode` 1 counts lines, 2 pages, 0 pixels.
    const unit = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? 400 : 1;
    const turned = event.deltaY * unit;
    // Turning back starts anew.
    if (Math.sign(turned) !== Math.sign(this.#turned)) {
      this.#turned = 0;
    }
    this.#turned += turned;
    if (Math.abs(this.#turned) < WHEEL_NOTCH_PX) {
      return 0;
    }
    const step = this.#turned < 0 ? 1 : -1;
    this.#turned = 0;
    return step;
  }
}
