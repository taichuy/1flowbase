import {
  clamp,
  clampWindowWorkspaceRect,
  getWindowWorkspaceViewport
} from '../../../../shared/ui/window-workspace/window-workspace-geometry';
import type { WindowWorkspaceRect } from '../../../../shared/ui/window-workspace/window-workspace-state';

export type FloatingWindowRect = WindowWorkspaceRect;

export const FLOATING_WINDOW_MARGIN = 8;
export const DEFAULT_MIN_WIDTH = 360;
export const DEFAULT_MIN_HEIGHT = 320;

const FLOATING_WINDOW_WIDTH_STORAGE_PREFIX =
  'applicationLogsFloatingWindowWidth';

export { clamp };

export function getViewportSize() {
  return getWindowWorkspaceViewport();
}

export function clampRect(
  rect: FloatingWindowRect,
  minWidth: number,
  minHeight: number
): FloatingWindowRect {
  return clampWindowWorkspaceRect(rect, minWidth, minHeight);
}

function getWidthStorageKey(testId: string) {
  return `${FLOATING_WINDOW_WIDTH_STORAGE_PREFIX}:${testId}`;
}

function readStoredWidth(testId: string) {
  if (typeof window === 'undefined') {
    return null;
  }

  const rawWidth = window.localStorage.getItem(getWidthStorageKey(testId));
  const width = rawWidth ? Number(rawWidth) : Number.NaN;

  return Number.isFinite(width) && width > 0 ? width : null;
}

export function writeStoredWidth(testId: string, width: number) {
  if (typeof window === 'undefined') {
    return;
  }

  window.localStorage.setItem(
    getWidthStorageKey(testId),
    String(Math.round(width))
  );
}

export function applyStoredWidth(
  rect: FloatingWindowRect,
  testId: string
): FloatingWindowRect {
  const storedWidth = readStoredWidth(testId);

  if (!storedWidth) {
    return rect;
  }

  const right = rect.left + rect.width;

  return {
    ...rect,
    left: right - storedWidth,
    width: storedWidth
  };
}
