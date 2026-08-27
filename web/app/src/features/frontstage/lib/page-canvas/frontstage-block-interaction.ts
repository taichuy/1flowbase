import {
  noCompactor,
  verticalCompactor,
  type Compactor,
  type Layout,
  type LayoutItem
} from 'react-grid-layout';
import type { FrontstagePageLayoutMode } from '../page-document';
import {
  normalizeFrontstageAutomaticRows,
  projectFrontstageAutomaticRow
} from './frontstage-row-layout';

export type FrontstageBlockInteractionInput = {
  committedLayout: Layout;
  activeId: string;
  proposedPosition: { x: number; y: number };
  columns: number;
  requiredRowsByBlock?: Readonly<Record<string, number>>;
  dragIntent?: FrontstageDragInsertionIntent;
};

export type FrontstageDragProjection =
  | { kind: 'join-row'; rowIndex: number; cellIndex: number }
  | { kind: 'standalone-row'; rowIndex: number };

export type FrontstageDragPointer = {
  column: number;
  row: number;
};

export type FrontstageDragInsertionIntent = {
  pointerColumn: number;
  pointerRow: number;
  previousProjection: FrontstageDragProjection | null;
  deadbandColumns: number;
};

export type FrontstageBlockInteractionResult = {
  previewLayout: Layout;
  contacts: string[];
  projection: FrontstageDragProjection | null;
};

export type FrontstageInteractionCompactor = Compactor & {
  begin: (
    layout: Layout,
    activeId: string,
    interactionKind?: 'drag' | 'resize',
    requiredRowsByBlock?: Readonly<Record<string, number>>
  ) => void;
  updateDragPointer: (pointer: FrontstageDragPointer) => void;
  end: () => void;
};

export const FRONTSTAGE_DRAG_INSERTION_DEADBAND_COLUMNS = 0.5;
const FRONTSTAGE_ROW_BOUNDARY_ENTER_RATIO = 0.12;
const FRONTSTAGE_ROW_BOUNDARY_ENTER_MIN = 3;
const FRONTSTAGE_ROW_BOUNDARY_ENTER_MAX = 8;
const FRONTSTAGE_ROW_BOUNDARY_EXIT_DELTA = 2;

type FrontstageAutomaticRow = {
  members: Layout;
  y: number;
  height: number;
};

export function frontstageLayoutsCollide(
  left: LayoutItem,
  right: LayoutItem
): boolean {
  return !(
    left.i === right.i ||
    left.x + left.w <= right.x ||
    left.x >= right.x + right.w ||
    left.y + left.h <= right.y ||
    left.y >= right.y + right.h
  );
}

export function frontstageLayoutsEqualForCommit(
  left: Layout,
  right: Layout
): boolean {
  if (left.length !== right.length) return false;

  const rightById = new Map(right.map((item) => [item.i, item]));
  return left.every((item) => {
    const candidate = rightById.get(item.i);
    return (
      candidate !== undefined &&
      item.x === candidate.x &&
      item.y === candidate.y &&
      item.w === candidate.w &&
      item.h === candidate.h
    );
  });
}

function overlapArea(left: LayoutItem, right: LayoutItem): number {
  const width = Math.max(
    0,
    Math.min(left.x + left.w, right.x + right.w) - Math.max(left.x, right.x)
  );
  const height = Math.max(
    0,
    Math.min(left.y + left.h, right.y + right.h) - Math.max(left.y, right.y)
  );
  return width * height;
}

function resolveFrontstageInsertionIndex({
  active,
  destinationMembers,
  dragIntent,
  proposedActive,
  rowIndex
}: {
  active: LayoutItem;
  destinationMembers: readonly LayoutItem[];
  dragIntent?: FrontstageDragInsertionIntent;
  proposedActive: LayoutItem;
  rowIndex: number;
}): number {
  const proposedCenter = proposedActive.x + proposedActive.w / 2;
  const horizontalDirection = Math.sign(proposedActive.x - active.x);

  for (let index = 0; index < destinationMembers.length; index += 1) {
    const item = destinationMembers[index]!;
    const midpoint = item.x + item.w / 2;
    if (dragIntent && Number.isFinite(dragIntent.pointerColumn)) {
      if (
        Math.abs(dragIntent.pointerColumn - midpoint) <=
        dragIntent.deadbandColumns
      ) {
        const previous = dragIntent.previousProjection;
        if (previous?.kind === 'join-row' && previous.rowIndex === rowIndex) {
          return Math.max(
            0,
            Math.min(destinationMembers.length, previous.cellIndex)
          );
        }
        return horizontalDirection < 0 || active.x < item.x ? index : index + 1;
      }
      if (dragIntent.pointerColumn < midpoint) return index;
      continue;
    }

    if (proposedCenter < midpoint) return index;
    if (proposedCenter === midpoint && horizontalDirection < 0) return index;
  }

  return destinationMembers.length;
}

