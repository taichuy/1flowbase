import AntdTreeSelect from 'antd/es/tree-select';
import {
  forwardRef,
  type ComponentPropsWithoutRef,
  type ComponentRef
} from 'react';

import { useNativeSurfacePopupContainer } from './native-surface-popup-container';

const NativeBlockTreeSelectComponent = forwardRef<
  ComponentRef<typeof AntdTreeSelect>,
  ComponentPropsWithoutRef<typeof AntdTreeSelect>
>(function NativeBlockTreeSelectComponent(
  { getPopupContainer, ...props },
  ref
) {
  const popupContainer = useNativeSurfacePopupContainer(getPopupContainer);
  return (
    <AntdTreeSelect {...props} ref={ref} getPopupContainer={popupContainer} />
  );
});

export const NativeBlockTreeSelect = Object.assign(
  NativeBlockTreeSelectComponent,
  {
    _InternalPanelDoNotUseOrYouWillBeFired:
      AntdTreeSelect._InternalPanelDoNotUseOrYouWillBeFired,
    SHOW_ALL: AntdTreeSelect.SHOW_ALL,
    SHOW_CHILD: AntdTreeSelect.SHOW_CHILD,
    SHOW_PARENT: AntdTreeSelect.SHOW_PARENT,
    TreeNode: AntdTreeSelect.TreeNode
  }
) as typeof AntdTreeSelect;
