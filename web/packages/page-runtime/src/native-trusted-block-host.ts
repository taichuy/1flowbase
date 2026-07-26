const NATIVE_TRUSTED_BLOCK_ROOT_ATTRIBUTE =
  'data-flowbase-native-trusted-block-root';
const NATIVE_TRUSTED_BLOCK_ID_ATTRIBUTE =
  'data-flowbase-native-trusted-block-id';
const NATIVE_TRUSTED_BLOCK_MOUNT_ATTRIBUTE =
  'data-flowbase-native-trusted-block-mount';

const ownedShadowRoots = new WeakMap<Element, ShadowRoot>();
const activeRoots = new WeakSet<Element>();

export interface NativeTrustedBlockPortalSurface {
  root: Element;
  shadowRoot: ShadowRoot;
  mountElement: HTMLElement;
  dispose(): void;
}

export interface NativeTrustedBlockPortalSurfaceInput {
  root: Element;
  blockId: string;
}

/**
 * Attaches the DOM boundary for one surface-owned React portal.
 * React render and unmount remain exclusively owned by the caller's tree.
 */
export function attachNativeTrustedBlockPortalSurface({
  root,
  blockId
}: NativeTrustedBlockPortalSurfaceInput): NativeTrustedBlockPortalSurface {
  validatePortalSurfaceRoot(root);
  if (activeRoots.has(root)) {
    throw new Error('Native trusted block root is already active.');
  }

  let shadowRoot = ownedShadowRoots.get(root);
  if (!shadowRoot) {
    if (root.shadowRoot) {
      throw new Error(
        'Native trusted block root must not contain a pre-existing ShadowRoot.'
      );
    }
    shadowRoot = root.attachShadow({ mode: 'open' });
    ownedShadowRoots.set(root, shadowRoot);
  }

  const styleScope = applyStyleScope(root, blockId);
  const mountElement = document.createElement('div');
  mountElement.setAttribute(NATIVE_TRUSTED_BLOCK_MOUNT_ATTRIBUTE, '');
  mountElement.setAttribute(NATIVE_TRUSTED_BLOCK_ID_ATTRIBUTE, blockId);
  shadowRoot.replaceChildren(mountElement);
  activeRoots.add(root);

  let didDispose = false;
  return {
    root,
    shadowRoot,
    mountElement,
    dispose() {
      if (didDispose) return;
      didDispose = true;
      activeRoots.delete(root);
      shadowRoot.replaceChildren();
      styleScope.restore();
    }
  };
}

function validatePortalSurfaceRoot(root: unknown): asserts root is Element {
  if (typeof Element === 'undefined' || !(root instanceof Element)) {
    throw new Error('Native trusted block portal root must be a DOM Element.');
  }
}

interface AttributeSnapshot {
  attribute: string;
  value: string | null;
}

function applyStyleScope(
  root: Element,
  blockId: string
): { restore(): void } {
  const snapshots = [
    snapshotAttribute(root, NATIVE_TRUSTED_BLOCK_ROOT_ATTRIBUTE),
    snapshotAttribute(root, NATIVE_TRUSTED_BLOCK_ID_ATTRIBUTE)
  ];

  root.setAttribute(NATIVE_TRUSTED_BLOCK_ROOT_ATTRIBUTE, '');
  root.setAttribute(NATIVE_TRUSTED_BLOCK_ID_ATTRIBUTE, blockId);

  return {
    restore() {
      snapshots.forEach(restoreAttribute.bind(null, root));
    }
  };
}

function snapshotAttribute(
  root: Element,
  attribute: string
): AttributeSnapshot {
  return { attribute, value: root.getAttribute(attribute) };
}

function restoreAttribute(root: Element, snapshot: AttributeSnapshot): void {
  if (snapshot.value === null) {
    root.removeAttribute(snapshot.attribute);
    return;
  }
  root.setAttribute(snapshot.attribute, snapshot.value);
}
