import type AntdDatePickerType from 'antd/es/date-picker';
import {
  forwardRef,
  useEffect,
  useState,
  type ComponentPropsWithoutRef,
  type ComponentRef,
  type ComponentType
} from 'react';

import { useNativeSurfacePopupContainer } from './native-surface-popup-container';

type AntdDatePickerModule = typeof AntdDatePickerType;

let cachedDatePicker: AntdDatePickerModule | null = null;
let datePickerFlight: Promise<AntdDatePickerModule> | null = null;

function loadDatePickerModule(): Promise<AntdDatePickerModule> {
  if (cachedDatePicker) return Promise.resolve(cachedDatePicker);
  datePickerFlight ??= import('antd/es/date-picker')
    .then((mod) => {
      cachedDatePicker = mod.default;
      return mod.default;
    })
    .catch((err) => {
      datePickerFlight = null;
      throw err;
    });
  return datePickerFlight;
}

function useMountedDatePicker(): AntdDatePickerModule | null {
  const [impl, setImpl] = useState<AntdDatePickerModule | null>(
    () => cachedDatePicker
  );

  useEffect(() => {
    if (impl) return undefined;
    let active = true;
    void loadDatePickerModule()
      .then((loaded) => {
        if (active) setImpl(() => loaded);
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [impl]);

  return impl;
}

const NativeBlockDatePickerComponent = forwardRef<
  ComponentRef<AntdDatePickerModule>,
  ComponentPropsWithoutRef<AntdDatePickerModule>
>(function NativeBlockDatePickerComponent(
  { getPopupContainer, ...props },
  ref
) {
  const popupContainer = useNativeSurfacePopupContainer(getPopupContainer);
  const Impl = useMountedDatePicker();
  if (!Impl) return null;
  return <Impl {...props} ref={ref} getPopupContainer={popupContainer} />;
});

function createSubPickerFacade<
  Key extends
    | 'RangePicker'
    | 'MonthPicker'
    | 'QuarterPicker'
    | 'TimePicker'
    | 'WeekPicker'
    | 'YearPicker'
>(subKey: Key) {
  return forwardRef<unknown, Record<string, unknown>>(
    function NativeBlockSubDatePicker(props, ref) {
      const pickerModule = useMountedDatePicker();
      if (!pickerModule) return null;
      const SubImpl = pickerModule[subKey] as unknown as ComponentType<
        Record<string, unknown>
      >;
      return <SubImpl {...props} ref={ref} />;
    }
  );
}

export const NativeBlockDatePicker = Object.assign(
  NativeBlockDatePickerComponent,
  {
    MonthPicker: createSubPickerFacade('MonthPicker'),
    QuarterPicker: createSubPickerFacade('QuarterPicker'),
    RangePicker: createSubPickerFacade('RangePicker'),
    TimePicker: createSubPickerFacade('TimePicker'),
    WeekPicker: createSubPickerFacade('WeekPicker'),
    YearPicker: createSubPickerFacade('YearPicker')
  }
) as unknown as AntdDatePickerModule;
