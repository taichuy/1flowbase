interface NativeAnchorAffixLayerOptions {
  placement: 'top' | 'bottom';
  offset: number;
}

interface NativeAnchorAffixEntry {
  sequence: number;
  host: HTMLElement;
  mountElement: HTMLElement;
  placeholder: HTMLElement;
  sentinel: HTMLElement;
  options: () => NativeAnchorAffixLayerOptions;
  onPinnedChange: (pinned: boolean) => void;
  pinned: boolean;
}

interface NativeAnchorAffixRegistry {
  owner: HTMLElement | Window;
  entries: Set<NativeAnchorAffixEntry>;
  ownerInlinePosition: string | null;
}

export interface NativeAnchorAffixLayer {
  mountElement: HTMLElement;
  shadowRoot: ShadowRoot;
  refresh(): void;
  dispose(): void;
}

const AFFIX_ENTRY_EPSILON = 0.5;
const registries = new WeakMap<object, NativeAnchorAffixRegistry>();
let entrySequence = 0;

export function attachNativeAnchorAffixLayer({
  blockId,
  onPinnedChange,
  options,
  placeholder,
  scrollOwner,
  sentinel
}: {
  blockId: string;
  onPinnedChange: (pinned: boolean) => void;
  options: () => NativeAnchorAffixLayerOptions;
  placeholder: HTMLElement;
  scrollOwner: HTMLElement | Window;
  sentinel: HTMLElement;
}): NativeAnchorAffixLayer {
  const registry = getOrCreateRegistry(scrollOwner, placeholder.ownerDocument);
  const host = placeholder.ownerDocument.createElement('div');
  host.dataset.flowbaseNativeAnchorAffixLayer = blockId;
  host.dataset.flowbaseNativeAnchorPinned = 'false';
  Object.assign(host.style, {
    display: 'block',
    height: '0px',
    overflow: 'visible',
    pointerEvents: 'none',
    width: '0px',
    zIndex: '20'
  });
  const shadowRoot = host.attachShadow({ mode: 'open' });
  const mountElement = placeholder.ownerDocument.createElement('div');
  mountElement.dataset.flowbaseNativeAnchorAffixMount = '';
  Object.assign(mountElement.style, {
    display: 'flow-root',
    pointerEvents: 'auto',
    width: '100%'
  });
  shadowRoot.append(mountElement);
  ownerElement(scrollOwner, placeholder.ownerDocument).prepend(host);

  const entry: NativeAnchorAffixEntry = {
    sequence: ++entrySequence,
    host,
    mountElement,
    placeholder,
    sentinel,
    options,
    onPinnedChange,
    pinned: false
  };
  registry.entries.add(entry);
  refreshRegistry(registry);

  let disposed = false;
  return {
    mountElement,
    shadowRoot,
    refresh() {
      if (!disposed) refreshRegistry(registry);
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      registry.entries.delete(entry);
      host.remove();
      if (entry.pinned) onPinnedChange(false);
      if (registry.entries.size === 0) {
        restoreRegistry(registry);
        registries.delete(scrollOwner);
      } else {
        refreshRegistry(registry);
      }
    }
  };
}

function getOrCreateRegistry(
  scrollOwner: HTMLElement | Window,
  ownerDocument: Document
): NativeAnchorAffixRegistry {
  const active = registries.get(scrollOwner);
  if (active) return active;
  let ownerInlinePosition: string | null = null;
  if (scrollOwner instanceof HTMLElement) {
    ownerInlinePosition = scrollOwner.style.position;
    if (
      ownerDocument.defaultView?.getComputedStyle(scrollOwner).position ===
      'static'
    ) {
      scrollOwner.style.position = 'relative';
    }
  }
  const registry: NativeAnchorAffixRegistry = {
    owner: scrollOwner,
    entries: new Set(),
    ownerInlinePosition
  };
  registries.set(scrollOwner, registry);
  return registry;
}

function restoreRegistry(registry: NativeAnchorAffixRegistry): void {
  if (
    registry.owner instanceof HTMLElement &&
    registry.ownerInlinePosition !== null
  ) {
    registry.owner.style.position = registry.ownerInlinePosition;
  }
}

function refreshRegistry(registry: NativeAnchorAffixRegistry): void {
  const entries = [...registry.entries].filter(
    (entry) => entry.host.isConnected && entry.sentinel.isConnected
  );
  for (const entry of entries) syncLayerGeometry(registry.owner, entry);
  const topWinner = selectWinner(registry.owner, entries, 'top');
  const bottomWinner = selectWinner(registry.owner, entries, 'bottom');
  for (const entry of entries) {
    const placement = entry.options().placement;
    setPinned(
      registry.owner,
      entry,
      placement === 'top' ? entry === topWinner : entry === bottomWinner
    );
  }
}

