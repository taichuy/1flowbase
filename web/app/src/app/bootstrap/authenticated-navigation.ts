import { QueryClient } from '@tanstack/react-query';

import { frontstagePageTreeQueryOptions } from '../../features/frontstage/api/page-tree';
import { settingsConsoleNavigationQueryOptions } from '../../features/settings/api/console-navigation';
import { useAuthStore } from '../../state/auth-store';
import { selectNavigationQueryScope } from '../../state/navigation-query-scope';

// Owned by a mounted bootstrap (or a standalone provider), never a module singleton.
// The public auth runtime has its own client and does not receive this one.
export function createApplicationQueryClient() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } }
  });
  client.setQueryDefaults(['frontstage'], { refetchOnWindowFocus: false });
  return client;
}

export interface StartAuthenticatedNavigationOptions {
  shouldSkipPageTreePrefetch?: (
    state: ReturnType<typeof useAuthStore.getState>
  ) => boolean;
  onClear?: (queryClient: QueryClient) => void;
}

export function startAuthenticatedNavigation(
  queryClient: QueryClient,
  options: StartAuthenticatedNavigationOptions = {}
): () => void {
  let previousScope: string | null = null;
  const synchronize = () => {
    const state = useAuthStore.getState();
    const scope =
      state.sessionStatus === 'authenticated' && state.actor
        ? selectNavigationQueryScope(state)
        : null;
    if (scope === previousScope) return;
    // Cancel/remove the previous identity before publishing its replacement.
    // Query removal also prevents late, non-abortable transports repopulating it.
    if (previousScope !== null) {
      queryClient.clear();
      options.onClear?.(queryClient);
    }
    previousScope = scope;
    if (scope === null) return;
    void queryClient.prefetchQuery(
      settingsConsoleNavigationQueryOptions(scope)
    );
    const workspaceId = state.actor?.current_workspace_id;
    if (workspaceId && !options.shouldSkipPageTreePrefetch?.(state)) {
      void queryClient.prefetchQuery(
        frontstagePageTreeQueryOptions(workspaceId, scope)
      );
    }
  };
  const unsubscribe = useAuthStore.subscribe(synchronize);
  synchronize();
  return () => {
    unsubscribe();
    queryClient.clear();
    options.onClear?.(queryClient);
  };
}
