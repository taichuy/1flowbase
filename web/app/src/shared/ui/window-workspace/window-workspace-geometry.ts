import type { WindowWorkspaceRect } from './window-workspace-state';

export const WINDOW_WORKSPACE_MARGIN = 8;
export const WINDOW_WORKSPACE_MIN_WIDTH = 360;
export const WINDOW_WORKSPACE_MIN_HEIGHT = 320;

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
    left: clamp(rect.left, 8, Math.max(8, viewport.width - width - 8)),
    top: clamp(rect.top, 8, Math.max(8, viewport.height - height - 8)),
    width,
    height
  };
}

export function getWindowWorkspaceViewport(): {
  width: number;
  height: number;
} {
  return typeof window === 'undefined'
    ? { width: 1280, height: 720 }
    : { width: window.innerWidth, height: window.innerHeight };
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
