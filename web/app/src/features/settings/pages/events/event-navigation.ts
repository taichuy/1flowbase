import { useSyncExternalStore } from 'react';

export type EventTab = 'catalog' | 'queues' | 'activity';

function subscribe(listener: () => void) {
  window.addEventListener('popstate', listener);
  return () => window.removeEventListener('popstate', listener);
}

export function useEventTab(): EventTab {
  const search = useSyncExternalStore(
    subscribe,
    () => window.location.search,
    () => ''
  );
  const tab = new URLSearchParams(search).get('tab');
  return tab === 'queues' || tab === 'activity' ? tab : 'catalog';
}

export function navigateEventTab(tab: EventTab, replace = false) {
  const url = new URL(window.location.href);
  url.searchParams.set('tab', tab);
  const href = `${url.pathname}${url.search}${url.hash}`;
  if (
    href ===
    `${window.location.pathname}${window.location.search}${window.location.hash}`
  )
    return;
  if (replace) window.history.replaceState(window.history.state, '', href);
  else window.history.pushState(window.history.state, '', href);
  window.dispatchEvent(new PopStateEvent('popstate'));
}
