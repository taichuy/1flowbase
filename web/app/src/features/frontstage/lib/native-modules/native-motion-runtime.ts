import type { ConfigProviderProps } from 'antd/es/config-provider';
import { useMemo, useSyncExternalStore } from 'react';

const directMotionTokens = {
  motion: false,
  motionDurationFast: '0s',
  motionDurationMid: '0s',
  motionDurationSlow: '0s'
} as const;

const reducedMotionListeners = new Set<() => void>();
let reducedMotionQuery: MediaQueryList | null = null;

export function useNativeBlockMotionTheme(
  authoredTheme: ConfigProviderProps['theme']
): NonNullable<ConfigProviderProps['theme']> {
  const prefersReducedMotion = useSyncExternalStore(
    subscribeToReducedMotion,
    readReducedMotion,
    () => false
  );

  return useMemo(
    () => ({
      ...authoredTheme,
      token: {
        ...directMotionTokens,
        ...authoredTheme?.token,
        ...(prefersReducedMotion ? directMotionTokens : null)
      }
    }),
    [authoredTheme, prefersReducedMotion]
  );
}

function subscribeToReducedMotion(listener: () => void): () => void {
  const query = getReducedMotionQuery();
  if (!query) return () => undefined;
  reducedMotionListeners.add(listener);
  if (reducedMotionListeners.size === 1) {
    query.addEventListener('change', notifyReducedMotionListeners);
  }
  return () => {
    reducedMotionListeners.delete(listener);
    if (reducedMotionListeners.size === 0) {
      query.removeEventListener('change', notifyReducedMotionListeners);
      reducedMotionQuery = null;
    }
  };
}

function readReducedMotion(): boolean {
  return getReducedMotionQuery()?.matches ?? false;
}

function getReducedMotionQuery(): MediaQueryList | null {
  if (
    typeof window === 'undefined' ||
    typeof window.matchMedia !== 'function'
  ) {
    return null;
  }
  reducedMotionQuery ??= window.matchMedia(
    '(prefers-reduced-motion: reduce)'
  );
  return reducedMotionQuery;
}

function notifyReducedMotionListeners(): void {
  reducedMotionListeners.forEach((listener) => listener());
}
