import type {
  Layout,
  ResponsiveLayouts
} from 'react-grid-layout/legacy';

import type { FrontstageBlockRenderPlanItem } from './page-canvas/render-plan';
import { normalizeFrontstageAutomaticRows } from './page-canvas/frontstage-row-layout';

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
export const FRONTSTAGE_VERTICAL_GRID_VERSION = 2;
// RGL applies vertical margin between every grid row. Keep its internal margin
// at zero so row precision and the product-level block gap remain independent.
export const FRONTSTAGE_GRID_ROW_HEIGHT = 3;
export const FRONTSTAGE_GRID_ROW_GAP = 10;
export const FRONTSTAGE_GRID_VERTICAL_MARGIN = 0;

const LEGACY_FRONTSTAGE_GRID_ROW_HEIGHT = 32;
const LEGACY_FRONTSTAGE_GRID_ROW_GAP = 12;

export type FrontstageGridBreakpoint = keyof typeof FRONTSTAGE_GRID_COLUMNS;
const DEFAULT_FRONTSTAGE_GRID_HEIGHT_PX = 320;
const MIN_FRONTSTAGE_GRID_HEIGHT_PX = 120;
export type FrontstagePersistedGridLayout = Record<
  string,
  Record<string, unknown>
>;

function finiteLayoutValue(value: unknown, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

function layoutForBreakpoint(
  item: Pick<
    FrontstageBlockRenderPlanItem,
    'blockId' | 'layout' | 'presentation'
  >,
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
  const isCurrentVerticalGrid =
    item.layout.verticalGridVersion === FRONTSTAGE_VERTICAL_GRID_VERSION;
  const horizontalScale = isCurrentGrid
    ? 1
    : columns / LEGACY_FRONTSTAGE_GRID_COLUMNS[breakpoint];
  const height =
    item.presentation.heightMode === 'fixed'
      ? item.presentation.height ?? 320
      : autoHeight;
  const storedY =
    typeof layout.y === 'number' && Number.isFinite(layout.y)
      ? layout.y
      : null;
  const y =
    storedY === null
      ? index * pixelsToFrontstageGridRows(DEFAULT_FRONTSTAGE_GRID_HEIGHT_PX)
      : isCurrentVerticalGrid
        ? storedY
        : Math.round(
            (storedY *
              (LEGACY_FRONTSTAGE_GRID_ROW_HEIGHT +
                LEGACY_FRONTSTAGE_GRID_ROW_GAP)) /
              FRONTSTAGE_GRID_ROW_HEIGHT
          );

  return {
    i: item.blockId,
    x: isMobile ? 0 : Math.round(finiteLayoutValue(layout.x, 0) * horizontalScale),
    y,
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
      ? pixelsToFrontstageGridRows(DEFAULT_FRONTSTAGE_GRID_HEIGHT_PX)
      : pixelsToFrontstageGridRows(height),
    minW: 1,
    minH: pixelsToFrontstageGridRows(MIN_FRONTSTAGE_GRID_HEIGHT_PX),
    resizeHandles:
      item.presentation.heightMode === 'fixed'
        ? (['e', 'w', 's', 'se', 'sw'] as const)
        : (['e', 'w'] as const)
  };
}

export function createFrontstageResponsiveLayouts(
  items: Array<
    Pick<FrontstageBlockRenderPlanItem, 'blockId' | 'layout' | 'presentation'>
  >,
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

export function normalizeFrontstageAutomaticResponsiveLayouts(
  layouts: ResponsiveLayouts<FrontstageGridBreakpoint>
): ResponsiveLayouts<FrontstageGridBreakpoint> {
  return Object.fromEntries(
    Object.entries(layouts).map(([breakpoint, layout]) => [
      breakpoint,
      normalizeFrontstageAutomaticRows(
        layout ?? [],
        FRONTSTAGE_GRID_COLUMNS[breakpoint as FrontstageGridBreakpoint]
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
      byBlock[item.i] ??= {
        gridColumns: FRONTSTAGE_GRID_VERSION,
        verticalGridVersion: FRONTSTAGE_VERTICAL_GRID_VERSION
      };
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
    1,
    Math.ceil(
      (height + FRONTSTAGE_GRID_ROW_GAP) / FRONTSTAGE_GRID_ROW_HEIGHT
    )
  );
}

export function frontstageGridRowsToPixels(rows: number): number {
  const normalizedRows = Math.max(1, Math.round(rows));
  return Math.max(
    0,
    normalizedRows * FRONTSTAGE_GRID_ROW_HEIGHT - FRONTSTAGE_GRID_ROW_GAP
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
