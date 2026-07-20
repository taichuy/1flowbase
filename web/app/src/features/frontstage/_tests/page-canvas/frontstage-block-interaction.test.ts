import { describe, expect, test } from 'vitest';
import type { Layout } from 'react-grid-layout';

import {
  createFrontstageInteractionCompactor,
  frontstageLayoutsCollide,
  frontstageLayoutsEqualForCommit,
  solveFrontstageBlockInteraction
} from '../../lib/page-canvas/frontstage-block-interaction';

function expectValidLayout(layout: Layout, columns: number) {
  for (const item of layout) {
    expect(item.x).toBeGreaterThanOrEqual(0);
    expect(item.w).toBeGreaterThanOrEqual(item.minW ?? 1);
    expect(item.x + item.w).toBeLessThanOrEqual(columns);
  }

  for (let left = 0; left < layout.length; left += 1) {
    for (let right = left + 1; right < layout.length; right += 1) {
      expect(frontstageLayoutsCollide(layout[left]!, layout[right]!)).toBe(
        false
      );
    }
  }
}

describe('frontstage block interaction solver', () => {
  test('AC-003 identifies a restored drag as a no-op commit', () => {
    const committed: Layout = [
      { i: 'first', x: 0, y: 0, w: 12, h: 6 },
      { i: 'second', x: 12, y: 0, w: 12, h: 6 }
    ];

    expect(
      frontstageLayoutsEqualForCommit(committed, [
        { ...committed[1]!, moved: false },
        { ...committed[0]!, moved: true }
      ])
    ).toBe(true);
    expect(
      frontstageLayoutsEqualForCommit(committed, [
        committed[0]!,
        { ...committed[1]!, x: 0, y: 8 }
      ])
    ).toBe(false);
  });

  test('AC-001 projects two full-width blocks into one aligned row on direct contact', () => {
    const committed: Layout = [
      { i: 'first', x: 0, y: 0, w: 24, h: 6, minW: 1 },
      { i: 'second', x: 0, y: 8, w: 24, h: 6, minW: 1 }
    ];

    const result = solveFrontstageBlockInteraction({
      committedLayout: committed,
      activeId: 'second',
      proposedPosition: { x: 0, y: 0 },
      columns: 24
    });

    expect(result.contacts).toEqual(['first']);
    expect(result.previewLayout).toEqual([
      expect.objectContaining({ i: 'first', x: 0, y: 0, w: 12 }),
      expect.objectContaining({ i: 'second', x: 12, y: 0, w: 12 })
    ]);
    expectValidLayout(result.previewLayout, 24);
  });

  test('AC-002 distributes three flexible blocks deterministically and respects minimum widths', () => {
    const committed: Layout = [
      { i: 'first', x: 0, y: 0, w: 12, h: 6, minW: 10 },
      { i: 'second', x: 12, y: 0, w: 12, h: 6, minW: 1 },
      { i: 'third', x: 0, y: 8, w: 24, h: 6, minW: 1 }
    ];
    const input = {
      committedLayout: committed,
      activeId: 'third',
      proposedPosition: { x: 18, y: 0 },
      columns: 24
    } as const;

    const firstResult = solveFrontstageBlockInteraction(input);
    const secondResult = solveFrontstageBlockInteraction(input);

    expect(firstResult.previewLayout).toEqual(secondResult.previewLayout);
    expect(
      firstResult.previewLayout.map(({ i, x, w }) => ({ i, x, w }))
    ).toEqual([
      { i: 'first', x: 0, w: 10 },
      { i: 'second', x: 10, w: 7 },
      { i: 'third', x: 17, w: 7 }
    ]);
    expectValidLayout(firstResult.previewLayout, 24);
  });

  test('AC-003 restores committed peer sizes when the active block leaves the collision row', () => {
    const committed: Layout = [
      { i: 'first', x: 0, y: 0, w: 24, h: 6 },
      { i: 'second', x: 0, y: 8, w: 24, h: 6 }
    ];

    const result = solveFrontstageBlockInteraction({
      committedLayout: committed,
      activeId: 'second',
      proposedPosition: { x: 0, y: 16 },
      columns: 24
    });

    expect(result.contacts).toEqual([]);
    expect(result.previewLayout).toEqual([
      expect.objectContaining({ i: 'first', x: 0, y: 0, w: 24 }),
      expect.objectContaining({ i: 'second', x: 0, y: 16, w: 24 })
    ]);
    expectValidLayout(result.previewLayout, 24);
  });

  test('AC-010 reflows both the source row and an empty destination row', () => {
    const committed: Layout = [
      { i: 'first', x: 0, y: 0, w: 8, h: 6 },
      { i: 'second', x: 8, y: 0, w: 8, h: 6 },
      { i: 'third', x: 16, y: 0, w: 8, h: 6 }
    ];

    const result = solveFrontstageBlockInteraction({
      committedLayout: committed,
      activeId: 'third',
      proposedPosition: { x: 4, y: 8 },
      columns: 24
    });

    expect(
      result.previewLayout.map(({ i, x, y, w }) => ({ i, x, y, w }))
    ).toEqual([
      { i: 'first', x: 0, y: 0, w: 12 },
      { i: 'second', x: 12, y: 0, w: 12 },
      { i: 'third', x: 0, y: 8, w: 24 }
    ]);
    expectValidLayout(result.previewLayout, 24);
  });

  test('AC-011 free layout mode preserves a sparse empty-row drop', () => {
    const committed: Layout = [
      { i: 'first', x: 0, y: 0, w: 8, h: 6 },
      { i: 'second', x: 8, y: 0, w: 8, h: 6 },
      { i: 'third', x: 16, y: 0, w: 8, h: 6 }
    ];
    const compactor = createFrontstageInteractionCompactor('free');

    compactor.begin(committed, 'third');
    const result = compactor.compact(
      [committed[0]!, committed[1]!, { ...committed[2]!, x: 4, y: 8 }],
      24
    );

    expect(result.map(({ i, x, y, w }) => ({ i, x, y, w }))).toEqual([
      { i: 'first', x: 0, y: 0, w: 8 },
      { i: 'second', x: 8, y: 0, w: 8 },
      { i: 'third', x: 4, y: 8, w: 8 }
    ]);
  });

  test('AC-011 automatic layout gives a resized boundary to its adjacent block', () => {
    const committed: Layout = [
      { i: 'first', x: 0, y: 0, w: 12, h: 6, minW: 1 },
      { i: 'second', x: 12, y: 0, w: 12, h: 6, minW: 1 }
    ];
    const compactor = createFrontstageInteractionCompactor('auto');

    compactor.begin(committed, 'first', 'resize');
    const result = compactor.compact(
      [{ ...committed[0]!, w: 16 }, committed[1]!],
      24
    );

    expect(result.map(({ i, x, w }) => ({ i, x, w }))).toEqual([
      { i: 'first', x: 0, w: 16 },
      { i: 'second', x: 16, w: 8 }
    ]);
    expectValidLayout(result, 24);
  });

  test('AC-001/003 keeps one committed baseline across the live compactor session', () => {
    const committed: Layout = [
      { i: 'first', x: 0, y: 0, w: 24, h: 6 },
      { i: 'second', x: 0, y: 8, w: 24, h: 6 }
    ];
    const compactor = createFrontstageInteractionCompactor();

    compactor.begin(committed, 'second');
    const contacted = compactor.compact(
      [committed[0]!, { ...committed[1]!, x: 0, y: 0, moved: true }],
      24
    );
    expect(contacted.map(({ i, x, y, w }) => ({ i, x, y, w }))).toEqual([
      { i: 'first', x: 0, y: 0, w: 12 },
      { i: 'second', x: 12, y: 0, w: 12 }
    ]);

    const leftRow = compactor.compact(
      [contacted[0]!, { ...contacted[1]!, x: 0, y: 16, moved: true }],
      24
    );
    expect(leftRow.map(({ i, x, y, w }) => ({ i, x, y, w }))).toEqual([
      { i: 'first', x: 0, y: 0, w: 24 },
      { i: 'second', x: 0, y: 16, w: 24 }
    ]);

    compactor.end();
  });
});
