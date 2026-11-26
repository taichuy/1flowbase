import AntdCascader from 'antd/es/cascader';
import {
  forwardRef,
  type ComponentPropsWithoutRef,
  type ComponentRef
} from 'react';

import { useNativeSurfacePopupContainer } from './native-surface-popup-container';

const NativeBlockCascaderComponent = forwardRef<
  ComponentRef<typeof AntdCascader>,
  ComponentPropsWithoutRef<typeof AntdCascader>
>(function NativeBlockCascaderComponent({ getPopupContainer, ...props }, ref) {
  const popupContainer = useNativeSurfacePopupContainer(getPopupContainer);
  return (
    <AntdCascader {...props} ref={ref} getPopupContainer={popupContainer} />
  );
});

export const NativeBlockCascader = Object.assign(NativeBlockCascaderComponent, {
  _InternalPanelDoNotUseOrYouWillBeFired:
    AntdCascader._InternalPanelDoNotUseOrYouWillBeFired,
  Panel: AntdCascader.Panel,
  SHOW_CHILD: AntdCascader.SHOW_CHILD,
  SHOW_PARENT: AntdCascader.SHOW_PARENT
}) as typeof AntdCascader;