function selectWinner(
  scrollOwner: HTMLElement | Window,
  entries: NativeAnchorAffixEntry[],
  placement: 'top' | 'bottom'
): NativeAnchorAffixEntry | null {
  const ownerRect = getOwnerRect(scrollOwner);
  const scrollPosition = getScrollPosition(scrollOwner);
  let winner: NativeAnchorAffixEntry | null = null;
  let winnerPosition = placement === 'top' ? -Infinity : Infinity;
  for (const entry of entries) {
    const options = entry.options();
    if (options.placement !== placement) continue;
    const sentinelTop = entry.sentinel.getBoundingClientRect().top;
    const desiredTop =
      placement === 'top'
        ? ownerRect.top + options.offset
        : ownerRect.bottom - options.offset;
    const entered =
      placement === 'top'
        ? sentinelTop <= desiredTop - AFFIX_ENTRY_EPSILON
        : sentinelTop >= desiredTop + AFFIX_ENTRY_EPSILON;
    if (!entered) continue;
    const documentPosition = sentinelTop + scrollPosition;
    const improves =
      placement === 'top'
        ? documentPosition > winnerPosition ||
          (documentPosition === winnerPosition &&
            entry.sequence > (winner?.sequence ?? -Infinity))
        : documentPosition < winnerPosition ||
          (documentPosition === winnerPosition &&
            entry.sequence > (winner?.sequence ?? -Infinity));
    if (improves) {
      winner = entry;
      winnerPosition = documentPosition;
    }
  }
  return winner;
}

function setPinned(
  scrollOwner: HTMLElement | Window,
  entry: NativeAnchorAffixEntry,
  pinned: boolean
): void {
  if (entry.pinned === pinned) return;
  entry.pinned = pinned;
  entry.host.dataset.flowbaseNativeAnchorPinned = String(pinned);
  syncLayerPosition(scrollOwner, entry);
  entry.onPinnedChange(pinned);
}

function syncLayerGeometry(
  scrollOwner: HTMLElement | Window,
  entry: NativeAnchorAffixEntry
): void {
  const placeholderRect = entry.placeholder.getBoundingClientRect();
  const ownerRect = getOwnerRect(scrollOwner);
  const relativeLeft =
    scrollOwner instanceof HTMLElement
      ? placeholderRect.left - ownerRect.left + scrollOwner.scrollLeft
      : placeholderRect.left + window.scrollX;
  entry.host.style.width = `${placeholderRect.width}px`;
  entry.host.style.transform = `translateX(${relativeLeft}px)`;
  syncLayerPosition(scrollOwner, entry);
}

function syncLayerPosition(
  scrollOwner: HTMLElement | Window,
  entry: NativeAnchorAffixEntry
): void {
  const options = entry.options();
  entry.host.style.bottom = '';
  if (entry.pinned) {
    entry.host.style.position =
      scrollOwner instanceof HTMLElement ? 'sticky' : 'fixed';
    if (options.placement === 'top') {
      entry.host.style.top = `${options.offset}px`;
    } else {
      entry.host.style.top = '';
      entry.host.style.bottom = `${options.offset}px`;
    }
    return;
  }
  const placeholderRect = entry.placeholder.getBoundingClientRect();
  const ownerRect = getOwnerRect(scrollOwner);
  entry.host.style.position = 'absolute';
  entry.host.style.top = `${
    scrollOwner instanceof HTMLElement
      ? placeholderRect.top - ownerRect.top + scrollOwner.scrollTop
      : placeholderRect.top + window.scrollY
  }px`;
}

function getOwnerRect(scrollOwner: HTMLElement | Window): {
  top: number;
  bottom: number;
  left: number;
} {
  return scrollOwner instanceof HTMLElement
    ? scrollOwner.getBoundingClientRect()
    : { top: 0, bottom: window.innerHeight, left: 0 };
}

function getScrollPosition(scrollOwner: HTMLElement | Window): number {
  return scrollOwner instanceof HTMLElement
    ? scrollOwner.scrollTop
    : window.scrollY;
}

function ownerElement(
  scrollOwner: HTMLElement | Window,
  ownerDocument: Document
): HTMLElement {
  return scrollOwner instanceof HTMLElement ? scrollOwner : ownerDocument.body;
}
