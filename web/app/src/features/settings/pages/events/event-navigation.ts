import { useSyncExternalStore } from 'react';

export type EventTab = 'catalog' | 'queues' | 'activity';

function subscribe(listener: () => void) {
  window.addEventListener('popstate', listener);
  return () => window.removeEventListener('popstate', listener);
}

export function useEventParameter(key: string): string | null {
  const search = useSyncExternalStore(
    subscribe,
    () => window.location.search,
    () => ''
  );
  return new URLSearchParams(search).get(key);
}

export function navigateEventState(
  values: Record<string, string | null>,
  replace = false
) {
  const url = new URL(window.location.href);
  for (const [key, value] of Object.entries(values)) {
    if (value === null || value === '') url.searchParams.delete(key);
    else url.searchParams.set(key, value);
  }
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
  if (tab !== 'activity') url.searchParams.delete('observation');
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

export function useEventScope(): string | null {
  const search = useSyncExternalStore(
    subscribe,
    () => window.location.search,
    () => ''
  );
  return new URLSearchParams(search).get('scope');
}
export function navigateEventScope(scope: string) {
  navigateEventState({
    scope,
    // Clear stale URL parameters from the retired source-inspector UI.
    source: null,
    queue: null,
    source_details: null,
    source_observation: null,
    record: null,
    workflow_queue: null,
    queues_page: null,
    activity_page: null,
    event: null,
    subscription: null,
    queue_query: null,
    'event-groups_page': null
  });
}
