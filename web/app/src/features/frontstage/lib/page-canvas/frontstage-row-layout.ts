import type { Layout, LayoutItem } from 'react-grid-layout';

export function allocateFrontstageRowWidths(
  items: readonly LayoutItem[],
  columns: number
): number[] | null {
  const widths = items.map((item) => Math.max(1, Math.round(item.minW ?? 1)));
  const maximums = items.map((item) =>
    Math.max(1, Math.min(columns, Math.round(item.maxW ?? columns)))
  );
  let remaining = columns - widths.reduce((sum, width) => sum + width, 0);

  if (remaining < 0) return null;

  while (remaining > 0) {
    let candidate = -1;
    for (let index = 0; index < widths.length; index += 1) {
      if (widths[index]! >= maximums[index]!) continue;
      if (candidate < 0 || widths[index]! < widths[candidate]!) {
        candidate = index;
      }
    }

    if (candidate < 0) return null;
    widths[candidate]! += 1;
    remaining -= 1;
  }

  return widths;
}

export function projectFrontstageAutomaticRow(
  items: readonly LayoutItem[],
  columns: number,
  rowY: number
): Layout | null {
  const widths = allocateFrontstageRowWidths(items, columns);
  if (!widths) return null;
  const rowHeight = Math.max(...items.map((item) => item.h));

  let nextX = 0;
  return items.map((item, index) => {
    const width = widths[index]!;
    const projected = {
      ...item,
      x: nextX,
      y: rowY,
      w: width,
      h: rowHeight,
      moved: false
    };
    nextX += width;
    return projected;
  });
}

/** Solves intrinsic row contributions into deterministic allocations and positions. */
export function solveFrontstageAutomaticLayout(
  layout: Layout,
  columns: number
): Layout {
  const rows = new Map<number, LayoutItem[]>();
  for (const item of layout) {
    const row = rows.get(item.y) ?? [];
    row.push(item);
    rows.set(item.y, row);
  }

  const projectedById = new Map<string, LayoutItem>();
  let nextY = 0;
  for (const [, row] of [...rows.entries()].sort(
    ([leftY], [rightY]) => leftY - rightY
  )) {
    const ordered = [...row].sort(
      (left, right) => left.x - right.x || left.i.localeCompare(right.i)
    );
    const isAlreadyContinuous = ordered.every(
      (item, index) =>
        item.x ===
        ordered
          .slice(0, index)
          .reduce((offset, previous) => offset + previous.w, 0)
    );
    const fillsRow =
      ordered.reduce((width, item) => width + item.w, 0) === columns;
    const rowHeight = Math.max(...ordered.map((item) => item.h));
    const projected =
      isAlreadyContinuous && fillsRow
        ? ordered.map((item) => ({
            ...item,
            y: nextY,
            h: rowHeight,
            moved: false
          }))
        : projectFrontstageAutomaticRow(ordered, columns, nextY);
    if (!projected) {
      for (const item of ordered) {
        const standalone = projectFrontstageAutomaticRow(
          [item],
          columns,
          nextY
        )?.[0] ?? { ...item, x: 0, y: nextY, w: columns, moved: false };
        projectedById.set(item.i, standalone);
        nextY += standalone.h;
      }
      continue;
    }
    for (const item of projected) projectedById.set(item.i, item);
    nextY += rowHeight;
  }

  return layout.map((item) => ({ ...(projectedById.get(item.i) ?? item) }));
}
