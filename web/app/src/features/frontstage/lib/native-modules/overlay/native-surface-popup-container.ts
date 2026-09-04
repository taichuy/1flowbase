import { useCallback } from 'react';

import { useNativeBlockSurface } from '../native-block-surface-context';

export type NativePopupContainer = (triggerNode: HTMLElement) => HTMLElement;

export function useNativeSurfacePopupContainer(
  authoredContainer?: NativePopupContainer
): NativePopupContainer | undefined {
  const surface = useNativeBlockSurface();
  const surfaceContainer = useCallback(
    () => surface?.overlayHost.container ?? document.body,
    [surface]
  );
  return authoredContainer ?? (surface ? surfaceContainer : undefined);
}
