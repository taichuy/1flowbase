import { useEffect, type RefObject } from 'react';
import { canPrefetchPublicAuth } from './public-auth-loading';

// This is deliberately not a startup import or a fixed-delay timer. A real form
// must have committed; measurements can exclude this marked idle background work.
export function useAuthenticatedPrewarm(
  host: RefObject<HTMLElement | null>,
  identity: unknown
) {
  useEffect(() => {
    const target = host.current;
    if (!target || typeof window.requestIdleCallback !== 'function') return;
    let cancelled = false;
    let idle: number | undefined;
    let started = false;
    const form = () => target.querySelector('form');
    const observe = () => {
      if (!form()) {
        if (idle !== undefined) window.cancelIdleCallback(idle);
        idle = undefined;
        return;
      }
      if (started || idle !== undefined || !canPrefetchPublicAuth()) return;
      performance.mark?.('anonymous:form-committed');
      idle = window.requestIdleCallback(() => {
        idle = undefined;
        if (
          cancelled ||
          !target.isConnected ||
          !form() ||
          !canPrefetchPublicAuth()
        )
          return;
        started = true;
        performance.mark?.('anonymous:authenticated-prewarm-start');
        void import('../../../app/AuthenticatedAppRuntime').then(
          () => performance.mark?.('anonymous:authenticated-prewarm-end'),
          () => performance.mark?.('anonymous:authenticated-prewarm-failed')
        );
      });
    };
    const observer = new MutationObserver(observe);
    observer.observe(target, { childList: true, subtree: true });
    observe();
    return () => {
      cancelled = true;
      observer.disconnect();
      if (idle !== undefined) window.cancelIdleCallback(idle);
    };
  }, [host, identity]);
}
