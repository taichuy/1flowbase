import AntdSelect from 'antd/es/select';
import {
  forwardRef,
  type ComponentPropsWithoutRef,
  type ComponentRef
} from 'react';

import { useNativeSurfacePopupContainer } from './native-surface-popup-container';

const NativeBlockSelectComponent = forwardRef<
  ComponentRef<typeof AntdSelect>,
  ComponentPropsWithoutRef<typeof AntdSelect>
>(function NativeBlockSelectComponent({ getPopupContainer, ...props }, ref) {
  const popupContainer = useNativeSurfacePopupContainer(getPopupContainer);
  return <AntdSelect {...props} ref={ref} getPopupContainer={popupContainer} />;
});

export const NativeBlockSelect = Object.assign(NativeBlockSelectComponent, {
  _InternalPanelDoNotUseOrYouWillBeFired:
    AntdSelect._InternalPanelDoNotUseOrYouWillBeFired,
  OptGroup: AntdSelect.OptGroup,
  Option: AntdSelect.Option,
  SECRET_COMBOBOX_MODE_DO_NOT_USE: AntdSelect.SECRET_COMBOBOX_MODE_DO_NOT_USE
}) as typeof AntdSelect;
