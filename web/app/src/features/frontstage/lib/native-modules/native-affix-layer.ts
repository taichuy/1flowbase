interface NativeAffixLayerOptions {
  placement: 'top' | 'bottom';
  offset: number;
}

type NativeAffixState = 'flow' | 'pinned' | 'boundary';

interface NativeAffixOwnerLease {
  count: number;
  inlinePosition: string | null;
}

export interface NativeAffixLayer {
  mountElement: HTMLElement;
  shadowRoot: ShadowRoot;
  refresh(): void;
  dispose(): void;
}

export interface NativeAffixPortal {
  hostElement: HTMLElement;
  mountElement: HTMLElement;
  shadowRoot: ShadowRoot;
}

const AFFIX_STATE_EPSILON = 0.5;
const ownerLeases = new WeakMap<object, NativeAffixOwnerLease>();

export function createNativeAffixPortal(
  blockId: string,
  ownerDocument: Document
): NativeAffixPortal {
  const hostElement = ownerDocument.createElement('div');
  hostElement.dataset.flowbaseNativeAffixLayer = blockId;
  hostElement.dataset.flowbaseNativeAffixState = 'flow';
  Object.assign(hostElement.style, {
    boxSizing: 'border-box',
    overflow: 'visible',
    pointerEvents: 'none',
    position: 'absolute',
    zIndex: '30'
  });
  const shadowRoot = hostElement.attachShadow({ mode: 'open' });
  const mountElement = ownerDocument.createElement('div');
  mountElement.dataset.flowbaseNativeAffixMount = '';
  Object.assign(mountElement.style, {
    boxSizing: 'border-box',
    display: 'flow-root',
    pointerEvents: 'auto',
    width: '100%'
  });
  shadowRoot.append(mountElement);
  return { hostElement, mountElement, shadowRoot };
}

export function attachNativeAffixLayer({
  blockId,
  onPinnedChange,
  options,
  placeholder,
  portal,
  scrollOwner,
  sentinel,
  surfaceHost
}: {
  blockId: string;
  onPinnedChange: (pinned: boolean) => void;
  options: () => NativeAffixLayerOptions;
  placeholder: HTMLElement;
  portal: NativeAffixPortal;
  scrollOwner: HTMLElement | Window;
  sentinel: HTMLElement;
  surfaceHost: HTMLElement;
}): NativeAffixLayer {
  const ownerDocument = placeholder.ownerDocument;
  acquireOwnerLease(scrollOwner, ownerDocument);
  const { hostElement: host, mountElement, shadowRoot } = portal;
  host.dataset.flowbaseNativeAffixLayer = blockId;
  ownerElement(scrollOwner, ownerDocument).prepend(host);

  let state: NativeAffixState = 'flow';
  let frame = 0;
  let disposed = false;

  const publishState = (nextState: NativeAffixState) => {
    if (state === nextState) return;
    const wasPinned = state === 'pinned';
    state = nextState;
    host.dataset.flowbaseNativeAffixState = nextState;
    const pinned = nextState === 'pinned';
    if (pinned !== wasPinned) onPinnedChange(pinned);
  };

  const refresh = () => {
    if (disposed || !surfaceHost.isConnected || !placeholder.isConnected) {
      return;
    }
    syncGeometry({
      host,
      mountElement,
      options: options(),
      placeholder,
      scrollOwner,
      surfaceHost
    });
    publishState(
      resolveAffixState({
        current: state,
        mountElement,
        options: options(),
        placeholder,
        scrollOwner,
        sentinel,
        surfaceHost
      })
    );
  };

  const scheduleRefresh = () => {
    if (frame || disposed) return;
    frame =
      ownerDocument.defaultView?.requestAnimationFrame(() => {
        frame = 0;
        refresh();
      }) ?? 0;
  };

  scrollOwner.addEventListener('scroll', scheduleRefresh, { passive: true });
  ownerDocument.defaultView?.addEventListener('resize', scheduleRefresh);
  const observer =
    typeof ResizeObserver === 'undefined'
      ? null
      : new ResizeObserver(scheduleRefresh);
  observer?.observe(surfaceHost);
  observer?.observe(placeholder);
  observer?.observe(mountElement);
  refresh();

  return {
    mountElement,
    shadowRoot,
    refresh,
    dispose() {
      if (disposed) return;
      disposed = true;
      if (frame) ownerDocument.defaultView?.cancelAnimationFrame(frame);
      observer?.disconnect();
      scrollOwner.removeEventListener('scroll', scheduleRefresh);
      ownerDocument.defaultView?.removeEventListener('resize', scheduleRefresh);
      if (state === 'pinned') onPinnedChange(false);
      host.remove();
      releaseOwnerLease(scrollOwner);
    }
  };
}

