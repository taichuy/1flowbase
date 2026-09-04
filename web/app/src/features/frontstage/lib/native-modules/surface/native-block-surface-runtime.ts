import type { BlockContextSurface } from '@1flowbase/page-protocol';

import type { NativeOverlayHost } from '../native-overlay-host';

export interface NativeBlockSurfaceAnchor<TMeasurement> {
  target(): Element | null;
  measure(): TMeasurement;
  commit(measurement: TMeasurement): void;
}

interface RegisteredAnchor {
  target(): Element | null;
  measure(): unknown;
  commit(measurement: unknown): void;
  overflowAncestors: Set<EventTarget>;
  observedElements: Set<Element>;
  composedAncestors: Set<Element>;
  slots: Set<HTMLSlotElement>;
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
  const dirtyAnchors = new Set<RegisteredAnchor>();
  const scrollMembers = new Map<EventTarget, Set<RegisteredAnchor>>();
  const resizeMembers = new Map<Element, Set<RegisteredAnchor>>();
  const mutationMembers = new Map<Element, Set<RegisteredAnchor>>();
  const slotMembers = new Map<HTMLSlotElement, Set<RegisteredAnchor>>();
  const scrollListeners = new Map<EventTarget, EventListener>();
  const slotListeners = new Map<HTMLSlotElement, EventListener>();
  const ResizeObserverConstructor = ownerWindow.ResizeObserver;
  const resizeObserver = ResizeObserverConstructor
    ? new ResizeObserverConstructor((entries) => {
        for (const entry of entries) {
          markDirty(resizeMembers.get(entry.target));
        }
      })
    : null;
  const MutationObserverConstructor = ownerWindow.MutationObserver;
  const mutationObserver = MutationObserverConstructor
    ? new MutationObserverConstructor((records) => {
        for (const record of records) {
          if (record.target instanceof Element) {
            markDirty(mutationMembers.get(record.target));
          }
        }
      })
    : null;
  let currentLayoutEpoch = layoutEpoch;
  let currentGeneration = 1;
  let disposed = false;
  let scheduledFrame: number | null = null;
  let windowResizeListening = false;

