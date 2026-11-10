import { afterEach, describe, expect, it, vi } from 'vitest';
import { setActiveFacade, type ActiveFacade } from '../facade/active';
import { jumpTo, takeJump } from './jump';

afterEach(() => {
  setActiveFacade(null);
  vi.restoreAllMocks();
  takeJump('');
});

describe('M-07.3 a jump to a heading', () => {
  it('goes to the view of the note shown now', () => {
    const jump = vi.fn();
    setActiveFacade({ note: 'Plan', jump } as unknown as ActiveFacade);

    jumpTo('Plan', 3, 12);

    expect(jump).toHaveBeenCalledWith(12, 3);
    expect(takeJump('Plan')).toBeNull();
  });

  it('waits for the view of a note that opens, once', () => {
    jumpTo('Plan', 2, 7);

    expect(takeJump('Plan')).toEqual({ offset: 7, rev: 2 });
    expect(takeJump('Plan')).toBeNull();
  });

  it('is dropped when another note shows first', () => {
    jumpTo('Plan', 2, 7);

    expect(takeJump('Home')).toBeNull();
    expect(takeJump('Plan')).toBeNull();
  });

  it('waits at most 3 s', () => {
    const now = vi.spyOn(performance, 'now').mockReturnValue(1000);
    jumpTo('Plan', 2, 7);

    now.mockReturnValue(4001);

    expect(takeJump('Plan')).toBeNull();
  });
});
