import { notification as AntdNotification } from 'antd';
import {
  Suspense,
  startTransition,
  useEffect,
  useLayoutEffect,
  useSyncExternalStore,
  type ReactNode
} from 'react';
import { useNativeBlockSurface } from '../native-block-surface-context';
import { NativeBlockNotification } from '../native-notification-runtime';

type Api = ReturnType<typeof AntdNotification.useNotification>[0];
type Config = Parameters<typeof AntdNotification.config>[0];
const methods = ['open', 'success', 'info', 'warning', 'error'] as const;

/** Static syntax backed by one mounted Block's hook instance and defaults. */
export function createBlockNotificationRuntime() {
  let active = false;
  let api: Api | undefined;
  let config: Config = {};
  const listeners = new Set<() => void>();
  const subscribe = (listener: () => void) => {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  };
  const snapshot = () => config;
  const destroy: Api['destroy'] = (key) => {
    if (active) api?.destroy(key);
  };
  function Lifecycle() {
    const surface = useNativeBlockSurface();
    useLayoutEffect(() => {
      active = true;
    }, []);
    useEffect(() => {
      active = true;
      const dispose = () => {
        active = false;
        api?.destroy();
      };
      const unregister = surface?.registerEffectResource({
        invalidate: () => api?.destroy(),
        dispose
      });
      return () => {
        unregister?.();
        dispose();
      };
    }, [surface]);
    return null;
  }
  function Provider({ children }: { children: ReactNode }) {
    const surface = useNativeBlockSurface();
    const defaults = useSyncExternalStore(subscribe, snapshot, snapshot);
    const [instance, holder] = AntdNotification.useNotification({
      ...defaults,
      getContainer: () => {
        if (!surface)
          throw new Error('Notification requires a mounted Block surface.');
        return surface.overlayHost.getPopupContainer();
      }
    });
    api = instance;
    return (
      <>
        <Suspense fallback={null}>{holder}</Suspense>
        <Lifecycle />
        {children}
      </>
    );
  }
  const notification: typeof AntdNotification = {
    ...NativeBlockNotification,
    ...Object.fromEntries(
      methods.map((method) => [
        method,
        (args: Parameters<Api['open']>[0]) => {
          if (!active) return;
          startTransition(() => api?.[method]({ ...config, ...args }));
        }
      ])
    ),
    destroy,
    config(next) {
      config = { ...config, ...next };
      for (const listener of listeners) listener();
    }
  };
  return { notification, Provider };
}