function createFrontstageAutomaticRows(
  layout: Layout,
  activeId: string,
  columns: number,
  requiredRowsByBlock?: Readonly<Record<string, number>>
): FrontstageAutomaticRow[] {
  const membersByY = new Map<number, LayoutItem[]>();
  for (const item of layout) {
    if (item.i === activeId) continue;
    const members = membersByY.get(item.y) ?? [];
    members.push({
      ...item,
      h: requiredRowsByBlock?.[item.i] ?? item.h,
      moved: false
    });
    membersByY.set(item.y, members);
  }

  let nextY = 0;
  return [...membersByY.entries()]
    .sort(([leftY], [rightY]) => leftY - rightY)
    .map(([, members]) => {
      const ordered = [...members].sort(
        (left, right) => left.x - right.x || left.i.localeCompare(right.i)
      );
      const projected =
        projectFrontstageAutomaticRow(ordered, columns, nextY) ??
        ordered.map((item) => ({ ...item, y: nextY, moved: false }));
      const height = Math.max(...projected.map((item) => item.h));
      const row = { members: projected, y: nextY, height };
      nextY += height;
      return row;
    });
}

function frontstageRowBoundaryPosition(
  rows: readonly FrontstageAutomaticRow[],
  rowIndex: number
): number {
  if (rows.length === 0) return 0;
  if (rowIndex === rows.length) {
    const last = rows[rows.length - 1]!;
    return last.y + last.height;
  }
  return rows[rowIndex]!.y;
}

function frontstageRowBoundaryEnterThreshold(
  rows: readonly FrontstageAutomaticRow[],
  rowIndex: number
): number {
  const adjacentHeights = [
    rows[rowIndex - 1]?.height,
    rows[rowIndex]?.height
  ].filter((height): height is number => height !== undefined);
  const referenceHeight = Math.min(...adjacentHeights);
  return Math.min(
    referenceHeight / 4,
    Math.max(
      FRONTSTAGE_ROW_BOUNDARY_ENTER_MIN,
      Math.min(
        FRONTSTAGE_ROW_BOUNDARY_ENTER_MAX,
        referenceHeight * FRONTSTAGE_ROW_BOUNDARY_ENTER_RATIO
      )
    )
  );
}

function frontstageRowBoundaryDistance(
  pointerRow: number,
  boundary: number,
  rowIndex: number,
  rowCount: number
): number {
  if (rowIndex === 0 && pointerRow <= boundary) return 0;
  if (rowIndex === rowCount && pointerRow >= boundary) return 0;
  return Math.abs(pointerRow - boundary);
}

function resolveFrontstageDragProjection({
  active,
  dragIntent,
  proposedActive,
  rows
}: {
  active: LayoutItem;
  dragIntent?: FrontstageDragInsertionIntent;
  proposedActive: LayoutItem;
  rows: readonly FrontstageAutomaticRow[];
}): FrontstageDragProjection {
  if (rows.length === 0) return { kind: 'standalone-row', rowIndex: 0 };

  const pointerRow =
    dragIntent?.pointerRow ?? proposedActive.y + proposedActive.h / 2;
  const previous = dragIntent?.previousProjection;
  if (previous?.kind === 'standalone-row') {
    const boundary = frontstageRowBoundaryPosition(rows, previous.rowIndex);
    const exitThreshold =
      frontstageRowBoundaryEnterThreshold(rows, previous.rowIndex) +
      FRONTSTAGE_ROW_BOUNDARY_EXIT_DELTA;
    if (
      frontstageRowBoundaryDistance(
        pointerRow,
        boundary,
        previous.rowIndex,
        rows.length
      ) <= exitThreshold
    ) {
      return previous;
    }
  }

  let closestBoundaryIndex = 0;
  let closestBoundaryDistance = Infinity;
  for (let rowIndex = 0; rowIndex <= rows.length; rowIndex += 1) {
    const distance = frontstageRowBoundaryDistance(
      pointerRow,
      frontstageRowBoundaryPosition(rows, rowIndex),
      rowIndex,
      rows.length
    );
    if (distance < closestBoundaryDistance) {
      closestBoundaryIndex = rowIndex;
      closestBoundaryDistance = distance;
    }
  }
  if (
    closestBoundaryDistance <=
    frontstageRowBoundaryEnterThreshold(rows, closestBoundaryIndex)
  ) {
    return { kind: 'standalone-row', rowIndex: closestBoundaryIndex };
  }

  let closestRowIndex = 0;
  let closestRowDistance = Infinity;
  for (let rowIndex = 0; rowIndex < rows.length; rowIndex += 1) {
    const row = rows[rowIndex]!;
    const distance = Math.abs(pointerRow - (row.y + row.height / 2));
    if (distance < closestRowDistance) {
      closestRowIndex = rowIndex;
      closestRowDistance = distance;
    }
  }
  const destinationMembers = rows[closestRowIndex]!.members;
  return {
    kind: 'join-row',
    rowIndex: closestRowIndex,
    cellIndex: resolveFrontstageInsertionIndex({
      active,
      destinationMembers,
      dragIntent,
      proposedActive,
      rowIndex: closestRowIndex
    })
  };
}

