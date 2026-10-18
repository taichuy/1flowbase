import { App as AntdApp } from 'antd';
import { useLayoutEffect, type ReactNode } from 'react';
import { useNativeBlockSurface } from '../native-block-surface-context';

export function NativeBlockAntdEffectScope({
  children
}: {
  children: ReactNode;
}): ReactNode {
  const surface = useNativeBlockSurface();
  const { notification } = AntdApp.useApp();

  useLayoutEffect(() => {
    if (!surface) return;
    const unregister = surface.registerEffectResource({
      invalidate: () => notification.destroy(),
      dispose: () => notification.destroy()
    });
    return () => {
      unregister();
      notification.destroy();
    };
  }, [notification, surface]);

  return children;
}
