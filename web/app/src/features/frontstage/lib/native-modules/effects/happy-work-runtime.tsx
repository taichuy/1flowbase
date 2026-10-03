import DotEffect from '@ant-design/happy-work-theme/es/DotEffect';
import type { HappyProviderProps } from '@ant-design/happy-work-theme/es/HappyProvider';
import { ConfigProvider, type ConfigProviderProps } from 'antd';
import {
  useCallback,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ComponentProps,
  type PropsWithChildren
} from 'react';
import { createPortal } from 'react-dom';

import { useNativeBlockSurface } from '../native-block-surface-context';

type Wave = NonNullable<ConfigProviderProps['wave']>;
type Effect = Omit<ComponentProps<typeof DotEffect>, 'onFinish'> & {
  id: number;
};

// Reuse the pinned upstream animation, but keep React context and DOM ownership
// in the Block. Upstream HappyProvider creates an independent root on body.
export function HappyProvider({
  children,
  disabled
}: PropsWithChildren<HappyProviderProps>) {
  const surface = useNativeBlockSurface();
  const [effects, setEffects] = useState<Effect[]>([]);
  const sequence = useRef(0);
  const clear = useCallback(() => setEffects([]), []);
  const finish = useCallback((id: number) => {
    setEffects((current) => current.filter((effect) => effect.id !== id));
  }, []);
  const showEffect = useCallback<NonNullable<Wave['showEffect']>>(
    (target, { token, hashId }) => {
      if (!surface || target.getRootNode() !== surface.targetRoot) return;
      const effect = { target, token, hashId, id: ++sequence.current };
      setEffects((current) => [...current, effect]);
    },
    [surface]
  );
  const wave = useMemo<Wave>(
    () => (disabled ? {} : { showEffect }),
    [disabled, showEffect]
  );

  useLayoutEffect(() => {
    if (disabled) clear();
  }, [disabled, clear]);
  useLayoutEffect(
    () =>
      surface?.registerEffectResource({ invalidate: clear, dispose: clear }),
    [surface, clear]
  );

  if (!surface)
    throw new Error('HappyProvider requires a native Block surface.');
  return (
    <ConfigProvider wave={wave}>
      {children}
      {effects.map(({ id, ...effect }) =>
        createPortal(
          <div data-flowbase-happy-effect="" style={{ pointerEvents: 'none' }}>
            <DotEffect {...effect} onFinish={() => finish(id)} />
          </div>,
          surface.overlayHost.getPopupContainer(),
          String(id)
        )
      )}
    </ConfigProvider>
  );
}
