import { App as AntdApp, notification as AntdNotification } from 'antd';
import { useLayoutEffect, type ReactNode } from 'react';

import { useNativeBlockSurface } from './native-block-surface-context';

type NotificationHookConfig = Parameters<
  typeof AntdNotification.useNotification
>[0];
type NotificationHookResult = ReturnType<
  typeof AntdNotification.useNotification
>;

function useNativeBlockNotification(
  config?: NotificationHookConfig
): NotificationHookResult {
  const surface = useNativeBlockSurface();
  const [api, holder] = AntdNotification.useNotification(config);

  useLayoutEffect(() => {
    if (!surface) return;
    const unregister = surface.registerEffectResource({
      invalidate: () => api.destroy(),
      dispose: () => api.destroy()
    });
    return () => {
      unregister();
      api.destroy();
    };
  }, [api, surface]);

  return [api, holder];
}

function denyStaticNotification(method: string): never {
  throw new Error(
    `AntD static notification method '${method}' is unavailable in a native Block. Use App.useApp() or notification.useNotification().`
  );
}

export const NativeBlockNotification: typeof AntdNotification = {
  ...AntdNotification,
  config: () => denyStaticNotification('config'),
  destroy: () => denyStaticNotification('destroy'),
  error: () => denyStaticNotification('error'),
  info: () => denyStaticNotification('info'),
  open: () => denyStaticNotification('open'),
  success: () => denyStaticNotification('success'),
  useNotification: useNativeBlockNotification,
  warning: () => denyStaticNotification('warning')
};

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
