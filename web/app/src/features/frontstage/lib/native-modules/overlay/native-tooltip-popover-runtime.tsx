import {
  Popover as AntdPopover,
  Tooltip as AntdTooltip,
  type PopoverProps,
  type TooltipProps,
  type TooltipRef
} from 'antd';
import {
  forwardRef,
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
  type Ref
} from 'react';

import { useNativeBlockSurface } from '../native-block-surface-context';
import { useNativeSurfacePopupContainer } from './native-surface-popup-container';

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

export const NativeBlockTooltip = Object.assign(NativeBlockTooltipComponent, {
  _InternalPanelDoNotUseOrYouWillBeFired:
    AntdTooltip._InternalPanelDoNotUseOrYouWillBeFired,
  UniqueProvider: AntdTooltip.UniqueProvider
}) as typeof AntdTooltip;

export const NativeBlockPopover = Object.assign(NativeBlockPopoverComponent, {
  _InternalPanelDoNotUseOrYouWillBeFired:
    AntdPopover._InternalPanelDoNotUseOrYouWillBeFired
}) as typeof AntdPopover;

function useSurfaceTooltipAnchor({
  active,
  tooltipRef
}: {
  active: boolean;
  tooltipRef: Readonly<{ current: TooltipRef | null }>;
}): void {
  const surface = useNativeBlockSurface();

  useLayoutEffect(() => {
    if (!active || !surface) return;
    return surface.registerAnchor({
      measure: () => tooltipRef.current,
      commit: (tooltip) => tooltip?.forceAlign()
    });
  }, [active, surface, tooltipRef]);
}

function useTooltipRef(
  tooltipRef: { current: TooltipRef | null },
  forwardedRef: Ref<TooltipRef>
): (value: TooltipRef | null) => void {
  return useCallback(
    (value: TooltipRef | null) => {
      tooltipRef.current = value;
      if (typeof forwardedRef === 'function') forwardedRef(value);
      else if (forwardedRef) forwardedRef.current = value;
    },
    [forwardedRef, tooltipRef]
  );
}
