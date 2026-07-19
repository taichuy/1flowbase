import type {
  Layout,
  ResponsiveLayouts
} from 'react-grid-layout/legacy';

import type { FrontstageBlockRenderPlanItem } from './page-canvas/render-plan';

export const FRONTSTAGE_GRID_BREAKPOINTS = {
  lg: 1200,
  md: 996,
  sm: 768,
  xs: 480,
  xxs: 0
} as const;

export const FRONTSTAGE_GRID_COLUMNS = {
  lg: 24,
  md: 20,
  sm: 12,
  xs: 1,
  xxs: 1
} as const;

const LEGACY_FRONTSTAGE_GRID_COLUMNS = {
  lg: 12,
  md: 10,
  sm: 6,
  xs: 4,
  xxs: 2
} as const;

export const FRONTSTAGE_GRID_VERSION = 24;
export const FRONTSTAGE_GRID_ROW_HEIGHT = 32;
export const FRONTSTAGE_GRID_ROW_GAP = 12;

export type FrontstageGridBreakpoint = keyof typeof FRONTSTAGE_GRID_COLUMNS;
const DEFAULT_FRONTSTAGE_GRID_HEIGHT = 8;
export type FrontstagePersistedGridLayout = Record<
  string,
  Record<string, unknown>
>;

function finiteLayoutValue(value: unknown, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

function layoutForBreakpoint(
  item: FrontstageBlockRenderPlanItem,
  breakpoint: FrontstageGridBreakpoint,
  index: number,
  autoHeight: number | undefined
) {
  const stored = item.layout[breakpoint];
  const layout =
    typeof stored === 'object' && stored !== null && !Array.isArray(stored)
      ? (stored as Record<string, unknown>)
      : item.layout;
  const columns = FRONTSTAGE_GRID_COLUMNS[breakpoint];
  const isMobile = breakpoint === 'xs' || breakpoint === 'xxs';
  const isCurrentGrid = item.layout.gridColumns === FRONTSTAGE_GRID_VERSION;
  const horizontalScale = isCurrentGrid
    ? 1
    : columns / LEGACY_FRONTSTAGE_GRID_COLUMNS[breakpoint];
  const height =
    item.presentation.heightMode === 'fixed'
      ? item.presentation.height ?? 320
      : autoHeight;

  return {
    i: item.blockId,
    x: isMobile ? 0 : Math.round(finiteLayoutValue(layout.x, 0) * horizontalScale),
    y: finiteLayoutValue(layout.y, index * DEFAULT_FRONTSTAGE_GRID_HEIGHT),
    w: isMobile
      ? 1
      : Math.min(
          columns,
          Math.max(
            1,
            Math.round(
              finiteLayoutValue(
                layout.w,
                isCurrentGrid
                  ? columns
                  : LEGACY_FRONTSTAGE_GRID_COLUMNS[breakpoint]
              ) * horizontalScale
            )
          )
        ),
    h: height === undefined
      ? DEFAULT_FRONTSTAGE_GRID_HEIGHT
      : pixelsToFrontstageGridRows(height),
    minW: 1,
    minH: 3,
    resizeHandles:
      item.presentation.heightMode === 'fixed'
        ? (['e', 'w', 's', 'se', 'sw'] as const)
        : (['e', 'w'] as const)
  };
}

export function createFrontstageResponsiveLayouts(
  items: FrontstageBlockRenderPlanItem[],
  autoHeights: Record<string, number> = {}
): ResponsiveLayouts<FrontstageGridBreakpoint> {
  return Object.fromEntries(
    Object.keys(FRONTSTAGE_GRID_COLUMNS).map((breakpoint) => [
      breakpoint,
      items.map((item, index) =>
        layoutForBreakpoint(
          item,
          breakpoint as FrontstageGridBreakpoint,
          index,
          autoHeights[item.blockId]
        )
      )
    ])
  ) as ResponsiveLayouts<FrontstageGridBreakpoint>;
}

export function createFrontstagePersistedGridLayout(
  layouts: ResponsiveLayouts<FrontstageGridBreakpoint>
): FrontstagePersistedGridLayout {
  const byBlock: FrontstagePersistedGridLayout = {};

  for (const [breakpoint, layout] of Object.entries(layouts)) {
    for (const item of layout ?? []) {
      byBlock[item.i] ??= { gridColumns: FRONTSTAGE_GRID_VERSION };
      byBlock[item.i][breakpoint] = {
        x: item.x,
        y: item.y,
        w: item.w
      };
    }
  }

  return byBlock;
}

export function pixelsToFrontstageGridRows(height: number): number {
  return Math.max(
    3,
    Math.ceil(
      (height + FRONTSTAGE_GRID_ROW_GAP) /
        (FRONTSTAGE_GRID_ROW_HEIGHT + FRONTSTAGE_GRID_ROW_GAP)
    )
  );
}

export function frontstageGridRowsToPixels(rows: number): number {
  const normalizedRows = Math.max(3, Math.round(rows));
  return (
    normalizedRows * FRONTSTAGE_GRID_ROW_HEIGHT +
    (normalizedRows - 1) * FRONTSTAGE_GRID_ROW_GAP
  );
}

export function replaceFrontstageBreakpointLayout(
  layouts: ResponsiveLayouts<FrontstageGridBreakpoint>,
  breakpoint: FrontstageGridBreakpoint,
  layout: Layout
): ResponsiveLayouts<FrontstageGridBreakpoint> {
  return {
    ...layouts,
    [breakpoint]: layout
  };
}
