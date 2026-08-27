import {
  createContext,
  createElement,
  useContext,
  type ReactNode
} from 'react';

export interface NativeBlockSurfaceScope {
  targetRoot: ShadowRoot;
  scrollOwner: HTMLElement | Window;
}

const NativeBlockSurfaceContext = createContext<NativeBlockSurfaceScope | null>(
  null
);

export function NativeBlockSurfaceProvider({
  children,
  scope
}: {
  children: ReactNode;
  scope: NativeBlockSurfaceScope;
}): ReactNode {
  return createElement(
    NativeBlockSurfaceContext.Provider,
    { value: scope },
    children
  );
}

export function useNativeBlockSurface(): NativeBlockSurfaceScope | null {
  return useContext(NativeBlockSurfaceContext);
}

export function resolveNativeBlockScrollOwner(root: Element): HTMLElement | Window {
  let candidate = root.parentElement;
  while (candidate) {
    if (candidate.hasAttribute('data-flowbase-frontstage-scroll-owner')) {
      return candidate;
    }
    const overflowY = window.getComputedStyle(candidate).overflowY;
    if (
      overflowY === 'auto' ||
      overflowY === 'scroll' ||
      overflowY === 'overlay'
    ) {
      return candidate;
    }
    candidate = candidate.parentElement;
  }
  return root.ownerDocument.defaultView ?? window;
}
