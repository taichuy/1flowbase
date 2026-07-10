import type {
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
  lg: 12,
  md: 10,
  sm: 6,
  xs: 4,
  xxs: 2
} as const;

type FrontstageGridBreakpoint = keyof typeof FRONTSTAGE_GRID_COLUMNS;
export type FrontstagePersistedGridLayout = Record<
  string,
  Record<string, { x: number; y: number; w: number; h: number }>
>;

function finiteLayoutValue(value: unknown, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

function layoutForBreakpoint(
  item: FrontstageBlockRenderPlanItem,
  breakpoint: FrontstageGridBreakpoint,
  index: number
) {
  const stored = item.layout[breakpoint];
  const layout =
    typeof stored === 'object' && stored !== null && !Array.isArray(stored)
      ? (stored as Record<string, unknown>)
      : item.layout;
  const columns = FRONTSTAGE_GRID_COLUMNS[breakpoint];

  return {
    i: item.blockId,
    x: finiteLayoutValue(layout.x, 0),
    y: finiteLayoutValue(layout.y, index * 4),
    w: Math.min(columns, finiteLayoutValue(layout.w, columns)),
    h: finiteLayoutValue(layout.h, 4)
  };
}

export function createFrontstageResponsiveLayouts(
  items: FrontstageBlockRenderPlanItem[]
): ResponsiveLayouts<FrontstageGridBreakpoint> {
  return Object.fromEntries(
    Object.keys(FRONTSTAGE_GRID_COLUMNS).map((breakpoint) => [
      breakpoint,
      items.map((item, index) =>
        layoutForBreakpoint(
          item,
          breakpoint as FrontstageGridBreakpoint,
          index
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
      byBlock[item.i] ??= {};
      byBlock[item.i][breakpoint] = {
        x: item.x,
        y: item.y,
        w: item.w,
        h: item.h
      };
    }
  }

  return byBlock;
}
