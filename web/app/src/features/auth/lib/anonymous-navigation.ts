import { useSyncExternalStore } from 'react';

export const publicNavigationEvent = '1flowbase:public-navigation';

export function subscribePublicLocation(listener: () => void) {
  window.addEventListener('popstate', listener);
  window.addEventListener(publicNavigationEvent, listener);
  return () => {
    window.removeEventListener('popstate', listener);
    window.removeEventListener(publicNavigationEvent, listener);
  };
}

function publicLocation() {
  return (
    window.location.pathname + window.location.search + window.location.hash
  );
}

export function usePublicLocation() {
  return useSyncExternalStore(
    subscribePublicLocation,
    publicLocation,
    () => '/sign-in'
  );
}

export function navigatePublicRoute({
  to,
  search,
  replace = false
}: {
  to: '/' | '/sign-in';
  search?: { login_entry_id?: string };
  replace?: boolean;
}) {
  // Entry selection changes just the entry; language and external-auth feedback
  // survive selection and Back/Forward. Successful authentication clears them.
  const url = new URL(to, window.location.origin);
  if (to === '/sign-in') {
    url.search = window.location.search;
    if (search?.login_entry_id)
      url.searchParams.set('login_entry_id', search.login_entry_id);
    else url.searchParams.delete('login_entry_id');
  }
  window.history[replace ? 'replaceState' : 'pushState'](
    null,
    '',
    url.pathname + url.search
  );
  window.dispatchEvent(new Event(publicNavigationEvent));
}
