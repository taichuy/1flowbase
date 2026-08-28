import { message as AntdMessage } from 'antd';
import {
  useCallback,
  useLayoutEffect,
  useMemo,
  useRef,
  useState
} from 'react';

import { useNativeBlockSurface } from './native-block-surface-context';
import {
  createNativeOverlayLayer,
  type NativeOverlayLayer
} from './native-overlay-layer';

type MessageHookConfig = Parameters<typeof AntdMessage.useMessage>[0];
type MessageHookResult = ReturnType<typeof AntdMessage.useMessage>;
type MessageInstance = MessageHookResult[0];
type MessageResult = ReturnType<MessageInstance['open']>;

function useNativeBlockMessage(config?: MessageHookConfig): MessageHookResult {
  const surface = useNativeBlockSurface();
  const [layer, setLayer] = useState<NativeOverlayLayer | null>(null);
  const activeNotices = useRef(new Set<symbol>());
  const previousLayoutEpoch = useRef(surface?.layoutEpoch);
  const targetRoot = surface?.targetRoot;
  const configuredContainer = config?.getContainer;
  const usesNativeLayer = !!targetRoot && !configuredContainer;

  useLayoutEffect(() => {
    if (!usesNativeLayer || !targetRoot) {
      setLayer(null);
      return;
    }
    const blockId =
      targetRoot.host.getAttribute(
        'data-flowbase-native-trusted-block-id'
      ) ?? 'native-message';
    const nextLayer = createNativeOverlayLayer({ blockId, targetRoot });
    nextLayer.container.dataset.flowbaseNativeMessageLayer = '';
    setLayer(nextLayer);
    return () => nextLayer.dispose();
  }, [targetRoot, usesNativeLayer]);

  const getContainer = useCallback(
    () =>
      layer?.container ??
      configuredContainer?.() ??
      (targetRoot?.host as HTMLElement | undefined) ??
      document.body,
    [configuredContainer, layer, targetRoot]
  );
  const resolvedConfig = useMemo(
    () =>
      usesNativeLayer
        ? {
            ...config,
            getContainer
          }
        : config,
    [config, getContainer, usesNativeLayer]
  );
  const [messageApi, contextHolder] = AntdMessage.useMessage(resolvedConfig);

  const trackNotice = useCallback(
    (openNotice: () => MessageResult): MessageResult => {
      if (!layer) return openNotice();
      const notice = Symbol('native-message-notice');
      activeNotices.current.add(notice);
      layer.activate();
      const result = openNotice();
      const release = () => {
        activeNotices.current.delete(notice);
        if (activeNotices.current.size === 0) layer.deactivate();
      };
      void Promise.resolve(result).then(release, release);
      return result;
    },
    [layer]
  );

  const nativeMessageApi = useMemo<MessageInstance>(
    () => ({
      open: (...args: Parameters<MessageInstance['open']>) =>
        trackNotice(() => messageApi.open(...args)),
      info: (...args: Parameters<MessageInstance['info']>) =>
        trackNotice(() => messageApi.info(...args)),
      success: (...args: Parameters<MessageInstance['success']>) =>
        trackNotice(() => messageApi.success(...args)),
      error: (...args: Parameters<MessageInstance['error']>) =>
        trackNotice(() => messageApi.error(...args)),
      warning: (...args: Parameters<MessageInstance['warning']>) =>
        trackNotice(() => messageApi.warning(...args)),
      loading: (...args: Parameters<MessageInstance['loading']>) =>
        trackNotice(() => messageApi.loading(...args)),
      destroy: (key) => messageApi.destroy(key)
    }),
    [messageApi, trackNotice]
  );

  useLayoutEffect(() => {
    const nextLayoutEpoch = surface?.layoutEpoch;
    if (previousLayoutEpoch.current === nextLayoutEpoch) return;
    previousLayoutEpoch.current = nextLayoutEpoch;
    activeNotices.current.clear();
    messageApi.destroy();
    layer?.deactivate();
  }, [layer, messageApi, surface?.layoutEpoch]);

  useLayoutEffect(
    () => () => {
      activeNotices.current.clear();
      messageApi.destroy();
      layer?.deactivate();
    },
    [layer, messageApi]
  );

  return [nativeMessageApi, contextHolder];
}

export const NativeBlockMessage: typeof AntdMessage = {
  ...AntdMessage,
  useMessage: useNativeBlockMessage
};
