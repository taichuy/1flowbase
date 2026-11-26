import {
  ResizableDrawer as SharedResizableDrawer,
  type ResizableDrawerProps
} from '../../../../../shared/ui/resizable-drawer/ResizableDrawer';
import { useNativeBlockSurface } from '../native-block-surface-context';

const MISSING_SURFACE_MESSAGE =
  'Native ResizableDrawer container requires an active Block Surface runtime.';

export function NativeBlockResizableDrawer({
  getContainer,
  ...props
}: ResizableDrawerProps) {
  const surface = useNativeBlockSurface();
  if (getContainer === undefined && !surface) {
    throw new Error(MISSING_SURFACE_MESSAGE);
  }
  return (
    <SharedResizableDrawer
      {...props}
      getContainer={
        getContainer !== undefined
          ? getContainer
          : surface!.overlayHost.getPopupContainer
      }
    />
  );
}
