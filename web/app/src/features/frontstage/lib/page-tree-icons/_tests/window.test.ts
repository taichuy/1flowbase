import { describe, expect, it } from 'vitest';

import { iconWindow } from '../window';

describe('iconWindow', () => {
  it('DV-F03 renders a bounded initial window instead of the full catalog', () => {
    expect(
      iconWindow({
        itemCount: 847,
        scrollTop: 0,
        columnCount: 9,
        cellSize: 44,
        viewportHeight: 320,
        overscanRows: 2
      })
    ).toEqual({
      startRow: 0,
      endRow: 12,
      startIndex: 0,
      endIndex: 108
    });
  });

  it('DV-F03 advances the window while retaining bounded overscan', () => {
    const visible = iconWindow({
      itemCount: 847,
      scrollTop: 880,
      columnCount: 9,
      cellSize: 44,
      viewportHeight: 320,
      overscanRows: 2
    });

    expect(visible.startRow).toBe(18);
    expect(visible.endRow - visible.startRow).toBe(12);
    expect(visible.endIndex - visible.startIndex).toBe(108);
  });
});
