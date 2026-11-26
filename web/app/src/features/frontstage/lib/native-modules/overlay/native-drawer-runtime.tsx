import type { DrawerProps } from 'antd';
import AntdDrawer from 'antd/es/drawer';

import { useNativeBlockSurface } from '../native-block-surface-context';

const MISSING_SURFACE_MESSAGE =
  'Native Drawer container requires an active Block Surface runtime.';

function NativeBlockDrawerComponent({ getContainer, ...props }: DrawerProps) {
  const surface = useNativeBlockSurface();
  if (getContainer === undefined && !surface) {
    throw new Error(MISSING_SURFACE_MESSAGE);
  }
  return (
    <AntdDrawer
      {...props}
      getContainer={
        getContainer !== undefined
          ? getContainer
          : surface!.overlayHost.getPopupContainer
      }
    />
  );
}

export const NativeBlockDrawer = Object.assign(NativeBlockDrawerComponent, {
  _InternalPanelDoNotUseOrYouWillBeFired:
    AntdDrawer._InternalPanelDoNotUseOrYouWillBeFired
}) as typeof AntdDrawer;
