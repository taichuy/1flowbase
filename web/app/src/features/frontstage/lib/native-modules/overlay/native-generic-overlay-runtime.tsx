import {
  Cascader as AntdCascader,
  DatePicker as AntdDatePicker,
  Select as AntdSelect,
  TreeSelect as AntdTreeSelect
} from 'antd';
import {
  forwardRef,
  type ComponentProps,
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

const NativeBlockSelectComponent = forwardRef<
  ComponentRef<typeof AntdSelect>,
  ComponentPropsWithoutRef<typeof AntdSelect>
>(function NativeBlockSelectComponent({ getPopupContainer, ...props }, ref) {
  const popupContainer = useNativeSurfacePopupContainer(getPopupContainer);
  return <AntdSelect {...props} ref={ref} getPopupContainer={popupContainer} />;
});

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

function NativeBlockDatePickerComponent({
  getPopupContainer,
  ...props
}: ComponentProps<typeof AntdDatePicker>) {
  const popupContainer = useNativeSurfacePopupContainer(getPopupContainer);
  return <AntdDatePicker {...props} getPopupContainer={popupContainer} />;
}

export const NativeBlockCascader = Object.assign(NativeBlockCascaderComponent, {
  _InternalPanelDoNotUseOrYouWillBeFired:
    AntdCascader._InternalPanelDoNotUseOrYouWillBeFired,
  Panel: AntdCascader.Panel,
  SHOW_CHILD: AntdCascader.SHOW_CHILD,
  SHOW_PARENT: AntdCascader.SHOW_PARENT
}) as typeof AntdCascader;

export const NativeBlockSelect = Object.assign(NativeBlockSelectComponent, {
  _InternalPanelDoNotUseOrYouWillBeFired:
    AntdSelect._InternalPanelDoNotUseOrYouWillBeFired,
  OptGroup: AntdSelect.OptGroup,
  Option: AntdSelect.Option,
  SECRET_COMBOBOX_MODE_DO_NOT_USE: AntdSelect.SECRET_COMBOBOX_MODE_DO_NOT_USE
}) as typeof AntdSelect;

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

export const NativeBlockDatePicker = Object.assign(
  NativeBlockDatePickerComponent,
  {
    _InternalPanelDoNotUseOrYouWillBeFired:
      AntdDatePicker._InternalPanelDoNotUseOrYouWillBeFired,
    _InternalRangePanelDoNotUseOrYouWillBeFired:
      AntdDatePicker._InternalRangePanelDoNotUseOrYouWillBeFired,
    generatePicker: AntdDatePicker.generatePicker,
    MonthPicker: AntdDatePicker.MonthPicker,
    QuarterPicker: AntdDatePicker.QuarterPicker,
    RangePicker: AntdDatePicker.RangePicker,
    TimePicker: AntdDatePicker.TimePicker,
    WeekPicker: AntdDatePicker.WeekPicker,
    YearPicker: AntdDatePicker.YearPicker
  }
) as typeof AntdDatePicker;
