import { Menu as AntdMenu, type MenuProps, type MenuRef } from 'antd';
import {
  forwardRef,
  useCallback,
  useLayoutEffect,
  useRef,
  useState
} from 'react';

import { useNativeBlockSurface } from '../native-block-surface-context';
import {
  createNativeOverlayLayer,
  type NativeOverlayLayer
} from '../native-overlay-layer';

const NativeBlockMenuBase = forwardRef<MenuRef, MenuProps>(
  function NativeBlockMenu(
    { defaultOpenKeys, getPopupContainer, onOpenChange, openKeys, ...props },
    ref
  ) {
    const surface = useNativeBlockSurface();
    const [layer, setLayer] = useState<NativeOverlayLayer | null>(null);
    const [uncontrolledOpenKeys, setUncontrolledOpenKeys] = useState<string[]>(
      () => defaultOpenKeys ?? []
    );
    const previousLayoutEpoch = useRef(surface?.layoutEpoch);
    const controlled = openKeys !== undefined;
    const resolvedOpenKeys = controlled ? openKeys : uncontrolledOpenKeys;
    const targetRoot = surface?.targetRoot;
    const usesNativeLayer = !!targetRoot && !getPopupContainer;

    useLayoutEffect(() => {
      if (!usesNativeLayer || !targetRoot) {
        setLayer(null);
        return;
      }
      const blockId =
        targetRoot.host.getAttribute('data-flowbase-native-trusted-block-id') ??
        'native-menu';
      const nextLayer = createNativeOverlayLayer({ blockId, targetRoot });
      setLayer(nextLayer);
      return () => nextLayer.dispose();
    }, [targetRoot, usesNativeLayer]);

    useLayoutEffect(() => {
      if (!layer) return;
      if (resolvedOpenKeys.length > 0) layer.activate();
      else layer.deactivate();
    }, [layer, resolvedOpenKeys]);

    useLayoutEffect(() => {
      const nextLayoutEpoch = surface?.layoutEpoch;
      if (previousLayoutEpoch.current === nextLayoutEpoch) return;
      previousLayoutEpoch.current = nextLayoutEpoch;
      if (controlled) return;
      layer?.deactivate();
      setUncontrolledOpenKeys([]);
    }, [controlled, layer, surface?.layoutEpoch]);

    const transitionOpenKeys = useCallback(
      (nextOpenKeys: string[]) => {
        if (nextOpenKeys.length > 0) layer?.activate();
        else layer?.deactivate();
        if (!controlled) setUncontrolledOpenKeys(nextOpenKeys);
        onOpenChange?.(nextOpenKeys);
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

    return (
      <AntdMenu
        {...props}
        ref={ref}
        getPopupContainer={
          usesNativeLayer ? resolvePopupContainer : getPopupContainer
        }
        onOpenChange={transitionOpenKeys}
        openKeys={resolvedOpenKeys}
      />
    );
  }
);

export const NativeBlockMenu = Object.assign(NativeBlockMenuBase, {
  Item: AntdMenu.Item,
  SubMenu: AntdMenu.SubMenu,
  Divider: AntdMenu.Divider,
  ItemGroup: AntdMenu.ItemGroup
}) as unknown as typeof AntdMenu;
