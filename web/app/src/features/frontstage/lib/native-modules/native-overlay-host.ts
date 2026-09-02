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
  if (
    element.hasAttribute('hidden') ||
    element.getAttribute('aria-hidden') === 'true' ||
    /(?:^|\s)[^\s]*-hidden(?:\s|$)/u.test(element.className)
  ) {
    return false;
  }
  const style = ownerWindow?.getComputedStyle(element);
  return style?.display !== 'none' && style?.visibility !== 'hidden';
}