function projectFrontstageAutomaticRows({
  active,
  columns,
  committedLayout,
  projection,
  rows
}: {
  active: LayoutItem;
  columns: number;
  committedLayout: Layout;
  projection: FrontstageDragProjection;
  rows: readonly FrontstageAutomaticRow[];
}): Layout {
  const projectedRows = rows.map((row) => [...row.members]);
  if (projection.kind === 'standalone-row') {
    projectedRows.splice(projection.rowIndex, 0, [
      { ...active, x: 0, w: columns, moved: false }
    ]);
  } else {
    projectedRows[projection.rowIndex]!.splice(projection.cellIndex, 0, active);
  }

  let nextY = 0;
  const projectedById = new Map<string, LayoutItem>();
  for (const row of projectedRows) {
    const rowProjection = projectFrontstageAutomaticRow(row, columns, nextY);
    if (!rowProjection) return committedLayout;
    for (const item of rowProjection) projectedById.set(item.i, item);
    nextY += Math.max(...rowProjection.map((item) => item.h));
  }

  return committedLayout.map((item) => ({
    ...item,
    ...(projectedById.get(item.i) ?? item),
    moved: false
  }));
}

function solveFrontstageAutomaticResize(
  committedLayout: Layout,
  proposedActive: LayoutItem,
  columns: number,
  requiredRowsByBlock?: Readonly<Record<string, number>>
): Layout {
  const active = committedLayout.find((item) => item.i === proposedActive.i);
  if (!active) return committedLayout;
  const activeHeight =
    proposedActive.h === active.h
      ? (requiredRowsByBlock?.[active.i] ?? active.h)
      : proposedActive.h;

  const restoreRequiredHeight = (item: LayoutItem): LayoutItem => ({
    ...item,
    h:
      item.i === active.i
        ? activeHeight
        : (requiredRowsByBlock?.[item.i] ?? item.h),
    moved: false
  });

  const row = committedLayout
    .filter((item) => item.y === active.y)
    .sort((left, right) => left.x - right.x || left.i.localeCompare(right.i));
  const activeIndex = row.findIndex((item) => item.i === active.i);
  const resizedFromWest = proposedActive.x !== active.x;
  const neighbor = resizedFromWest
    ? row[activeIndex - 1]
    : row[activeIndex + 1];

  if (!neighbor || proposedActive.w === active.w) {
    return normalizeFrontstageAutomaticRows(
      committedLayout.map(restoreRequiredHeight),
      columns
    );
  }

  const pairWidth = active.w + neighbor.w;
  const activeMin = Math.max(
    active.minW ?? 1,
    pairWidth - (neighbor.maxW ?? columns)
  );
  const activeMax = Math.min(
    active.maxW ?? columns,
    pairWidth - (neighbor.minW ?? 1)
  );
  const activeWidth = Math.max(
    activeMin,
    Math.min(Math.round(proposedActive.w), activeMax)
  );
  const neighborWidth = pairWidth - activeWidth;
  const activeX = resizedFromWest
    ? active.x + active.w - activeWidth
    : active.x;
  const neighborX = resizedFromWest ? neighbor.x : activeX + activeWidth;

  return normalizeFrontstageAutomaticRows(
    committedLayout.map((item) => {
      if (item.i === active.i) {
        return {
          ...restoreRequiredHeight(item),
          x: activeX,
          w: activeWidth
        };
      }
      if (item.i === neighbor.i) {
        return {
          ...restoreRequiredHeight(item),
          x: neighborX,
          w: neighborWidth
        };
      }
      return restoreRequiredHeight(item);
    }),
    columns
  );
}

export function solveFrontstageBlockInteraction({
  activeId,
  columns,
  committedLayout,
  requiredRowsByBlock,
  dragIntent,
  proposedPosition
}: FrontstageBlockInteractionInput): FrontstageBlockInteractionResult {
  return solveFrontstageBlockInteractionWithRows(
    {
      activeId,
      columns,
      committedLayout,
      requiredRowsByBlock,
      dragIntent,
      proposedPosition
    },
    createFrontstageAutomaticRows(
      committedLayout,
      activeId,
      columns,
      requiredRowsByBlock
    )
  );
}

