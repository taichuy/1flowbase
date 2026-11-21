import { Suspense, lazy, useLayoutEffect, useState } from 'react';

import { prepareApplicationI18n } from '../../shared/i18n/app-i18n';
import { resolveAuthenticatedLocale } from '../../shared/i18n/initial-locale';
import { LoadingState } from '../../shared/ui/loading-state/LoadingState';
import { useAuthStore } from '../../state/auth-store';
import {
  createApplicationQueryClient,
  startAuthenticatedNavigation
} from './authenticated-navigation';
import {
  claimPrewarmedAuthenticatedQueryClient,
  commitPrewarmedAuthenticatedQueryClient,
  startFrontstageStartupPrefetch
} from './frontstage-startup-prefetch';

const AuthenticatedRuntime = lazy(async () => {
  const [runtimeModule] = await Promise.all([
    import('../AuthenticatedAppRuntime'),
    prepareApplicationI18n(
      resolveAuthenticatedLocale(useAuthStore.getState().me),
      window.location.pathname
    ),
    import('../../app-shell/AppShellFrame')
  ]);
  return { default: runtimeModule.AuthenticatedAppRuntime };
});

export function AuthenticatedStartup() {
  const [queryClient] = useState(
    () =>
      claimPrewarmedAuthenticatedQueryClient() ?? createApplicationQueryClient()
  );
  // Commit the lightweight owner even while the runtime is suspended. Navigation
  // transports run in parallel with shell code/dictionaries, not after its mount.
  useLayoutEffect(() => {
    commitPrewarmedAuthenticatedQueryClient(queryClient);
    const stopNavigation = startAuthenticatedNavigation(queryClient);
    const stopFrontstagePrefetch = startFrontstageStartupPrefetch(queryClient);
    return () => {
      stopFrontstagePrefetch();
      stopNavigation();
    };
  }, [queryClient]);
  return (
    <Suspense fallback={<LoadingState fullscreen />}>
      <AuthenticatedRuntime queryClient={queryClient} />
    </Suspense>
  );
}
