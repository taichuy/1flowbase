import { useEffect, type RefObject } from 'react';
import authenticatedRuntimeUrl from 'virtual:1flowbase-authenticated-preload';
import { canPrefetchPublicAuth } from './public-auth-loading';

// This is deliberately not a startup import or a fixed-delay timer. A real form
// must have committed; measurements can exclude this marked idle background work.
// Only preload the URL: importing here would initialize authenticated providers.
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
        const link = document.createElement('link');
        link.rel = 'modulepreload';
        link.href = authenticatedRuntimeUrl;
        // Keep a completed/in-flight hint across entry changes and remounts.
        if (
          [
            ...document.querySelectorAll<HTMLLinkElement>(
              'link[rel="modulepreload"]'
            )
          ].some((existing) => existing.href === link.href)
        )
          return;
        link.crossOrigin = 'anonymous';
        link.fetchPriority = 'low';
        link.addEventListener(
          'load',
          () => performance.mark?.('anonymous:authenticated-prewarm-end'),
          { once: true }
        );
        link.addEventListener(
          'error',
          () => performance.mark?.('anonymous:authenticated-prewarm-failed'),
          { once: true }
        );
        performance.mark?.('anonymous:authenticated-prewarm-start');
        document.head.append(link);
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
