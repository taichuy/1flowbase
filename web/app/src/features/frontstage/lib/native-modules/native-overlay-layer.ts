export interface NativeOverlayLayer {
  container: HTMLDivElement;
  activate(): void;
  deactivate(): void;
  dispose(): void;
}

export function createNativeOverlayLayer({
  blockId,
  targetRoot
}: {
  blockId: string;
  targetRoot: ShadowRoot;
}): NativeOverlayLayer {
  const container = targetRoot.ownerDocument.createElement('div');
  container.dataset.flowbaseNativeOverlayLayer = blockId;
  container.dataset.flowbaseNativeOverlayState = 'closed';
  container.setAttribute('popover', 'manual');
  Object.assign(container.style, {
    background: 'transparent',
    border: '0',
    boxSizing: 'border-box',
    height: '100vh',
    inset: '0',
    margin: '0',
    maxHeight: 'none',
    maxWidth: 'none',
    overflow: 'visible',
    padding: '0',
    pointerEvents: 'none',
    position: 'fixed',
    width: '100vw'
  });
  const interactionStyle = targetRoot.ownerDocument.createElement('style');
  interactionStyle.dataset.flowbaseNativeOverlayInteraction = '';
  interactionStyle.textContent =
    '[data-flowbase-native-overlay-layer] > :not(style) { pointer-events: auto; }';
  container.append(interactionStyle);
  targetRoot.append(container);

  let active = false;
  return {
    container,
    activate() {
      if (active) return;
      active = true;
      container.dataset.flowbaseNativeOverlayState = 'open';
      container.showPopover?.();
    },
    deactivate() {
      if (!active) return;
      active = false;
      container.dataset.flowbaseNativeOverlayState = 'closed';
      container.hidePopover?.();
    },
    dispose() {
      if (active) container.hidePopover?.();
      active = false;
      container.remove();
    }
  };
}
