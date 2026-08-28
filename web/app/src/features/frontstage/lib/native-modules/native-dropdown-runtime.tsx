import { Dropdown as AntdDropdown, type DropdownProps } from 'antd';
import {
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode
} from 'react';

import { useNativeBlockSurface } from './native-block-surface-context';
import {
  createNativeOverlayLayer,
  type NativeOverlayLayer
} from './native-overlay-layer';

export function NativeBlockDropdown({
  children,
  disabled,
  getPopupContainer,
  onOpenChange,
  open,
  trigger,
  ...props
}: DropdownProps): ReactNode {
  const surface = useNativeBlockSurface();
  const [layer, setLayer] = useState<NativeOverlayLayer | null>(null);
  const [uncontrolledOpen, setUncontrolledOpen] = useState(false);
  const [overlayGeneration, setOverlayGeneration] = useState(0);
  const previousLayoutEpoch = useRef(surface?.layoutEpoch);
  const controlled = open !== undefined;
  const resolvedOpen = controlled ? open : uncontrolledOpen;
  const resolvedOpenRef = useRef(resolvedOpen);
  resolvedOpenRef.current = resolvedOpen;
  const targetRoot = surface?.targetRoot;
  const usesNativeLayer = !!targetRoot && !getPopupContainer;

  useLayoutEffect(() => {
    if (!usesNativeLayer || !targetRoot) {
      setLayer(null);
      return;
    }
    const blockId =
      targetRoot.host.getAttribute(
        'data-flowbase-native-trusted-block-id'
      ) ?? 'native-dropdown';
    const nextLayer = createNativeOverlayLayer({
      blockId,
      targetRoot
    });
    setLayer(nextLayer);
    return () => nextLayer.dispose();
  }, [targetRoot, usesNativeLayer]);

  useLayoutEffect(() => {
    const nextLayoutEpoch = surface?.layoutEpoch;
    if (previousLayoutEpoch.current === nextLayoutEpoch) return;
    previousLayoutEpoch.current = nextLayoutEpoch;
    layer?.deactivate();
    if (!controlled) {
      resolvedOpenRef.current = false;
      setUncontrolledOpen(false);
    }
    setOverlayGeneration((generation) => generation + 1);
  }, [controlled, layer, surface?.layoutEpoch]);

  useLayoutEffect(() => {
    if (!layer) return;
    if (resolvedOpen) layer.activate();
    else layer.deactivate();
  }, [layer, overlayGeneration, resolvedOpen]);

  const transitionOpen = useCallback(
    (nextOpen: boolean, info: { source: 'trigger' | 'menu' }) => {
      if (resolvedOpenRef.current === nextOpen) return;
      resolvedOpenRef.current = nextOpen;
      if (!controlled) setUncontrolledOpen(nextOpen);
      if (nextOpen) layer?.activate();
      else layer?.deactivate();
      onOpenChange?.(nextOpen, info);
    },
    [controlled, layer, onOpenChange]
  );
  const resolvePopupContainer = useCallback(
    (triggerNode: HTMLElement) =>
      layer?.container ??
      getPopupContainer?.(triggerNode) ??
      (targetRoot?.host as HTMLElement | undefined) ??
      triggerNode.ownerDocument.body,
    [getPopupContainer, layer, targetRoot]
  );
  const hoverTrigger = (trigger ?? ['hover']).includes('hover');
  const dropdown = (
    <AntdDropdown
      {...props}
      key={overlayGeneration}
      disabled={disabled}
      getPopupContainer={resolvePopupContainer}
      onOpenChange={transitionOpen}
      open={resolvedOpen}
      trigger={trigger}
    >
      {children}
    </AntdDropdown>
  );
  return hoverTrigger && !disabled ? (
    <span
      data-flowbase-native-dropdown-intent=""
      style={{ display: 'contents' }}
      onPointerOverCapture={() => transitionOpen(true, { source: 'trigger' })}
    >
      {dropdown}
    </span>
  ) : (
    dropdown
  );
}