function solveFrontstageBlockInteractionWithRows(
  {
    activeId,
    columns,
    committedLayout,
    requiredRowsByBlock,
    dragIntent,
    proposedPosition
  }: FrontstageBlockInteractionInput,
  rows: readonly FrontstageAutomaticRow[]
): FrontstageBlockInteractionResult {
  const active = committedLayout.find((item) => item.i === activeId);
  if (!active) {
    return { previewLayout: committedLayout, contacts: [], projection: null };
  }

  const proposedActive: LayoutItem = {
    ...active,
    h: requiredRowsByBlock?.[active.i] ?? active.h,
    x: proposedPosition.x,
    y: Math.max(0, Math.round(proposedPosition.y))
  };
  const contacts = committedLayout
    .filter(
      (item) =>
        item.i !== activeId && frontstageLayoutsCollide(proposedActive, item)
    )
    .sort((left, right) => {
      const overlapDifference =
        overlapArea(proposedActive, right) - overlapArea(proposedActive, left);
      return overlapDifference || left.x - right.x || left.y - right.y;
    });

  const projection = resolveFrontstageDragProjection({
    active,
    dragIntent,
    proposedActive,
    rows
  });

  return {
    previewLayout: projectFrontstageAutomaticRows({
      active: proposedActive,
      columns,
      committedLayout,
      projection,
      rows
    }),
    contacts: contacts.map((item) => item.i),
    projection
  };
}

export function createFrontstageInteractionCompactor(
  layoutMode: FrontstagePageLayoutMode = 'auto'
): FrontstageInteractionCompactor {
  let committedLayout: Layout | null = null;
  let activeId: string | null = null;
  let interactionKind: 'drag' | 'resize' = 'drag';
  let dragPointer: FrontstageDragPointer | null = null;
  let consumedDragPointer: FrontstageDragPointer | null = null;
  let pointerRowOffset = 0;
  let stableProjection: FrontstageDragProjection | null = null;
  let automaticRows: FrontstageAutomaticRow[] | null = null;
  let requiredRowsByBlock: Readonly<Record<string, number>> | undefined;

  return {
    // `null` keeps RGL from pre-emptively pushing colliding items. The custom
    // compactor below owns the deterministic row projection during a session.
    type: null,
    allowOverlap: false,
    begin(
      layout,
      nextActiveId,
      nextInteractionKind = 'drag',
      nextRequiredRowsByBlock
    ) {
      committedLayout = layout.map((item) => ({ ...item, moved: false }));
      activeId = nextActiveId;
      interactionKind = nextInteractionKind;
      dragPointer = null;
      consumedDragPointer = null;
      pointerRowOffset = 0;
      stableProjection = null;
      automaticRows = null;
      requiredRowsByBlock = nextRequiredRowsByBlock;
    },
    updateDragPointer(nextPointer) {
      if (
        activeId &&
        Number.isFinite(nextPointer.column) &&
        Number.isFinite(nextPointer.row)
      ) {
        dragPointer = nextPointer;
      }
    },
    compact(layout, columns) {
      if (layoutMode === 'free') {
        return noCompactor.compact(layout, columns);
      }
      if (!committedLayout || !activeId) {
        return verticalCompactor.compact(layout, columns);
      }

      const proposedActive = layout.find((item) => item.i === activeId);
      if (!proposedActive) {
        return committedLayout;
      }

      if (interactionKind === 'resize') {
        return solveFrontstageAutomaticResize(
          committedLayout,
          proposedActive,
          columns,
          requiredRowsByBlock
        );
      }

      automaticRows ??= createFrontstageAutomaticRows(
        committedLayout,
        activeId,
        columns,
        requiredRowsByBlock
      );
      if (dragPointer !== null && consumedDragPointer !== dragPointer) {
        pointerRowOffset = dragPointer.row - proposedActive.y;
        consumedDragPointer = dragPointer;
      }
      const result = solveFrontstageBlockInteractionWithRows(
        {
          committedLayout,
          activeId,
          proposedPosition: { x: proposedActive.x, y: proposedActive.y },
          columns,
          requiredRowsByBlock,
          dragIntent:
            dragPointer === null
              ? undefined
              : {
                  pointerColumn: dragPointer.column,
                  pointerRow: proposedActive.y + pointerRowOffset,
                  previousProjection: stableProjection,
                  deadbandColumns: FRONTSTAGE_DRAG_INSERTION_DEADBAND_COLUMNS
                }
        },
        automaticRows
      );
      stableProjection = result.projection;
      return result.previewLayout;
    },
    end() {
      committedLayout = null;
      activeId = null;
      interactionKind = 'drag';
      dragPointer = null;
      consumedDragPointer = null;
      pointerRowOffset = 0;
      stableProjection = null;
      automaticRows = null;
      requiredRowsByBlock = undefined;
    }
  };
}
