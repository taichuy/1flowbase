import { Suspense, lazy, useEffect, useLayoutEffect, useState } from 'react';

import { WorkspaceShellSkeleton } from '../../features/auth/components/anonymous-loading/WorkspaceShellSkeleton';
import { LoadingState } from '../../shared/ui/loading-state/LoadingState';
import { resolveAuthenticatedLocale } from '../../shared/i18n/initial-locale';
import { useAuthStore } from '../../state/auth-store';
import {
  createApplicationQueryClient,
  startAuthenticatedNavigation
} from './authenticated-navigation';
import {
  getResolvedAuthenticatedRuntime,
  preloadAuthenticatedStartupRuntime
} from './authenticated-shell-preload';
import {
  claimPrewarmedAuthenticatedQueryClient,
  clearFrontstageStartupHydrationState,
  commitPrewarmedAuthenticatedQueryClient,
  isFrontstageBootstrapOwnedForState,
  prepareAuthenticatedStartupMount,
  startFrontstageStartupPrefetch
} from './frontstage-startup-prefetch';

const LazyAuthenticatedRuntime = lazy(async () => {
  const [Component] = await Promise.all([
    preloadAuthenticatedStartupRuntime(
      resolveAuthenticatedLocale(useAuthStore.getState().me),
      window.location.pathname
    ),
    import('../../app-shell/AppShellFrame')
  ]);
  return { default: Component };
});

export interface AuthenticatedStartupProps {
  /**
   * Defaults to true in browser runtimes (`!import.meta.env.VITEST`) and can be
   * explicitly enabled in startup integration tests to exercise the full
   * `useLayoutEffect` + `useEffect` mount gate together.
   */
  enforceStartupMountGate?: boolean;
}

function RuntimeCommitMarker({ onCommit }: { onCommit: () => void }) {
  useLayoutEffect(() => {
    onCommit();
  }, [onCommit]);
  return null;
}

export function AuthenticatedStartup({
  enforceStartupMountGate = !import.meta.env.VITEST
}: AuthenticatedStartupProps = {}) {
  const [queryClient] = useState(
    () =>
      claimPrewarmedAuthenticatedQueryClient() ?? createApplicationQueryClient()
  );
  const [runtimeReady, setRuntimeReady] = useState(() =>
    Boolean(!enforceStartupMountGate || getResolvedAuthenticatedRuntime())
  );
  const [hasCommittedRuntime, setHasCommittedRuntime] = useState(() =>
    Boolean(getResolvedAuthenticatedRuntime())
  );

  // Single 1-RTT control-plane owner: `/navigation` + `/frontstage/bootstrap`
  // (with automatic fallback to `/pages -> /tabs -> /blocks -> /code` only on error).
  useLayoutEffect(() => {
    commitPrewarmedAuthenticatedQueryClient(queryClient);
    const stopNavigation = startAuthenticatedNavigation(queryClient, {
      shouldSkipPageTreePrefetch: isFrontstageBootstrapOwnedForState,
      onClear: clearFrontstageStartupHydrationState
    });
    const stopFrontstagePrefetch = startFrontstageStartupPrefetch(queryClient);
    return () => {
      stopFrontstagePrefetch();
      stopNavigation();
    };
  }, [queryClient]);

  useEffect(() => {
    if (!enforceStartupMountGate || runtimeReady) return undefined;
    let cancelled = false;
    void prepareAuthenticatedStartupMount(queryClient, () => cancelled)
      .catch(() => undefined)
      .finally(() => {
        if (!cancelled) {
          setRuntimeReady(true);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [enforceStartupMountGate, queryClient, runtimeReady]);

  const ActiveRuntime =
    getResolvedAuthenticatedRuntime() ?? LazyAuthenticatedRuntime;

  return (
    <Suspense
      fallback={
        hasCommittedRuntime ? (
          <LoadingState fullscreen />
        ) : (
          <WorkspaceShellSkeleton mode="full" />
        )
      }
    >
      {runtimeReady ? (
        <>
          <RuntimeCommitMarker
            onCommit={() => setHasCommittedRuntime(true)}
          />
          <ActiveRuntime queryClient={queryClient} />
        </>
      ) : (
        <WorkspaceShellSkeleton mode="full" />
      )}
    </Suspense>
  );
}
