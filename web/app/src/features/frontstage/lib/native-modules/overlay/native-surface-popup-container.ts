import { useCallback } from 'react';

import { useNativeBlockSurface } from '../native-block-surface-context';

export type NativePopupContainer = (triggerNode: HTMLElement) => HTMLElement;

const MISSING_SURFACE_MESSAGE =
  'Native popup container requires an active Block Surface runtime.';

export function useNativeSurfacePopupContainer(
  authoredContainer?: NativePopupContainer
): NativePopupContainer {
  const surface = useNativeBlockSurface();
  const surfaceContainer = useCallback(() => {
    if (!surface) throw new Error(MISSING_SURFACE_MESSAGE);
    return surface.overlayHost.container;
  }, [surface]);
  if (authoredContainer) return authoredContainer;
  if (!surface) throw new Error(MISSING_SURFACE_MESSAGE);
  return surfaceContainer;
}
