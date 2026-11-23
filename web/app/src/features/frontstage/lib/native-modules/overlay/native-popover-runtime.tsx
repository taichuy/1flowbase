import type { PopoverProps, TooltipRef } from 'antd';
import AntdPopover from 'antd/es/popover';
import { forwardRef, useCallback, useRef, useState } from 'react';

import { useNativeSurfacePopupContainer } from './native-surface-popup-container';
import {
  useSurfaceTooltipAnchor,
  useTooltipRef
} from './use-surface-tooltip-anchor';

const NativeBlockPopoverComponent = forwardRef<TooltipRef, PopoverProps>(
  function NativeBlockPopoverComponent(
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
      <AntdPopover
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

export const NativeBlockPopover = Object.assign(NativeBlockPopoverComponent, {
  _InternalPanelDoNotUseOrYouWillBeFired:
    AntdPopover._InternalPanelDoNotUseOrYouWillBeFired
}) as typeof AntdPopover;
