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
  test('AC-002 inserts a standalone row when the pointer targets the boundary between rows', () => {
    const committed: Layout = [
      { i: 'first', x: 0, y: 0, w: 24, h: 20 },
      { i: 'second', x: 0, y: 20, w: 24, h: 20 },
      { i: 'active', x: 0, y: 40, w: 24, h: 20 }
    ];

    const result = solveFrontstageBlockInteraction({
      committedLayout: committed,
      activeId: 'active',
      proposedPosition: { x: 0, y: 10 },
      columns: 24,
      dragIntent: {
        pointerColumn: 12,
        pointerRow: 20,
        previousProjection: null,
        deadbandColumns: 0.5
      }
    });

    expect(
      result.previewLayout.map(({ i, x, y, w }) => ({ i, x, y, w }))
    ).toEqual([
      { i: 'first', x: 0, y: 0, w: 24 },
      { i: 'second', x: 0, y: 40, w: 24 },
      { i: 'active', x: 0, y: 20, w: 24 }
    ]);
    expect(result.projection).toEqual({
      kind: 'standalone-row',
      rowIndex: 1
    });
    expectValidLayout(result.previewLayout, 24);
  });

  test('AC-002 reaches standalone positions before, between, and after rows', () => {
    const committed: Layout = [
      { i: 'first', x: 0, y: 0, w: 24, h: 20 },
      { i: 'second', x: 0, y: 20, w: 24, h: 20 },
      { i: 'active', x: 0, y: 40, w: 24, h: 20 }
    ];
    const projectAt = (pointerRow: number) =>
      solveFrontstageBlockInteraction({
        committedLayout: committed,
        activeId: 'active',
        proposedPosition: { x: 0, y: pointerRow },
        columns: 24,
        dragIntent: {
          pointerColumn: 12,
          pointerRow,
          previousProjection: null,
          deadbandColumns: 0.5
        }
      });

    expect(projectAt(0).projection).toEqual({
      kind: 'standalone-row',
      rowIndex: 0
    });
    expect(projectAt(20).projection).toEqual({
      kind: 'standalone-row',
      rowIndex: 1
    });
    expect(projectAt(40).projection).toEqual({
      kind: 'standalone-row',
      rowIndex: 2
    });
  });

  test('AC-004 keeps standalone projection inside the wider exit threshold', () => {
    const committed: Layout = [
      { i: 'first', x: 0, y: 0, w: 24, h: 20 },
      { i: 'second', x: 0, y: 20, w: 24, h: 20 },
      { i: 'active', x: 0, y: 40, w: 24, h: 20 }
    ];
    const project = (
      pointerRow: number,
      previousProjection: ReturnType<
        typeof solveFrontstageBlockInteraction
      >['projection']
    ) =>
      solveFrontstageBlockInteraction({
        committedLayout: committed,
        activeId: 'active',
        proposedPosition: { x: 0, y: pointerRow },
        columns: 24,
        dragIntent: {
          pointerColumn: 12,
          pointerRow,
          previousProjection,
          deadbandColumns: 0.5
        }
      });

    const entered = project(20, null);
    const held = project(24, entered.projection);
    const exited = project(26, held.projection);

    expect(entered.projection).toEqual({
      kind: 'standalone-row',
      rowIndex: 1
    });
    expect(held.projection).toEqual(entered.projection);
    expect(exited.projection).toEqual({
      kind: 'join-row',
      rowIndex: 1,
      cellIndex: 1
    });
  });

  test('AC-006 advances pointer row from proposed drag position during edge scroll', () => {
    const committed: Layout = [
      { i: 'first', x: 0, y: 0, w: 24, h: 20 },
      { i: 'second', x: 0, y: 20, w: 24, h: 20 },
      { i: 'active', x: 0, y: 40, w: 24, h: 20 }
    ];
    const compactor = createFrontstageInteractionCompactor('auto');
    compactor.begin(committed, 'active');
    compactor.updateDragPointer({ column: 12, row: 20 });

    const atBoundary = compactor.compact(
      [committed[0]!, committed[1]!, { ...committed[2]!, y: 10 }],
      24
    );
    const afterScrollWithoutPointerMove = compactor.compact(
      [committed[0]!, committed[1]!, { ...committed[2]!, y: 0 }],
      24
    );

    expect(atBoundary.map(({ i, x, y, w }) => ({ i, x, y, w }))).toEqual([
      { i: 'first', x: 0, y: 0, w: 24 },
      { i: 'second', x: 0, y: 40, w: 24 },
      { i: 'active', x: 0, y: 20, w: 24 }
    ]);
    expect(
      afterScrollWithoutPointerMove.map(({ i, x, y, w }) => ({ i, x, y, w }))
    ).toEqual([
      { i: 'first', x: 0, y: 0, w: 12 },
      { i: 'second', x: 0, y: 20, w: 24 },
      { i: 'active', x: 12, y: 0, w: 12 }
    ]);
    compactor.end();
  });

  test('AC-005 recomputes row prefix positions for different block heights', () => {
    const committed: Layout = [
      { i: 'short', x: 0, y: 0, w: 24, h: 10 },
      { i: 'tall', x: 0, y: 10, w: 24, h: 30 },
      { i: 'active', x: 0, y: 40, w: 24, h: 20 }
    ];

    const result = solveFrontstageBlockInteraction({
      committedLayout: committed,
      activeId: 'active',
      proposedPosition: { x: 0, y: 10 },
      columns: 24,
      dragIntent: {
        pointerColumn: 12,
        pointerRow: 10,
        previousProjection: null,
        deadbandColumns: 0.5
      }
    });

    expect(result.previewLayout.map(({ i, y, h }) => ({ i, y, h }))).toEqual([
      { i: 'short', y: 0, h: 10 },
      { i: 'tall', y: 30, h: 30 },
      { i: 'active', y: 10, h: 20 }
    ]);
    expectValidLayout(result.previewLayout, 24);
  });

  test('AC-001 moves the trailing block into the first slot after crossing the leading block', () => {
    const committed: Layout = [
      { i: 'first', x: 0, y: 0, w: 12, h: 6 },
      { i: 'second', x: 12, y: 0, w: 12, h: 6 }
    ];

    const result = solveFrontstageBlockInteraction({
      committedLayout: committed,
      activeId: 'second',
      proposedPosition: { x: 0, y: 0 },
      columns: 24
    });

    expect(result.previewLayout.map(({ i, x }) => ({ i, x }))).toEqual([
      { i: 'first', x: 12 },
      { i: 'second', x: 0 }
    ]);
  });

  test('AC-002 reaches the first, middle, and last insertion indices across rows', () => {
    const committed: Layout = [
      { i: 'first', x: 0, y: 0, w: 12, h: 6 },
      { i: 'second', x: 12, y: 0, w: 12, h: 6 },
      { i: 'active', x: 0, y: 8, w: 24, h: 6 }
    ];
    const projectedOrder = (pointerColumn: number) =>
      solveFrontstageBlockInteraction({
        committedLayout: committed,
        activeId: 'active',
        proposedPosition: { x: 0, y: 0 },
        columns: 24,
        dragIntent: {
          pointerColumn,
          pointerRow: 3,
          previousProjection: null,
          deadbandColumns: 0.5
        }
      })
        .previewLayout.filter((item) => item.y === 0)
        .sort((left, right) => left.x - right.x)
        .map((item) => item.i);

    expect(projectedOrder(1)).toEqual(['active', 'first', 'second']);
    expect(projectedOrder(8)).toEqual(['first', 'active', 'second']);
    expect(projectedOrder(23)).toEqual(['first', 'second', 'active']);
  });

  test('AC-003 keeps the stable insertion index inside the midpoint deadband', () => {
    const committed: Layout = [
      { i: 'first', x: 0, y: 0, w: 12, h: 6 },
      { i: 'second', x: 12, y: 0, w: 12, h: 6 }
    ];
    const compactor = createFrontstageInteractionCompactor('auto');
    compactor.begin(committed, 'second');
    compactor.updateDragPointer({ column: 1, row: 3 });
    const insertedFirst = compactor.compact(
      [committed[0]!, { ...committed[1]!, x: 0 }],
      24
    );
    compactor.updateDragPointer({ column: 6.2, row: 3 });
    const heldInsideDeadband = compactor.compact(insertedFirst, 24);

    expect(
      heldInsideDeadband
        .filter((item) => item.y === 0)
        .sort((left, right) => left.x - right.x)
        .map((item) => item.i)
    ).toEqual(['second', 'first']);
    compactor.end();
  });

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
      expect.objectContaining({ i: 'second', x: 0, y: 6, w: 24 })
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
      { i: 'third', x: 0, y: 6, w: 24 }
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
      { i: 'second', x: 0, y: 6, w: 24 }
    ]);

    compactor.end();
  });
});
