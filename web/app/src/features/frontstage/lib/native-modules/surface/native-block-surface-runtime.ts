import type { BlockContextSurface } from '@1flowbase/page-protocol';

import type { NativeOverlayHost } from '../native-overlay-host';

export interface NativeBlockSurfaceAnchor<TMeasurement> {
  measure(): TMeasurement;
  commit(measurement: TMeasurement): void;
}

interface RegisteredAnchor {
  measure(): unknown;
  commit(measurement: unknown): void;
}

export interface NativeBlockSurfaceRuntime {
  readonly targetRoot: ShadowRoot;
  readonly scrollOwner: HTMLElement | Window;
  readonly overlayHost: NativeOverlayHost;
  readonly layoutEpoch: string;
  readonly generation: number;
  readonly blockContextSurface: BlockContextSurface;
  advanceLayoutEpoch(layoutEpoch: string): number;
  registerAnchor<TMeasurement>(
    anchor: NativeBlockSurfaceAnchor<TMeasurement>
  ): () => void;
  scheduleAnchors(): void;
  dispose(): void;
}

export function createNativeBlockSurfaceRuntime({
  layoutEpoch,
  overlayHost,
  scrollOwner,
  targetRoot
}: {
  layoutEpoch: string;
  overlayHost: NativeOverlayHost;
  scrollOwner: HTMLElement | Window;
  targetRoot: ShadowRoot;
}): NativeBlockSurfaceRuntime {
  const ownerWindow = targetRoot.ownerDocument.defaultView ?? window;
  const anchors = new Set<RegisteredAnchor>();
  let currentLayoutEpoch = layoutEpoch;
  let currentGeneration = 1;
  let disposed = false;
  let scheduledFrame: number | null = null;

  const scheduleAnchors = () => {
    if (disposed || scheduledFrame !== null || anchors.size === 0) return;
    const generation = currentGeneration;
    const requestId = ownerWindow.requestAnimationFrame(() => {
      if (
        disposed ||
        scheduledFrame !== requestId ||
        generation !== currentGeneration
      ) {
        return;
      }
      scheduledFrame = null;
      const measured = [...anchors].map((anchor) => ({
        anchor,
        measurement: anchor.measure()
      }));
      if (disposed || generation !== currentGeneration) return;
      for (const { anchor, measurement } of measured) {
        if (!anchors.has(anchor)) continue;
        anchor.commit(measurement);
        if (disposed || generation !== currentGeneration) return;
      }
    });
    scheduledFrame = requestId;
  };

  const blockContextSurface: BlockContextSurface = Object.freeze({
    reveal(target: Element): boolean {
      if (disposed || target.getRootNode() !== targetRoot) return false;
      return revealWithinScrollOwner(target, scrollOwner, ownerWindow);
    }
  });

  const runtime: NativeBlockSurfaceRuntime = {
    targetRoot,
    scrollOwner,
    overlayHost,
    get layoutEpoch() {
      return currentLayoutEpoch;
    },
    get generation() {
      return currentGeneration;
    },
    blockContextSurface,
    advanceLayoutEpoch(nextLayoutEpoch) {
      if (disposed || nextLayoutEpoch === currentLayoutEpoch) {
        return currentGeneration;
      }
      currentLayoutEpoch = nextLayoutEpoch;
      currentGeneration += 1;
      if (scheduledFrame !== null) {
        ownerWindow.cancelAnimationFrame(scheduledFrame);
        scheduledFrame = null;
      }
      scheduleAnchors();
      return currentGeneration;
    },
    registerAnchor<TMeasurement>(
      anchor: NativeBlockSurfaceAnchor<TMeasurement>
    ) {
      if (disposed) return () => undefined;
      const registered: RegisteredAnchor = {
        measure: () => anchor.measure(),
        commit(measurement) {
          anchor.commit(measurement as TMeasurement);
        }
      };
      anchors.add(registered);
      scheduleAnchors();
      return () => {
        anchors.delete(registered);
        if (anchors.size === 0 && scheduledFrame !== null) {
          ownerWindow.cancelAnimationFrame(scheduledFrame);
          scheduledFrame = null;
        }
      };
    },
    scheduleAnchors,
    dispose() {
      if (disposed) return;
      disposed = true;
      currentGeneration += 1;
      if (scheduledFrame !== null) {
        ownerWindow.cancelAnimationFrame(scheduledFrame);
        scheduledFrame = null;
      }
      anchors.clear();
      scrollOwner.removeEventListener('scroll', scheduleAnchors);
      ownerWindow.removeEventListener('resize', scheduleAnchors);
    }
  };

  scrollOwner.addEventListener('scroll', scheduleAnchors, { passive: true });
  ownerWindow.addEventListener('resize', scheduleAnchors, { passive: true });
  return runtime;
}

function revealWithinScrollOwner(
  target: Element,
  scrollOwner: HTMLElement | Window,
  ownerWindow: Window
): boolean {
  const targetRect = target.getBoundingClientRect();
  const ownerRect =
    scrollOwner instanceof HTMLElement
      ? scrollOwner.getBoundingClientRect()
      : { top: 0, bottom: ownerWindow.innerHeight };
  const displacement = nearestVerticalDisplacement(targetRect, ownerRect);
  if (displacement === 0) return true;
  const currentTop =
    scrollOwner instanceof HTMLElement
      ? scrollOwner.scrollTop
      : ownerWindow.scrollY;
  scrollOwner.scrollTo({
    top: currentTop + displacement,
    behavior: 'auto'
  });
  return true;
}

function nearestVerticalDisplacement(
  target: Pick<DOMRect, 'top' | 'bottom'>,
  viewport: Pick<DOMRect, 'top' | 'bottom'>
): number {
  if (target.top < viewport.top && target.bottom > viewport.bottom) return 0;
  if (target.top < viewport.top) return target.top - viewport.top;
  if (target.bottom > viewport.bottom) return target.bottom - viewport.bottom;
  return 0;
}
