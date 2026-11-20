import { QueryClientProvider, type QueryClient } from '@tanstack/react-query';
import type { PropsWithChildren } from 'react';
import { Suspense, lazy, useEffect, useState } from 'react';

import { AppThemeProvider } from '@1flowbase/ui/app-theme-provider';

import { AppI18nProvider } from './AppI18nProvider';
import { createApplicationQueryClient } from './bootstrap/authenticated-navigation';
import { WindowWorkspaceProvider } from '../shared/ui/window-workspace/WindowWorkspaceProvider';
import { useAuthStore } from '../state/auth-store';

const FrontstageRuntimeCacheLifecycle = lazy(() =>
  import('../features/frontstage/hooks/use-frontstage-runtime-cache-lifecycle').then(
    (module) => ({
      default: module.FrontstageRuntimeCacheLifecycle
    })
  )
);

function ActivatedRuntimeLifecycles() {
  const sessionStatus = useAuthStore((state) => state.sessionStatus);
  const [frontstageActivated, setFrontstageActivated] = useState(false);

  useEffect(() => {
    if (sessionStatus === 'authenticated') setFrontstageActivated(true);
  }, [sessionStatus]);

  return frontstageActivated ? (
    <Suspense fallback={null}>
      <FrontstageRuntimeCacheLifecycle />
    </Suspense>
  ) : null;
}

export function AppProviders({
  children,
  queryClient: suppliedClient
}: PropsWithChildren<{
  queryClient?: QueryClient;
}>) {
  const [ownedClient] = useState(
    () => suppliedClient ?? createApplicationQueryClient()
  );
  const queryClient = suppliedClient ?? ownedClient;

  return (
    <AppThemeProvider>
      <AppI18nProvider>
        <QueryClientProvider client={queryClient}>
          <ActivatedRuntimeLifecycles />
          <WindowWorkspaceProvider>{children}</WindowWorkspaceProvider>
        </QueryClientProvider>
      </AppI18nProvider>
    </AppThemeProvider>
  );
}
