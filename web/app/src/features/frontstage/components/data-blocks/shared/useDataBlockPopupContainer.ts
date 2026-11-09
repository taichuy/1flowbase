import { useCallback, useContext } from 'react';
import { ConfigContext } from 'antd/es/config-provider/context';
import { useNativeBlockSurface } from '../../../lib/native-modules/native-block-surface-context';

/** Use the surface's top layer so a clipped block viewport cannot clip its menus. */
export function useDataBlockPopupContainer() {
  const surface = useNativeBlockSurface();
  const { getPopupContainer } = useContext(ConfigContext);
  return useCallback(
    (trigger?: HTMLElement): HTMLElement => {
      if (surface) return surface.overlayHost.getPopupContainer();
      return (
        getPopupContainer?.(trigger) ??
        trigger?.ownerDocument.body ??
        document.body
      );
    },
    [getPopupContainer, surface]
  );
}
