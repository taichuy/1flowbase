import {
  noCompactor,
  verticalCompactor,
  type Compactor,
  type Layout,
  type LayoutItem
} from 'react-grid-layout';
import type { FrontstagePageLayoutMode } from '../page-document';
import { projectFrontstageAutomaticRow } from './frontstage-row-layout';

export type FrontstageBlockInteractionInput = {
  committedLayout: Layout;
  activeId: string;
  proposedPosition: { x: number; y: number };
  columns: number;
};

export type FrontstageBlockInteractionResult = {
  previewLayout: Layout;
  contacts: string[];
};

export type FrontstageInteractionCompactor = Compactor & {
  begin: (
    layout: Layout,
    activeId: string,
    interactionKind?: 'drag' | 'resize'
  ) => void;
  end: () => void;
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

function clampHorizontalPosition(item: LayoutItem, columns: number): number {
  return Math.max(
    0,
    Math.min(Math.round(item.x), Math.max(0, columns - item.w))
  );
}

function solveFrontstageAutomaticResize(
  committedLayout: Layout,
  proposedActive: LayoutItem,
  columns: number
): Layout {
  const active = committedLayout.find((item) => item.i === proposedActive.i);
  if (!active) return committedLayout;

  const row = committedLayout
    .filter((item) => item.y === active.y)
    .sort((left, right) => left.x - right.x || left.i.localeCompare(right.i));
  const activeIndex = row.findIndex((item) => item.i === active.i);
  const resizedFromWest = proposedActive.x !== active.x;
  const neighbor = resizedFromWest
    ? row[activeIndex - 1]
    : row[activeIndex + 1];

  if (!neighbor || proposedActive.w === active.w) {
    return committedLayout.map((item) =>
      item.i === active.i
        ? { ...item, h: proposedActive.h, moved: false }
        : { ...item, moved: false }
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

  return committedLayout.map((item) => {
    if (item.i === active.i) {
      return {
        ...item,
        x: activeX,
        w: activeWidth,
        h: proposedActive.h,
        moved: false
      };
    }
    if (item.i === neighbor.i) {
      return { ...item, x: neighborX, w: neighborWidth, moved: false };
    }
    return { ...item, moved: false };
  });
}

export function solveFrontstageBlockInteraction({
  activeId,
  columns,
  committedLayout,
  proposedPosition
}: FrontstageBlockInteractionInput): FrontstageBlockInteractionResult {
  const active = committedLayout.find((item) => item.i === activeId);
  if (!active) {
    return { previewLayout: committedLayout, contacts: [] };
  }

  const proposedActive: LayoutItem = {
    ...active,
    x: proposedPosition.x,
    y: Math.max(0, Math.round(proposedPosition.y))
  };
  if (proposedActive.x === active.x && proposedActive.y === active.y) {
    return {
      previewLayout: committedLayout.map((item) => ({ ...item, moved: false })),
      contacts: []
    };
  }
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

  const target = contacts[0];
  const targetY = target?.y ?? proposedActive.y;
  const destinationMembers = committedLayout
    .filter((item) => item.i !== activeId && item.y === targetY)
    .sort((left, right) => left.x - right.x || left.i.localeCompare(right.i));
  const insertionIndex = destinationMembers.findIndex(
    (item) => proposedActive.x + proposedActive.w / 2 < item.x + item.w / 2
  );
  destinationMembers.splice(
    insertionIndex < 0 ? destinationMembers.length : insertionIndex,
    0,
    proposedActive
  );

  const destinationProjection = projectFrontstageAutomaticRow(
    destinationMembers,
    columns,
    targetY
  );
  const sourceMembers =
    targetY === active.y
      ? []
      : committedLayout
          .filter((item) => item.i !== activeId && item.y === active.y)
          .sort(
            (left, right) => left.x - right.x || left.i.localeCompare(right.i)
          );
  const sourceProjection = sourceMembers.length
    ? projectFrontstageAutomaticRow(sourceMembers, columns, active.y)
    : [];

  if (!destinationProjection || !sourceProjection) {
    return {
      previewLayout: committedLayout.map((item) =>
        item.i === activeId
          ? {
              ...item,
              x: clampHorizontalPosition(proposedActive, columns),
              y: proposedActive.y,
              moved: false
            }
          : { ...item, moved: false }
      ),
      contacts: contacts.map((item) => item.i)
    };
  }

  const rowProjection = new Map<string, LayoutItem>();
  for (const item of [...sourceProjection, ...destinationProjection]) {
    rowProjection.set(item.i, item);
  }

  return {
    previewLayout: committedLayout.map((item) => {
      const projection = rowProjection.get(item.i);
      return projection
        ? { ...item, ...projection, moved: false }
        : { ...item, moved: false };
    }),
    contacts: contacts.map((item) => item.i)
  };
}

export function createFrontstageInteractionCompactor(
  layoutMode: FrontstagePageLayoutMode = 'auto'
): FrontstageInteractionCompactor {
  let committedLayout: Layout | null = null;
  let activeId: string | null = null;
  let interactionKind: 'drag' | 'resize' = 'drag';

  return {
    // `null` keeps RGL from pre-emptively pushing colliding items. The custom
    // compactor below owns the deterministic row projection during a session.
    type: null,
    allowOverlap: false,
    begin(layout, nextActiveId, nextInteractionKind = 'drag') {
      committedLayout = layout.map((item) => ({ ...item, moved: false }));
      activeId = nextActiveId;
      interactionKind = nextInteractionKind;
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
          columns
        );
      }

      return solveFrontstageBlockInteraction({
        committedLayout,
        activeId,
        proposedPosition: { x: proposedActive.x, y: proposedActive.y },
        columns
      }).previewLayout;
    },
    end() {
      committedLayout = null;
      activeId = null;
      interactionKind = 'drag';
    }
  };
}
