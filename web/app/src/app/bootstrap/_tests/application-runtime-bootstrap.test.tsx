import { act, render, screen, waitFor } from '@testing-library/react';
import { useQuery, type QueryClient } from '@tanstack/react-query';
import type { PropsWithChildren } from 'react';
import { afterEach, expect, test, vi } from 'vitest';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { selectNavigationQueryScope } from '../../../state/navigation-query-scope';
import { frontstagePageTreeQueryOptions } from '../../../features/frontstage/api/page-tree';
import { settingsConsoleNavigationQueryOptions } from '../../../features/settings/api/console-navigation';
import { ApplicationRuntimeBootstrap } from '../../ApplicationRuntimeBootstrap';
import { AppProviders } from '../../AppProviders';

const fixture = vi.hoisted(() => {
  let finishShell!: () => void;
  const shell = new Promise<void>((resolve) => {
    finishShell = resolve;
  });
  return {
    shell,
    finishShell,
    pages: vi.fn(),
    navigation: vi.fn(),
    clients: [] as unknown[]
  };
});
vi.mock('@1flowbase/api-client', () => ({
  getConsoleNavigation: fixture.navigation,
  listFrontstagePages: fixture.pages,
  getDefaultApiBaseUrl: () => '/api',
  createFrontstageGroup: vi.fn(),
  createFrontstagePage: vi.fn(),
  deleteFrontstagePageNode: vi.fn(),
  moveFrontstagePageNode: vi.fn(),
  updateFrontstagePageNodeTitle: vi.fn()
}));
vi.mock('@1flowbase/ui/app-theme-provider', () => ({
  AppThemeProvider: ({ children }: PropsWithChildren) => <>{children}</>
}));
vi.mock('../../AppI18nProvider', () => ({
  AppI18nProvider: ({ children }: PropsWithChildren) => <>{children}</>
}));
vi.mock('../../../shared/ui/window-workspace/WindowWorkspaceProvider', () => ({
  WindowWorkspaceProvider: ({ children }: PropsWithChildren) => <>{children}</>
}));
vi.mock(
  '../../../features/frontstage/hooks/use-frontstage-runtime-cache-lifecycle',
  () => ({
    FrontstageRuntimeCacheLifecycle: () => null
  })
);
vi.mock('../../../shared/i18n/app-i18n', () => ({
  prepareApplicationI18n: async () => true
}));
vi.mock('../../../shared/ui/loading-state/LoadingState', () => ({
  LoadingState: () => <div>Loading</div>
}));
vi.mock('../../../app-shell/AppShellFrame', async () => {
  await fixture.shell;
  return { AppShellFrame: () => null };
});
vi.mock('../../AnonymousAppRuntime', () => ({
  AnonymousAppRuntime: () => <div>Public sign in</div>
}));
vi.mock('../../AuthenticatedAppRuntime', () => ({
  AuthenticatedAppRuntime: ({ queryClient }: { queryClient: QueryClient }) => {
    fixture.clients.push(queryClient);
    return (
      <AppProviders queryClient={queryClient}>
        <Consumer />
      </AppProviders>
    );
  }
}));
function Consumer() {
  const state = useAuthStore();
  const scope = selectNavigationQueryScope(state);
  const pages = useQuery(
    frontstagePageTreeQueryOptions(state.actor!.current_workspace_id, scope)
  );
  useQuery(settingsConsoleNavigationQueryOptions(scope));
  return <div>{pages.data?.[0]?.title ?? 'Waiting for pages'}</div>;
}

afterEach(resetAuthStore);

test('anonymous startup never starts private navigation queries', async () => {
  resetAuthStore();
  useAuthStore.getState().setAnonymous();
  render(<ApplicationRuntimeBootstrap />);
  expect(await screen.findByText('Public sign in')).toBeInTheDocument();
  expect(fixture.pages).not.toHaveBeenCalled();
  expect(fixture.navigation).not.toHaveBeenCalled();
  expect(fixture.clients).toHaveLength(0);
});

test('prefetches while heavy runtime is unresolved, hands the same client to consumers, then clears on logout', async () => {
  resetAuthStore();
  fixture.pages.mockResolvedValue([{ id: 'page', title: 'Ready page' }]);
  fixture.navigation.mockResolvedValue({
    navigation_items: [],
    route_definitions: []
  });
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf',
    me: null,
    actor: {
      id: 'actor',
      account: 'actor',
      effective_display_role: 'member',
      current_workspace_id: 'workspace'
    }
  });
  render(<ApplicationRuntimeBootstrap />);
  await waitFor(() => expect(fixture.pages).toHaveBeenCalledTimes(1));
  expect(fixture.navigation).toHaveBeenCalledTimes(1);
  expect(fixture.clients).toHaveLength(0);
  await act(async () => fixture.finishShell());
  expect(await screen.findByText('Ready page')).toBeInTheDocument();
  const client = fixture.clients[0] as QueryClient;
  expect(client.getQueryCache().getAll()).toHaveLength(2);
  expect(fixture.pages).toHaveBeenCalledTimes(1);
  expect(fixture.navigation).toHaveBeenCalledTimes(1);
  await act(async () => useAuthStore.getState().setAnonymous());
  expect(await screen.findByText('Public sign in')).toBeInTheDocument();
  expect(client.getQueryCache().getAll()).toHaveLength(0);
});
