import type { PopconfirmProps, TooltipRef } from 'antd';
import AntdPopconfirm from 'antd/es/popconfirm';
import { forwardRef, useCallback, useRef, useState } from 'react';

import { useNativeSurfacePopupContainer } from './native-surface-popup-container';
import {
  useSurfaceTooltipAnchor,
  useTooltipRef
} from './use-surface-tooltip-anchor';

const NativeBlockPopconfirmComponent = forwardRef<TooltipRef, PopconfirmProps>(
  function NativeBlockPopconfirmComponent(
    { defaultOpen, getPopupContainer, onOpenChange, open, ...props },
    forwardedRef
  ) {
    const popupContainer = useNativeSurfacePopupContainer(getPopupContainer);
    const [observedOpen, setObservedOpen] = useState(defaultOpen ?? false);
    const tooltipRef = useRef<TooltipRef | null>(null);
    const mergedRef = useTooltipRef(tooltipRef, forwardedRef);
    useSurfaceTooltipAnchor({
      active: (open ?? observedOpen) && !getPopupContainer,
      tooltipRef
    });
    const handleOpenChange = useCallback(
      (nextOpen: boolean) => {
        setObservedOpen(nextOpen);
        onOpenChange?.(nextOpen);
      },
      [onOpenChange]
    );

    return (
      <AntdPopconfirm
        {...props}
        ref={mergedRef}
        defaultOpen={defaultOpen}
        getPopupContainer={popupContainer}
        onOpenChange={handleOpenChange}
        open={open}
      />
    );
  }
);

export const NativeBlockPopconfirm = Object.assign(
  NativeBlockPopconfirmComponent,
  {
    _InternalPanelDoNotUseOrYouWillBeFired:
      AntdPopconfirm._InternalPanelDoNotUseOrYouWillBeFired
  }
) as typeof AntdPopconfirm;
