import type { WindowWorkspaceRect } from './window-workspace-state';

export const WINDOW_WORKSPACE_MARGIN = 8;
export const WINDOW_WORKSPACE_MIN_WIDTH = 360;
export const WINDOW_WORKSPACE_MIN_HEIGHT = 320;
export const WINDOW_WORKSPACE_VISIBLE_TITLE_HEIGHT = 48;

export interface WindowWorkspaceViewport {
  left: number;
  top: number;
  width: number;
  height: number;
}

export function clampWindowWorkspaceRect(
  rect: WindowWorkspaceRect,
  minWidth = WINDOW_WORKSPACE_MIN_WIDTH,
  minHeight = WINDOW_WORKSPACE_MIN_HEIGHT,
  viewport = getWindowWorkspaceViewport()
): WindowWorkspaceRect {
  const width = clamp(
    rect.width,
    minWidth,
    Math.max(minWidth, viewport.width - 16)
  );
  const height = clamp(
    rect.height,
    minHeight,
    Math.max(minHeight, viewport.height - 16)
  );
  return {
    left: clamp(
      rect.left,
      viewport.left + WINDOW_WORKSPACE_MARGIN,
      Math.max(
        viewport.left + WINDOW_WORKSPACE_MARGIN,
        viewport.left + viewport.width - width - WINDOW_WORKSPACE_MARGIN
      )
    ),
    top: clamp(
      rect.top,
      viewport.top + WINDOW_WORKSPACE_MARGIN,
      Math.max(
        viewport.top + WINDOW_WORKSPACE_MARGIN,
        viewport.top + viewport.height - WINDOW_WORKSPACE_VISIBLE_TITLE_HEIGHT
      )
    ),
    width,
    height
  };
}

export function fitWindowWorkspaceRect(
  rect: WindowWorkspaceRect,
  minWidth = WINDOW_WORKSPACE_MIN_WIDTH,
  minHeight = WINDOW_WORKSPACE_MIN_HEIGHT,
  viewport = getWindowWorkspaceViewport()
): WindowWorkspaceRect {
  const availableWidth = Math.max(0, viewport.width - 2 * WINDOW_WORKSPACE_MARGIN);
  const availableHeight = Math.max(
    0,
    viewport.height - 2 * WINDOW_WORKSPACE_MARGIN
  );
  const fittedMinWidth = Math.min(minWidth, availableWidth);
  const fittedMinHeight = Math.min(minHeight, availableHeight);
  const width = clamp(rect.width, fittedMinWidth, availableWidth);
  const left = clamp(
    rect.left,
    viewport.left + WINDOW_WORKSPACE_MARGIN,
    viewport.left + viewport.width - WINDOW_WORKSPACE_MARGIN - width
  );
  const top = clamp(
    rect.top,
    viewport.top + WINDOW_WORKSPACE_MARGIN,
    viewport.top + viewport.height - WINDOW_WORKSPACE_MARGIN - fittedMinHeight
  );
  const height = clamp(
    rect.height,
    fittedMinHeight,
    viewport.top + viewport.height - WINDOW_WORKSPACE_MARGIN - top
  );
  return { left, top, width, height };
}

export function getWindowWorkspaceViewport(): WindowWorkspaceViewport {
  if (typeof window === 'undefined') {
    return { left: 0, top: 0, width: 1280, height: 720 };
  }
  const topInset = document
    .querySelector<HTMLElement>('[data-window-workspace-top-inset]')
    ?.getBoundingClientRect().bottom;
  const top = Math.max(0, topInset ?? 0);
  return {
    left: 0,
    top,
    width: window.innerWidth,
    height: Math.max(0, window.innerHeight - top)
  };
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
