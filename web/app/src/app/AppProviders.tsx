import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { PropsWithChildren } from 'react';
import { useState } from 'react';

import { AppThemeProvider } from '@1flowbase/ui';

import { AppI18nProvider } from './AppI18nProvider';
import { WindowWorkspaceProvider } from '../shared/ui/window-workspace/WindowWorkspaceProvider';
import { useFrontstageRuntimeCacheLifecycle } from '../features/frontstage/hooks/use-frontstage-runtime-cache-lifecycle';

function FrontstageRuntimeCacheLifecycle() {
  useFrontstageRuntimeCacheLifecycle();
  return null;
}

export function AppProviders({ children }: PropsWithChildren) {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            retry: false
          },
          mutations: {
            retry: false
          }
        }
      })
  );

  return (
    <AppThemeProvider>
      <AppI18nProvider>
        <QueryClientProvider client={queryClient}>
          <FrontstageRuntimeCacheLifecycle />
          <WindowWorkspaceProvider>{children}</WindowWorkspaceProvider>
        </QueryClientProvider>
      </AppI18nProvider>
    </AppThemeProvider>
  );
}
