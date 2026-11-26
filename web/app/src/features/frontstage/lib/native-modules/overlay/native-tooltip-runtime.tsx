import type { TooltipProps, TooltipRef } from 'antd';
import AntdTooltip from 'antd/es/tooltip';
import { forwardRef, useCallback, useRef, useState } from 'react';

import { useNativeSurfacePopupContainer } from './native-surface-popup-container';
import {
  useSurfaceTooltipAnchor,
  useTooltipRef
} from './use-surface-tooltip-anchor';

const NativeBlockTooltipComponent = forwardRef<TooltipRef, TooltipProps>(
  function NativeBlockTooltipComponent(
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
      <AntdTooltip
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

export const NativeBlockTooltip = Object.assign(NativeBlockTooltipComponent, {
  _InternalPanelDoNotUseOrYouWillBeFired:
    AntdTooltip._InternalPanelDoNotUseOrYouWillBeFired,
  UniqueProvider: AntdTooltip.UniqueProvider
}) as typeof AntdTooltip;
