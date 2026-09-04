import { afterEach, describe, expect, test, vi } from 'vitest';

import type { NativeOverlayHost } from '../../native-overlay-host';
import { createNativeBlockSurfaceRuntime } from '../native-block-surface-runtime';

afterEach(() => {
  document.body.replaceChildren();
  vi.restoreAllMocks();
});

describe('native block surface runtime kernel', () => {
  test('D1-AC-001 reveals only local targets with the minimum owner displacement', () => {
    const fixture = createSurfaceFixture();
    const target = document.createElement('div');
    fixture.targetRoot.append(target);
    fixture.scrollOwner.scrollTop = 40;
    fixture.scrollOwner.getBoundingClientRect = () =>
      domRect({ top: 100, bottom: 300 });
    target.getBoundingClientRect = () => domRect({ top: 330, bottom: 350 });
    const scrollTo = vi.fn(({ top }: ScrollToOptions) => {
      fixture.scrollOwner.scrollTop = top ?? fixture.scrollOwner.scrollTop;
    });
    Object.defineProperty(fixture.scrollOwner, 'scrollTo', {
      configurable: true,
      value: scrollTo
    });
    const documentScrollTop = document.documentElement.scrollTop;

    expect(fixture.runtime.blockContextSurface.reveal(target)).toBe(true);
    expect(scrollTo).toHaveBeenCalledWith({ top: 90, behavior: 'auto' });
    expect(document.documentElement.scrollTop).toBe(documentScrollTop);

    target.getBoundingClientRect = () => domRect({ top: 140, bottom: 180 });
    expect(fixture.runtime.blockContextSurface.reveal(target)).toBe(true);
    expect(scrollTo).toHaveBeenCalledTimes(1);

    const foreignHost = document.createElement('div');
    document.body.append(foreignHost);
    const foreignTarget = document.createElement('div');
    foreignHost.attachShadow({ mode: 'open' }).append(foreignTarget);
    expect(fixture.runtime.blockContextSurface.reveal(foreignTarget)).toBe(
      false
    );
    expect(scrollTo).toHaveBeenCalledTimes(1);
    expect(document.documentElement.scrollTop).toBe(documentScrollTop);

    fixture.runtime.dispose();
  });

  test('D1-AC-003 coalesces scroll and resize work into one animation frame', () => {
    const frames = installAnimationFrameQueue();
    const fixture = createSurfaceFixture();
    const measure = vi.fn(() => ({ top: 12 }));
    const commit = vi.fn();
    fixture.runtime.registerAnchor({ measure, commit });

    expect(frames.callbacks).toHaveLength(1);
    frames.callbacks[0]();
    expect(measure).toHaveBeenCalledOnce();
    expect(commit).toHaveBeenCalledWith({ top: 12 });

    fixture.scrollOwner.dispatchEvent(new Event('scroll'));
    fixture.scrollOwner.dispatchEvent(new Event('scroll'));
    window.dispatchEvent(new Event('resize'));
    expect(frames.callbacks).toHaveLength(2);
    frames.callbacks[1]();
    expect(measure).toHaveBeenCalledTimes(2);
    expect(commit).toHaveBeenCalledTimes(2);

    fixture.runtime.dispose();
  });

  test('D1-AC-003 rejects preview-design-preview ABA callbacks by generation', () => {
    const frames = installAnimationFrameQueue();
    const fixture = createSurfaceFixture('preview');
    const commit = vi.fn();
    fixture.runtime.registerAnchor({ measure: () => 'measured', commit });
    const previewGeneration = fixture.runtime.generation;

    const designGeneration = fixture.runtime.advanceLayoutEpoch('design');
    const nextPreviewGeneration = fixture.runtime.advanceLayoutEpoch('preview');
    expect([
      previewGeneration,
      designGeneration,
      nextPreviewGeneration
    ]).toEqual([1, 2, 3]);

    frames.callbacks[0]();
    frames.callbacks[1]();
    expect(commit).not.toHaveBeenCalled();
    frames.callbacks[2]();
    expect(commit).toHaveBeenCalledOnce();

    fixture.runtime.dispose();
  });

  test('D1-AC-003 dispose removes listeners and rejects queued commits', () => {
    const frames = installAnimationFrameQueue();
    const fixture = createSurfaceFixture();
    const removeOwnerListener = vi.spyOn(
      fixture.scrollOwner,
      'removeEventListener'
    );
    const removeWindowListener = vi.spyOn(window, 'removeEventListener');
    const commit = vi.fn();
    fixture.runtime.registerAnchor({ measure: () => 'measured', commit });

    fixture.runtime.dispose();
    frames.callbacks[0]();
    fixture.scrollOwner.dispatchEvent(new Event('scroll'));
    window.dispatchEvent(new Event('resize'));

    expect(commit).not.toHaveBeenCalled();
    expect(frames.callbacks).toHaveLength(1);
    expect(removeOwnerListener).toHaveBeenCalledWith(
      'scroll',
      fixture.runtime.scheduleAnchors
    );
    expect(removeWindowListener).toHaveBeenCalledWith(
      'resize',
      fixture.runtime.scheduleAnchors
    );
  });
});

function createSurfaceFixture(layoutEpoch = 'preview') {
  const scrollOwner = document.createElement('div');
  const host = document.createElement('div');
  scrollOwner.append(host);
  document.body.append(scrollOwner);
  const targetRoot = host.attachShadow({ mode: 'open' });
  const overlayContainer = document.createElement('div');
  const overlayHost: NativeOverlayHost = {
    container: overlayContainer,
    getPopupContainer: () => overlayContainer,
    dispose: vi.fn()
  };
  return {
    runtime: createNativeBlockSurfaceRuntime({
      layoutEpoch,
      overlayHost,
      scrollOwner,
      targetRoot
    }),
    scrollOwner,
    targetRoot
  };
}

function installAnimationFrameQueue() {
  const callbacks: Array<() => void> = [];
  vi.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    callbacks.push(() => callback(performance.now()));
    return callbacks.length;
  });
  vi.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => undefined);
  return { callbacks };
}

function domRect({ bottom, top }: { bottom: number; top: number }): DOMRect {
  return {
    bottom,
    height: bottom - top,
    left: 0,
    right: 100,
    top,
    width: 100,
    x: 0,
    y: top,
    toJSON: () => ({})
  };
}