function syncGeometry({
  host,
  mountElement,
  options,
  placeholder,
  scrollOwner,
  surfaceHost
}: {
  host: HTMLElement;
  mountElement: HTMLElement;
  options: NativeAffixLayerOptions;
  placeholder: HTMLElement;
  scrollOwner: HTMLElement | Window;
  surfaceHost: HTMLElement;
}): void {
  const ownerRect = getOwnerRect(scrollOwner);
  const placeholderRect = placeholder.getBoundingClientRect();
  const surfaceRect = surfaceHost.getBoundingClientRect();
  const scrollLeft =
    scrollOwner instanceof HTMLElement
      ? scrollOwner.scrollLeft
      : window.scrollX;
  const scrollTop =
    scrollOwner instanceof HTMLElement ? scrollOwner.scrollTop : window.scrollY;
  const relativeLeft = placeholderRect.left - ownerRect.left + scrollLeft;
  const relativeTop = surfaceRect.top - ownerRect.top + scrollTop;
  const localTop = Math.max(0, placeholderRect.top - surfaceRect.top);

  host.style.left = `${relativeLeft}px`;
  host.style.top = `${relativeTop}px`;
  host.style.width = `${placeholderRect.width}px`;
  host.style.height = `${Math.max(surfaceRect.height, localTop + placeholderRect.height)}px`;
  mountElement.style.marginTop = `${localTop}px`;
  mountElement.style.position = 'sticky';
  mountElement.style.top =
    options.placement === 'top' ? `${options.offset}px` : '';
  mountElement.style.bottom =
    options.placement === 'bottom' ? `${options.offset}px` : '';
}

function resolveAffixState({
  current,
  mountElement,
  options,
  placeholder,
  scrollOwner,
  sentinel,
  surfaceHost
}: {
  current: NativeAffixState;
  mountElement: HTMLElement;
  options: NativeAffixLayerOptions;
  placeholder: HTMLElement;
  scrollOwner: HTMLElement | Window;
  sentinel: HTMLElement;
  surfaceHost: HTMLElement;
}): NativeAffixState {
  const ownerRect = getOwnerRect(scrollOwner);
  const surfaceRect = surfaceHost.getBoundingClientRect();
  const sentinelRect = sentinel.getBoundingClientRect();
  const placeholderRect = placeholder.getBoundingClientRect();
  const mountHeight =
    mountElement.getBoundingClientRect().height || placeholderRect.height;
  const epsilon =
    current === 'pinned' ? -AFFIX_STATE_EPSILON : AFFIX_STATE_EPSILON;

  if (options.placement === 'bottom') {
    const desiredBottom = ownerRect.bottom - options.offset;
    if (surfaceRect.top >= desiredBottom - epsilon) return 'flow';
    if (sentinelRect.bottom < desiredBottom + epsilon) return 'boundary';
    return 'pinned';
  }

  const desiredTop = ownerRect.top + options.offset;
  if (sentinelRect.top > desiredTop - epsilon) return 'flow';
  if (surfaceRect.bottom <= desiredTop + mountHeight + epsilon) {
    return 'boundary';
  }
  return 'pinned';
}

function acquireOwnerLease(
  scrollOwner: HTMLElement | Window,
  ownerDocument: Document
): void {
  const active = ownerLeases.get(scrollOwner);
  if (active) {
    active.count += 1;
    return;
  }
  let inlinePosition: string | null = null;
  if (scrollOwner instanceof HTMLElement) {
    inlinePosition = scrollOwner.style.position;
    if (
      ownerDocument.defaultView?.getComputedStyle(scrollOwner).position ===
      'static'
    ) {
      scrollOwner.style.position = 'relative';
    }
  }
  ownerLeases.set(scrollOwner, { count: 1, inlinePosition });
}

function releaseOwnerLease(scrollOwner: HTMLElement | Window): void {
  const active = ownerLeases.get(scrollOwner);
  if (!active) return;
  active.count -= 1;
  if (active.count > 0) return;
  if (scrollOwner instanceof HTMLElement && active.inlinePosition !== null) {
    scrollOwner.style.position = active.inlinePosition;
  }
  ownerLeases.delete(scrollOwner);
}

function getOwnerRect(scrollOwner: HTMLElement | Window): DOMRect {
  return scrollOwner instanceof HTMLElement
    ? scrollOwner.getBoundingClientRect()
    : ({
        top: 0,
        bottom: window.innerHeight,
        left: 0,
        right: window.innerWidth,
        width: window.innerWidth,
        height: window.innerHeight
      } as DOMRect);
}

function ownerElement(
  scrollOwner: HTMLElement | Window,
  ownerDocument: Document
): HTMLElement {
  return scrollOwner instanceof HTMLElement ? scrollOwner : ownerDocument.body;
}
