import {
  DEFAULT_MIN_WIDTH,
  type FloatingWindowRect
} from '../floating-window-geometry';

const FLOATING_WINDOW_TOP = 112;
const FLOATING_WINDOW_GAP = 16;
const FLOATING_WINDOW_RIGHT = 32;
const FLOATING_WINDOW_MIN_WIDTH = 360;
const FLOATING_WINDOW_MAX_HEIGHT = 720;

export function getViewportSize() {
  if (typeof window === 'undefined') {
    return { width: 1280, height: 720 };
  }

  return {
    width: window.innerWidth,
    height: window.innerHeight
  };
}

function getFloatingWindowHeight() {
  const viewport = getViewportSize();

  return Math.max(
    320,
    Math.min(
      FLOATING_WINDOW_MAX_HEIGHT,
      viewport.height - FLOATING_WINDOW_TOP - FLOATING_WINDOW_RIGHT
    )
  );
}

export function getRunDetailInitialRect() {
  const viewport = getViewportSize();

  return {
    left: viewport.width - FLOATING_WINDOW_MIN_WIDTH - FLOATING_WINDOW_RIGHT,
    top: FLOATING_WINDOW_TOP,
    width: FLOATING_WINDOW_MIN_WIDTH,
    height: getFloatingWindowHeight()
  };
}

export function getConversationLogInitialRect() {
  const runDetailRect = getRunDetailInitialRect();

  return {
    left: runDetailRect.left - FLOATING_WINDOW_MIN_WIDTH - FLOATING_WINDOW_GAP,
    top: FLOATING_WINDOW_TOP,
    width: FLOATING_WINDOW_MIN_WIDTH,
    height: getFloatingWindowHeight()
  };
}

export function getResumeTimelineInitialRect() {
  return getConversationLogInitialRect();
}

export function resolveCollision(
  rectA: FloatingWindowRect,
  rectB: FloatingWindowRect,
  viewportWidth: number,
  minWidthB: number = DEFAULT_MIN_WIDTH,
  gap: number = FLOATING_WINDOW_GAP,
  margin: number = 8
): { rectA: FloatingWindowRect; rectB: FloatingWindowRect } {
  let nextLeftB = rectA.left - rectB.width - gap;

  if (nextLeftB < margin) {
    nextLeftB = margin;
    const availableWidthB = rectA.left - margin - gap;
    let nextWidthB = rectB.width;
    if (availableWidthB < rectB.width) {
      nextWidthB = Math.max(minWidthB, availableWidthB);
    }

    const overlap = nextLeftB + nextWidthB + gap - rectA.left;
    let nextLeftA = rectA.left;
    if (overlap > 0) {
      nextLeftA = Math.min(
        viewportWidth - rectA.width - margin,
        rectA.left + overlap
      );

      const newAvailableWidthB = nextLeftA - margin - gap;
      nextWidthB = Math.max(
        minWidthB,
        Math.min(rectB.width, newAvailableWidthB)
      );
      nextLeftB = Math.max(margin, nextLeftA - nextWidthB - gap);
    }

    return {
      rectA: { ...rectA, left: nextLeftA },
      rectB: { ...rectB, left: nextLeftB, width: nextWidthB }
    };
  } else {
    return {
      rectA,
      rectB: { ...rectB, left: nextLeftB }
    };
  }
}