  const requestDrain = () => {
    if (disposed || scheduledFrame !== null || dirtyAnchors.size === 0) return;
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
      const pending = [...dirtyAnchors];
      dirtyAnchors.clear();
      const measured: Array<{
        anchor: RegisteredAnchor;
        measurement: unknown;
      }> = [];
      for (const anchor of pending) {
        if (!anchors.has(anchor)) continue;
        reconcileAnchor(anchor);
        measured.push({ anchor, measurement: anchor.measure() });
        if (disposed || generation !== currentGeneration) return;
      }
      for (const { anchor, measurement } of measured) {
        if (!anchors.has(anchor)) continue;
        anchor.commit(measurement);
        if (disposed || generation !== currentGeneration) return;
      }
      requestDrain();
    });
    scheduledFrame = requestId;
  };

  function markDirty(members: Iterable<RegisteredAnchor> | undefined): void {
    if (disposed || !members) return;
    for (const anchor of members) {
      if (anchors.has(anchor)) dirtyAnchors.add(anchor);
    }
    requestDrain();
  }

  function reconcileAnchor(anchor: RegisteredAnchor): void {
    const target = anchor.target();
    const composedAncestors = target
      ? collectComposedAncestors(target)
      : new Set<Element>();
    const overflowAncestors = new Set<EventTarget>();
    if (target) {
      for (const ancestor of composedAncestors) {
        if (
          isOverflowAncestor(ancestor, ownerWindow) ||
          ancestor === scrollOwner
        ) {
          overflowAncestors.add(ancestor);
        }
      }
      if (scrollOwner === ownerWindow) overflowAncestors.add(ownerWindow);
    }
    const observedElements = new Set<Element>();
    if (target) observedElements.add(target);
    for (const ancestor of overflowAncestors) {
      if (ancestor instanceof Element) observedElements.add(ancestor);
    }
    const slots = new Set<HTMLSlotElement>();
    if (target?.assignedSlot) slots.add(target.assignedSlot);
    for (const ancestor of composedAncestors) {
      if (ancestor instanceof HTMLSlotElement) slots.add(ancestor);
      if (ancestor.assignedSlot) slots.add(ancestor.assignedSlot);
    }

    replaceMembership(
      anchor,
      anchor.overflowAncestors,
      overflowAncestors,
      scrollMembers,
      addScrollListener,
      removeScrollListener
    );
    replaceMembership(
      anchor,
      anchor.observedElements,
      observedElements,
      resizeMembers,
      (element) => resizeObserver?.observe(element),
      (element) => resizeObserver?.unobserve(element)
    );
    replaceMembership(
      anchor,
      anchor.composedAncestors,
      composedAncestors,
      mutationMembers
    );
    replaceMembership(
      anchor,
      anchor.slots,
      slots,
      slotMembers,
      addSlotListener,
      removeSlotListener
    );
    rebuildMutationObserver();
  }

  function addScrollListener(target: EventTarget): void {
    const listener: EventListener = () => markDirty(scrollMembers.get(target));
    scrollListeners.set(target, listener);
    target.addEventListener('scroll', listener, { passive: true });
  }

  function removeScrollListener(target: EventTarget): void {
    const listener = scrollListeners.get(target);
    if (!listener) return;
    target.removeEventListener('scroll', listener);
    scrollListeners.delete(target);
  }

  function addSlotListener(slot: HTMLSlotElement): void {
    const listener: EventListener = () => markDirty(slotMembers.get(slot));
    slotListeners.set(slot, listener);
    slot.addEventListener('slotchange', listener);
  }

  function removeSlotListener(slot: HTMLSlotElement): void {
    const listener = slotListeners.get(slot);
    if (!listener) return;
    slot.removeEventListener('slotchange', listener);
    slotListeners.delete(slot);
  }

  function rebuildMutationObserver(): void {
    if (!mutationObserver) return;
    mutationObserver.disconnect();
    for (const element of mutationMembers.keys()) {
      mutationObserver.observe(element, { attributes: true, childList: true });
    }
  }

  function removeAnchorMembership(anchor: RegisteredAnchor): void {
    replaceMembership(
      anchor,
      anchor.overflowAncestors,
      new Set<EventTarget>(),
      scrollMembers,
      addScrollListener,
      removeScrollListener
    );
    replaceMembership(
      anchor,
      anchor.observedElements,
      new Set<Element>(),
      resizeMembers,
      (element) => resizeObserver?.observe(element),
      (element) => resizeObserver?.unobserve(element)
    );
    replaceMembership(
      anchor,
      anchor.composedAncestors,
      new Set<Element>(),
      mutationMembers
    );
    replaceMembership(
      anchor,
      anchor.slots,
      new Set<HTMLSlotElement>(),
      slotMembers,
      addSlotListener,
      removeSlotListener
    );
    rebuildMutationObserver();
  }

  const scheduleAnchors = () => markDirty(anchors);
  const handleWindowResize = () => markDirty(anchors);

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
        target: () => anchor.target(),
        measure: () => anchor.measure(),
        commit(measurement) {
          anchor.commit(measurement as TMeasurement);
        },
        overflowAncestors: new Set(),
        observedElements: new Set(),
        composedAncestors: new Set(),
        slots: new Set()
      };
      anchors.add(registered);
      reconcileAnchor(registered);
      if (!windowResizeListening) {
        ownerWindow.addEventListener('resize', handleWindowResize, {
          passive: true
        });
        windowResizeListening = true;
      }
      markDirty([registered]);
      let unregistered = false;
      return () => {
        if (unregistered) return;
        unregistered = true;
        anchors.delete(registered);
        dirtyAnchors.delete(registered);
        removeAnchorMembership(registered);
        if (anchors.size === 0) {
          if (scheduledFrame !== null) {
            ownerWindow.cancelAnimationFrame(scheduledFrame);
            scheduledFrame = null;
          }
          if (windowResizeListening) {
            ownerWindow.removeEventListener('resize', handleWindowResize);
            windowResizeListening = false;
          }
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
      for (const target of scrollListeners.keys()) removeScrollListener(target);
      for (const slot of slotListeners.keys()) removeSlotListener(slot);
      if (windowResizeListening) {
        ownerWindow.removeEventListener('resize', handleWindowResize);
        windowResizeListening = false;
      }
      resizeObserver?.disconnect();
      mutationObserver?.disconnect();
      anchors.clear();
      dirtyAnchors.clear();
      scrollMembers.clear();
      resizeMembers.clear();
      mutationMembers.clear();
      slotMembers.clear();
    }
  };

  return runtime;
}

function replaceMembership<T extends EventTarget>(
  anchor: RegisteredAnchor,
  previous: Set<T>,
  next: Set<T>,
  memberships: Map<T, Set<RegisteredAnchor>>,
  onFirstMember?: (target: T) => void,
  onLastMember?: (target: T) => void
): void {
  for (const target of previous) {
    if (next.has(target)) continue;
    const members = memberships.get(target);
    members?.delete(anchor);
    if (members?.size === 0) {
      memberships.delete(target);
      onLastMember?.(target);
    }
  }
  for (const target of next) {
    if (previous.has(target)) continue;
    let members = memberships.get(target);
    if (!members) {
      members = new Set();
      memberships.set(target, members);
      onFirstMember?.(target);
    }
    members.add(anchor);
  }
  previous.clear();
  for (const target of next) previous.add(target);
}

function collectComposedAncestors(target: Element): Set<Element> {
  const ancestors = new Set<Element>();
  let current: Element | null = target;
  while ((current = composedParent(current))) ancestors.add(current);
  return ancestors;
}

function composedParent(element: Element): Element | null {
  if (element.assignedSlot) return element.assignedSlot;
  if (element.parentElement) return element.parentElement;
  const root = element.getRootNode();
  return root instanceof ShadowRoot ? root.host : null;
}

function isOverflowAncestor(element: Element, ownerWindow: Window): boolean {
  const style = ownerWindow.getComputedStyle(element);
  return [style.overflow, style.overflowX, style.overflowY].some(
    (overflow) =>
      overflow !== '' && overflow !== 'visible' && overflow !== 'clip'
  );
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
