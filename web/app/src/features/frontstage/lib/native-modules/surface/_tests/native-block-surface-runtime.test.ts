import { afterEach, describe, expect, test, vi } from 'vitest';

import type { NativeOverlayHost } from '../../native-overlay-host';
import { createNativeBlockSurfaceRuntime } from '../native-block-surface-runtime';

const restoreObserverHarnesses: Array<() => void> = [];

afterEach(() => {
  for (const restore of restoreObserverHarnesses.splice(0).reverse()) restore();
  document.body.replaceChildren();
  vi.unstubAllGlobals();
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

  test('D1-AC-006 discovers shared overflow ancestors across nested hosts and slots', () => {
    const frames = installAnimationFrameQueue();
    const observers = installObserverHarnesses();
    const fixture = createSurfaceFixture();
    const overflow = document.createElement('div');
    overflow.style.overflow = 'auto';
    const nestedHost = document.createElement('div');
    const nestedRoot = nestedHost.attachShadow({ mode: 'open' });
    const slot = document.createElement('slot');
    nestedRoot.append(slot);
    const first = document.createElement('button');
    const second = document.createElement('button');
    nestedHost.append(first, second);
    overflow.append(nestedHost);
    fixture.targetRoot.append(overflow);
    const addScroll = vi.spyOn(overflow, 'addEventListener');
    const removeScroll = vi.spyOn(overflow, 'removeEventListener');
    const removeSlot = vi.spyOn(slot, 'removeEventListener');
    const firstMeasure = vi.fn(() => first);
    const secondMeasure = vi.fn(() => second);

    const unregisterFirst = fixture.runtime.registerAnchor({
      target: () => first,
      measure: firstMeasure,
      commit: vi.fn()
    });
    fixture.runtime.registerAnchor({
      target: () => second,
      measure: secondMeasure,
      commit: vi.fn()
    });
    drainLatestFrame(frames);

    expect(first.assignedSlot).toBe(slot);
    expect(second.assignedSlot).toBe(slot);
    expect(
      addScroll.mock.calls.filter(([type]) => type === 'scroll')
    ).toHaveLength(1);
    expect(observers.resize.observed).toEqual(
      expect.arrayContaining([first, second, overflow])
    );

    overflow.dispatchEvent(new Event('scroll'));
    drainLatestFrame(frames);
    expect(firstMeasure).toHaveBeenCalledTimes(2);
    expect(secondMeasure).toHaveBeenCalledTimes(2);

    unregisterFirst();
    expect(
      removeScroll.mock.calls.filter(([type]) => type === 'scroll')
    ).toHaveLength(0);
    overflow.dispatchEvent(new Event('scroll'));
    drainLatestFrame(frames);
    expect(firstMeasure).toHaveBeenCalledTimes(2);
    expect(secondMeasure).toHaveBeenCalledTimes(3);

    fixture.runtime.dispose();
    expect(
      removeScroll.mock.calls.filter(([type]) => type === 'scroll')
    ).toHaveLength(1);
    expect(
      removeSlot.mock.calls.filter(([type]) => type === 'slotchange')
    ).toHaveLength(1);
  });

  test('D1-AC-006 reconciles slot reassignment and dirties only the assigned chain', () => {
    const frames = installAnimationFrameQueue();
    installObserverHarnesses();
    const fixture = createSurfaceFixture();
    const nestedHost = document.createElement('div');
    const nestedRoot = nestedHost.attachShadow({ mode: 'open' });
    const firstOverflow = document.createElement('div');
    const secondOverflow = document.createElement('div');
    firstOverflow.style.overflow = 'auto';
    secondOverflow.style.overflow = 'auto';
    const firstSlot = document.createElement('slot');
    const secondSlot = document.createElement('slot');
    firstSlot.name = 'first';
    secondSlot.name = 'second';
    firstOverflow.append(firstSlot);
    secondOverflow.append(secondSlot);
    nestedRoot.append(firstOverflow, secondOverflow);
    const target = document.createElement('button');
    target.slot = 'first';
    nestedHost.append(target);
    fixture.targetRoot.append(nestedHost);
    const measure = vi.fn(() => target);
    fixture.runtime.registerAnchor({
      target: () => target,
      measure,
      commit: vi.fn()
    });
    drainLatestFrame(frames);

    target.slot = 'second';
    firstSlot.dispatchEvent(new Event('slotchange'));
    drainLatestFrame(frames);
    firstOverflow.dispatchEvent(new Event('scroll'));
    expect(frames.callbacks).toHaveLength(2);
    secondOverflow.dispatchEvent(new Event('scroll'));
    expect(frames.callbacks).toHaveLength(3);
    drainLatestFrame(frames);
    expect(measure).toHaveBeenCalledTimes(3);

    fixture.runtime.dispose();
  });

  test('D1-AC-003 dirties only anchors associated with the changed ancestor or ResizeObserver target', () => {
    const frames = installAnimationFrameQueue();
    const observers = installObserverHarnesses();
    const fixture = createSurfaceFixture();
    const firstOverflow = document.createElement('div');
    const secondOverflow = document.createElement('div');
    firstOverflow.style.overflow = 'auto';
    secondOverflow.style.overflow = 'auto';
    const first = document.createElement('button');
    const second = document.createElement('button');
    firstOverflow.append(first);
    secondOverflow.append(second);
    fixture.targetRoot.append(firstOverflow, secondOverflow);
    const firstMeasure = vi.fn(() => first);
    const secondMeasure = vi.fn(() => second);
    fixture.runtime.registerAnchor({
      target: () => first,
      measure: firstMeasure,
      commit: vi.fn()
    });
    fixture.runtime.registerAnchor({
      target: () => second,
      measure: secondMeasure,
      commit: vi.fn()
    });
    drainLatestFrame(frames);

    firstOverflow.dispatchEvent(new Event('scroll'));
    drainLatestFrame(frames);
    expect(firstMeasure).toHaveBeenCalledTimes(2);
    expect(secondMeasure).toHaveBeenCalledOnce();

    observers.resize.notify(second);
    drainLatestFrame(frames);
    expect(firstMeasure).toHaveBeenCalledTimes(2);
    expect(secondMeasure).toHaveBeenCalledTimes(2);

    fixture.runtime.dispose();
  });

  test('D1-AC-003 coalesces associated scroll, resize, and observer work into one frame', () => {
    const frames = installAnimationFrameQueue();
    const observers = installObserverHarnesses();
    const fixture = createSurfaceFixture();
    const target = document.createElement('button');
    fixture.targetRoot.append(target);
    const measure = vi.fn(() => ({ top: 12 }));
    const commit = vi.fn();
    fixture.runtime.registerAnchor({ target: () => target, measure, commit });
    drainLatestFrame(frames);

    fixture.scrollOwner.dispatchEvent(new Event('scroll'));
    fixture.scrollOwner.dispatchEvent(new Event('scroll'));
    observers.resize.notify(target);
    window.dispatchEvent(new Event('resize'));
    expect(frames.callbacks).toHaveLength(2);
    drainLatestFrame(frames);
    expect(measure).toHaveBeenCalledTimes(2);
    expect(commit).toHaveBeenCalledTimes(2);

    fixture.runtime.dispose();
  });

  test('D1-AC-006 reconciles an anchor reparent without observing the document subtree', () => {
    const frames = installAnimationFrameQueue();
    const observers = installObserverHarnesses();
    const fixture = createSurfaceFixture();
    const firstOverflow = document.createElement('div');
    const secondOverflow = document.createElement('div');
    firstOverflow.style.overflow = 'auto';
    secondOverflow.style.overflow = 'auto';
    const target = document.createElement('button');
    firstOverflow.append(target);
    fixture.targetRoot.append(firstOverflow, secondOverflow);
    const measure = vi.fn(() => target);
    fixture.runtime.registerAnchor({
      target: () => target,
      measure,
      commit: vi.fn()
    });
    drainLatestFrame(frames);

    secondOverflow.append(target);
    observers.mutation.notify(firstOverflow);
    drainLatestFrame(frames);
    firstOverflow.dispatchEvent(new Event('scroll'));
    expect(frames.callbacks).toHaveLength(2);
    secondOverflow.dispatchEvent(new Event('scroll'));
    expect(frames.callbacks).toHaveLength(3);
    expect(observers.mutation.options.every(({ subtree }) => !subtree)).toBe(
      true
    );

    fixture.runtime.dispose();
  });

  test('D1-AC-006 reconciles a changed target accessor before measuring', () => {
    const frames = installAnimationFrameQueue();
    const observers = installObserverHarnesses();
    const fixture = createSurfaceFixture();
    const first = document.createElement('button');
    const second = document.createElement('button');
    fixture.targetRoot.append(first, second);
    let target = first;
    const measure = vi.fn(() => target);
    fixture.runtime.registerAnchor({
      target: () => target,
      measure,
      commit: vi.fn()
    });
    drainLatestFrame(frames);

    target = second;
    fixture.runtime.scheduleAnchors();
    drainLatestFrame(frames);
    expect(observers.resize.observed).toContain(second);
    expect(observers.resize.observed).not.toContain(first);
    expect(measure).toHaveBeenLastCalledWith();

    fixture.runtime.dispose();
  });

  test('D1-AC-003 rejects preview-design-preview ABA callbacks by generation', () => {
    const frames = installAnimationFrameQueue();
    installObserverHarnesses();
    const fixture = createSurfaceFixture('preview');
    const target = document.createElement('button');
    fixture.targetRoot.append(target);
    const commit = vi.fn();
    fixture.runtime.registerAnchor({
      target: () => target,
      measure: () => 'measured',
      commit
    });
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

  test('D1-AC-006 unregister and dispose clear listeners, observers, dirty work, and queued commits', () => {
    const frames = installAnimationFrameQueue();
    const observers = installObserverHarnesses();
    const fixture = createSurfaceFixture();
    const target = document.createElement('button');
    fixture.targetRoot.append(target);
    const removeOwnerListener = vi.spyOn(
      fixture.scrollOwner,
      'removeEventListener'
    );
    const removeWindowListener = vi.spyOn(window, 'removeEventListener');
    const commit = vi.fn();
    const unregister = fixture.runtime.registerAnchor({
      target: () => target,
      measure: () => 'measured',
      commit
    });

    unregister();
    frames.callbacks[0]();
    fixture.scrollOwner.dispatchEvent(new Event('scroll'));
    window.dispatchEvent(new Event('resize'));
    observers.resize.notify(target);
    expect(commit).not.toHaveBeenCalled();
    expect(frames.callbacks).toHaveLength(1);
    expect(
      removeOwnerListener.mock.calls.some(([type]) => type === 'scroll')
    ).toBe(true);
    expect(
      removeWindowListener.mock.calls.some(([type]) => type === 'resize')
    ).toBe(true);
    expect(observers.resize.observed).toHaveLength(0);

    fixture.runtime.registerAnchor({
      target: () => target,
      measure: () => 'disposed',
      commit
    });
    fixture.runtime.dispose();
    frames.callbacks[1]();
    expect(commit).not.toHaveBeenCalled();
    expect(observers.resize.disconnect).toHaveBeenCalledOnce();
    expect(observers.mutation.disconnect).toHaveBeenCalled();
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

function drainLatestFrame(frames: { callbacks: Array<() => void> }): void {
  frames.callbacks.at(-1)?.();
}

function installObserverHarnesses() {
  let resizeCallback: ResizeObserverCallback | null = null;
  const resizeObserved = new Set<Element>();
  const resizeDisconnect = vi.fn(() => resizeObserved.clear());
  class ResizeObserverHarness implements ResizeObserver {
    constructor(callback: ResizeObserverCallback) {
      resizeCallback = callback;
    }
    disconnect = resizeDisconnect;
    observe = vi.fn((target: Element) => resizeObserved.add(target));
    unobserve = vi.fn((target: Element) => resizeObserved.delete(target));
  }

  let mutationCallback: MutationCallback | null = null;
  const mutationObserved = new Set<Node>();
  const mutationOptions: MutationObserverInit[] = [];
  const mutationDisconnect = vi.fn(() => mutationObserved.clear());
  class MutationObserverHarness implements MutationObserver {
    constructor(callback: MutationCallback) {
      mutationCallback = callback;
    }
    disconnect = mutationDisconnect;
    observe = vi.fn((target: Node, options?: MutationObserverInit) => {
      mutationObserved.add(target);
      mutationOptions.push(options ?? {});
    });
    takeRecords = vi.fn(() => []);
  }

  const originalResizeObserver = window.ResizeObserver;
  const originalMutationObserver = window.MutationObserver;
  window.ResizeObserver = ResizeObserverHarness;
  window.MutationObserver = MutationObserverHarness;
  restoreObserverHarnesses.push(() => {
    window.ResizeObserver = originalResizeObserver;
    window.MutationObserver = originalMutationObserver;
  });
  return {
    resize: {
      disconnect: resizeDisconnect,
      get observed() {
        return [...resizeObserved];
      },
      notify(target: Element) {
        resizeCallback?.(
          [{ target } as ResizeObserverEntry],
          {} as ResizeObserver
        );
      }
    },
    mutation: {
      disconnect: mutationDisconnect,
      get observed() {
        return [...mutationObserved];
      },
      get options() {
        return mutationOptions;
      },
      notify(target: Node) {
        mutationCallback?.(
          [{ target } as MutationRecord],
          {} as MutationObserver
        );
      }
    }
  };
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
