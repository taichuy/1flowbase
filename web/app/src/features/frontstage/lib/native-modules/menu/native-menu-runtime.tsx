import { Menu as AntdMenu, type MenuProps, type MenuRef } from 'antd';
import {
  forwardRef,
  useCallback,
  useLayoutEffect,
  useRef,
  useState
} from 'react';

import { useNativeBlockSurface } from '../native-block-surface-context';

const NativeBlockMenuBase = forwardRef<MenuRef, MenuProps>(
  function NativeBlockMenu(
    { defaultOpenKeys, getPopupContainer, onOpenChange, openKeys, ...props },
    ref
  ) {
    const surface = useNativeBlockSurface();
    const [uncontrolledOpenKeys, setUncontrolledOpenKeys] = useState<string[]>(
      () => defaultOpenKeys ?? []
    );
    const previousLayoutEpoch = useRef(surface?.layoutEpoch);
    const controlled = openKeys !== undefined;
    const resolvedOpenKeys = controlled ? openKeys : uncontrolledOpenKeys;
    const overlayHost = surface?.overlayHost;
    const targetRoot = surface?.targetRoot;
    const usesNativeLayer = !!overlayHost && !getPopupContainer;

    useLayoutEffect(() => {
      const nextLayoutEpoch = surface?.layoutEpoch;
      if (previousLayoutEpoch.current === nextLayoutEpoch) return;
      previousLayoutEpoch.current = nextLayoutEpoch;
      if (controlled) return;
      setUncontrolledOpenKeys([]);
    }, [controlled, surface?.layoutEpoch]);

    const transitionOpenKeys = useCallback(
      (nextOpenKeys: string[]) => {
        if (!controlled) setUncontrolledOpenKeys(nextOpenKeys);
        onOpenChange?.(nextOpenKeys);
      },
      [controlled, onOpenChange]
    );
    const resolvePopupContainer = useCallback(
      (triggerNode: HTMLElement) =>
        overlayHost?.container ??
        getPopupContainer?.(triggerNode) ??
        (targetRoot?.host as HTMLElement | undefined) ??
        triggerNode.ownerDocument.body,
      [getPopupContainer, overlayHost, targetRoot]
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
