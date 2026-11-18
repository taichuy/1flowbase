import { Modal, type ModalProps } from 'antd';
import { useNativeBlockSurface } from '../../../lib/native-modules/native-block-surface-context';

/** Keep local settings in the owning native surface's overlay layer and style cache. */
export function DataBlockModal(props: ModalProps) {
  const surface = useNativeBlockSurface();
  return (
    <Modal
      {...props}
      getContainer={
        surface
          ? () => surface.overlayHost.getPopupContainer()
          : (props.getContainer ?? false)
      }
    />
  );
}
