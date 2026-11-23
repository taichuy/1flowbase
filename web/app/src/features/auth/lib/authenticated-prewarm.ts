import { useEffect, type RefObject } from 'react';
import authenticatedRuntimeUrl from 'virtual:1flowbase-authenticated-preload';
import {
  resolveRuntimeConcurrencyProfile,
  type RuntimeConcurrencyProfile
} from '../../../shared/utils/runtime-concurrency-profile';
import { canPrefetchPublicAuth } from './public-auth-loading';

let prewarmEpoch = 0;
const pendingDeltaLinks = new Set<HTMLLinkElement>();

function hasModulePreload(resolvedHref: string): boolean {
  for (const el of document.querySelectorAll<HTMLLinkElement>('link[rel="modulepreload"]')) {
    if (el.href === resolvedHref) return true;
  }
  return false;
}

function createLowPriorityPreload(href: string): HTMLLinkElement {
  const link = document.createElement('link');
  link.rel = 'modulepreload';
  link.href = href;
  link.crossOrigin = 'anonymous';
  link.fetchPriority = 'low';
  return link;
}

export function pauseAuthenticatedPrewarm(
  options: { removePendingLinks?: boolean } = {}
): void {
  prewarmEpoch += 1;
  if (
    options.removePendingLinks ??
    resolveRuntimeConcurrencyProfile().removePendingPrewarmOnSubmit
  ) {
    for (const link of pendingDeltaLinks) link.remove();
  }
  pendingDeltaLinks.clear();
}

function prioritizeActiveLocaleInQueue(queue: readonly string[]): string[] {
  const docLang = typeof document !== 'undefined' ? document.documentElement.lang : '';
  const rawLang =
    docLang && !/^en$/iu.test(docLang)
      ? docLang
      : (typeof navigator !== 'undefined' ? navigator.language : '') || docLang;
  const inactiveRe = /^en\b/iu.test(rawLang)
    ? /\/i18n-shell-zh-[^/]+\.js$/u
    : /\/i18n-shell-en-[^/]+\.js$/u;
  const primary: string[] = [];
  const tail: string[] = [];
  let inTail = false;
  for (const href of queue) {
    if (inactiveRe.test(href)) continue;
    if (/\/(?:frontstage-canvas|native-host)-/u.test(href)) inTail = true;
    (inTail ? tail : primary).push(href);
  }
  return [...primary, ...tail];
}

function streamTransitionDeltaPreloads(
  isActive: () => boolean,
  epoch: number,
  profile: RuntimeConcurrencyProfile
): void {
  if (typeof window.fetch !== 'function') return;
  void window
    .fetch('/authenticated-prewarm.json', { credentials: 'same-origin' })
    .then(async (res) => {
      if (!res.ok || !isActive() || prewarmEpoch !== epoch) return;
      const manifest = (await res.json()) as { files?: readonly string[] };
      if (!isActive() || prewarmEpoch !== epoch || !Array.isArray(manifest?.files)) return;
      const seen = new Set<string>();
      const rawQueue: string[] = [];
      for (const f of manifest.files) {
        if (f.endsWith('.js') && !seen.has(f)) {
          seen.add(f);
          rawQueue.push(`/${f.replace(/^\/+/u, '')}`);
        }
      }
      const ordered = prioritizeActiveLocaleInQueue(rawQueue);
      let cursor = 0;
      let inFlight = 0;
      const limit = Math.max(1, profile.prewarmConcurrencyLimit);
      const pump = () => {
        while (
          inFlight < limit &&
          cursor < ordered.length &&
          isActive() &&
          prewarmEpoch === epoch &&
          canPrefetchPublicAuth()
        ) {
          const href = ordered[cursor++]!;
          if (hasModulePreload(new URL(href, document.baseURI).href)) continue;
          inFlight += 1;
          const link = createLowPriorityPreload(href);
          pendingDeltaLinks.add(link);
          const settle = () => {
            pendingDeltaLinks.delete(link);
            inFlight = Math.max(0, inFlight - 1);
            pump();
          };
          link.addEventListener('load', settle, { once: true });
          link.addEventListener('error', settle, { once: true });
          document.head.append(link);
        }
      };
      pump();
    })
    .catch(() => undefined);
}

export function useAuthenticatedPrewarm(
  host: RefObject<HTMLElement | null>,
  identity: unknown,
  concurrencyProfile?: RuntimeConcurrencyProfile
) {
  useEffect(() => {
    const target = host.current;
    if (!target || typeof window.requestIdleCallback !== 'function') return;
    const profile = concurrencyProfile ?? resolveRuntimeConcurrencyProfile();
    let cancelled = false;
    let idle: number | undefined;
    let started = false;
    const epoch = prewarmEpoch;
    const isActive = () => !cancelled && target.isConnected;
    const form = () => target.querySelector('form');
    const onSubmit = () =>
      pauseAuthenticatedPrewarm({
        removePendingLinks: profile.removePendingPrewarmOnSubmit
      });
    target.addEventListener('submit', onSubmit, true);
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
        if (!isActive() || !form() || !canPrefetchPublicAuth()) return;
        started = true;
        const link = createLowPriorityPreload(authenticatedRuntimeUrl);
        if (hasModulePreload(link.href)) return;
        link.addEventListener(
          'load',
          () => {
            performance.mark?.('anonymous:authenticated-prewarm-end');
            streamTransitionDeltaPreloads(isActive, epoch, profile);
          },
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
      target.removeEventListener('submit', onSubmit, true);
      observer.disconnect();
      if (idle !== undefined) window.cancelIdleCallback(idle);
    };
  }, [host, identity, concurrencyProfile]);
}
