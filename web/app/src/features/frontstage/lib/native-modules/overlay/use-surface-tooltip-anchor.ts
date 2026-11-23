import type { TooltipRef } from 'antd';
import { useCallback, useLayoutEffect, type Ref } from 'react';

import { useNativeBlockSurface } from '../native-block-surface-context';

export function useSurfaceTooltipAnchor({
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
      target: () => tooltipRef.current?.nativeElement ?? null,
      measure: () => tooltipRef.current,
      commit: (tooltip) => tooltip?.forceAlign()
    });
  }, [active, surface, tooltipRef]);
}

export function useTooltipRef(
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
