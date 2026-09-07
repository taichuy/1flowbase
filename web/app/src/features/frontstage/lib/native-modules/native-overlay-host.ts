import { createNativeOverlayLayer } from './native-overlay-layer';

export interface NativeOverlayHost {
  readonly container: HTMLDivElement;
  getPopupContainer(): HTMLDivElement;
  dispose(): void;
}

export function createNativeOverlayHost({
  blockId,
  targetRoot
}: {
  blockId: string;
  targetRoot: ShadowRoot;
}): NativeOverlayHost {
  const layer = createNativeOverlayLayer({ blockId, targetRoot });
  const ownerWindow = targetRoot.ownerDocument.defaultView;
  let active = false;
  let disposed = false;

  const syncDefaultPopupOwner = () => {
    const hasVisiblePopup = Array.from(layer.container.children).some(
      (element) =>
        !element.hasAttribute('data-flowbase-native-overlay-interaction') &&
        isVisibleOverlayElement(element, ownerWindow)
    );
    if (disposed || hasVisiblePopup === active) return;
    active = hasVisiblePopup;
    if (active) layer.activate();
    else layer.deactivate();
  };
  const MutationObserver = ownerWindow?.MutationObserver;
  const observer = MutationObserver
    ? new MutationObserver(syncDefaultPopupOwner)
    : null;
  observer?.observe(layer.container, {
    attributeFilter: ['aria-hidden', 'class', 'hidden', 'style'],
    attributes: true,
    childList: true,
    subtree: true
  });

  return {
    container: layer.container,
    getPopupContainer() {
      return layer.container;
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      observer?.disconnect();
      active = false;
      layer.dispose();
    }
  };
}

function isVisibleOverlayElement(
  element: Element,
  ownerWindow: Window | null
): boolean {
  if (!isVisibleThroughOverlayRoot(element, element, ownerWindow)) {
    return false;
  }
  const descendants = element.querySelectorAll('*');
  if (descendants.length === 0) return true;
  return Array.from(descendants).some((candidate) =>
    isVisibleThroughOverlayRoot(candidate, element, ownerWindow)
  );
}

function isVisibleThroughOverlayRoot(
  element: Element,
  overlayRoot: Element,
  ownerWindow: Window | null
): boolean {
  let current: Element | null = element;
  while (current) {
    if (
      current.hasAttribute('hidden') ||
      current.getAttribute('aria-hidden') === 'true' ||
      /(?:^|\s)[^\s]*-hidden(?:\s|$)/u.test(current.className)
    ) {
      return false;
    }
    const style = ownerWindow?.getComputedStyle(current);
    if (style?.display === 'none' || style?.visibility === 'hidden') {
      return false;
    }
    if (current === overlayRoot) return true;
    current = current.parentElement;
  }
  return false;
}
