import { seedAccessFixture } from '../../../test/fixtures/console-access';
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import { useQuery, type QueryClient } from '@tanstack/react-query';
import type { PropsWithChildren } from 'react';
import { afterEach, expect, test, vi } from 'vitest';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { selectNavigationQueryScope } from '../../../state/navigation-query-scope';
import { frontstagePageTreeQueryOptions } from '../../../features/frontstage/api/page-tree';
import { settingsConsoleNavigationQueryOptions } from '../../../features/settings/api/console-navigation';
import { ApplicationRuntimeBootstrap } from '../../ApplicationRuntimeBootstrap';
import { AuthenticatedStartup } from '../AuthenticatedStartup';
import { AppProviders } from '../../AppProviders';
import {
  clearStartupPrefetchedBlockNodeCodes,
  readStartupPrefetchedBlockNodeCode
} from '../../../features/frontstage/api/startup-block-code-cache';

const fixture = vi.hoisted(() => {
  let finishShell!: () => void;
  const shell = new Promise<void>((resolve) => {
    finishShell = resolve;
  });
  return {
    shell,
    finishShell,
    bootstrap: vi.fn(),
    pages: vi.fn(),
    tabs: vi.fn(),
    roots: vi.fn(),
    code: vi.fn(),
    navigation: vi.fn(),
    clients: [] as unknown[]
  };
});
vi.mock('@1flowbase/api-client', () => ({
  getConsoleNavigation: fixture.navigation,
  getConsoleFrontstageBootstrap: fixture.bootstrap,
  listFrontstagePages: fixture.pages,
  listFrontstagePageTabs: fixture.tabs,
  listConsoleFrontstageBlockRoots: fixture.roots,
  getConsoleFrontstageBlockNodeCode: fixture.code,
  getFrontstageBlockCatalog: vi.fn(async () => ({ entries: [] })),
  getConsoleFrontstageRuntimeI18nCatalog: vi.fn(async () => ({
    locale: 'zh_Hans',
    messages: {},
    catalog_revision: 1,
    digest: 'd1'
  })),
  getDefaultApiBaseUrl: () => '/api',
  createFrontstageGroup: vi.fn(),
  createFrontstagePage: vi.fn(),
  deleteFrontstagePageNode: vi.fn(),
  moveFrontstagePageNode: vi.fn(),
  updateFrontstagePageNodeTitle: vi.fn()
}));
vi.mock('@1flowbase/api-client/auth', () => ({
  fetchPublicSystemMetadata: vi.fn(async () => ({ brand_name: '1flowbase' }))
}));
vi.mock(
  '../../../features/frontstage/pages/FrontstageWorkspacePage',
  () => ({
    FrontstageWorkspacePage: () => null
  })
);
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
  prepareApplicationI18n: async () => true,
  prepareShellTopbarI18n: async () => true
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

afterEach(() => {
  cleanup();
  resetAuthStore();
});

test('anonymous startup never starts private navigation queries', async () => {
  resetAuthStore();
  useAuthStore.getState().setAnonymous();
  render(<ApplicationRuntimeBootstrap />);
  await act(async () => {
    await vi.dynamicImportSettled();
  });
  expect(screen.getByText('Public sign in')).toBeInTheDocument();
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
  await act(async () => {
    await import('../AuthenticatedStartup');
  });
  await waitFor(() => expect(fixture.pages).toHaveBeenCalledTimes(1));
  expect(fixture.navigation).toHaveBeenCalledTimes(1);
  expect(fixture.clients).toHaveLength(0);
  await act(async () => {
    fixture.finishShell();
    await vi.dynamicImportSettled();
  });
  expect(await screen.findByText('Ready page')).toBeInTheDocument();
  const client = fixture.clients[0] as QueryClient;
  expect(client.getQueryCache().getAll()).toHaveLength(2);
  expect(fixture.pages).toHaveBeenCalledTimes(1);
  expect(fixture.navigation).toHaveBeenCalledTimes(1);
  await act(async () => useAuthStore.getState().setAnonymous());
  await act(async () => {
    await vi.dynamicImportSettled();
  });
  expect(screen.getByText('Public sign in')).toBeInTheDocument();
  expect(client.getQueryCache().getAll()).toHaveLength(0);
});

test('real AuthenticatedStartup mount (useLayoutEffect + useEffect) hydrates via single 1-RTT /frontstage/bootstrap with zero legacy /pages|/tabs|/blocks|/code waterfall', async () => {
  resetAuthStore();
  clearStartupPrefetchedBlockNodeCodes();
  fixture.clients.length = 0;
  fixture.bootstrap.mockReset();
  fixture.pages.mockReset();
  fixture.tabs.mockReset();
  fixture.roots.mockReset();
  fixture.code.mockReset();
  fixture.navigation.mockReset().mockResolvedValue({
    navigation_items: [],
    route_definitions: []
  });
  fixture.bootstrap.mockResolvedValue({
    pages: [
      {
        id: 'group-route',
        title: 'Reports Group',
        kind: 'group',
        placement: 'topbar',
        slug: 'route',
        content_presentation: 'single',
        show_title: true,
        children: [
          {
            id: 'page-reports',
            title: 'Mounted 1-RTT Page',
            kind: 'page',
            placement: 'sidebar',
            content_presentation: 'single',
            show_title: true,
            children: []
          }
        ]
      }
    ],
    resolved_page_id: 'page-reports',
    resolved_tab_id: 'tab-1',
    tabs: [
      {
        id: 'tab-1',
        page_id: 'page-reports',
        title: 'Main',
        rank: '001000',
        is_default: true,
        route_segment: null,
        document_root_uid: 'root-1'
      }
    ],
    page_detail: {
      page: {
        id: 'page-reports',
        title: 'Mounted 1-RTT Page',
        kind: 'page',
        parent_id: 'group-route',
        rank: '001000',
        content_presentation: 'single',
        show_title: true
      },
      tab: {
        id: 'tab-1',
        page_id: 'page-reports',
        title: 'Main',
        rank: '001000',
        is_default: true,
        route_segment: null,
        document_root_uid: 'root-1'
      },
      document: { root_uid: 'root-1', payload: { version: 1 } }
    },
    block_roots: [{ block_id: 'block-mounted-1' }],
    block_codes: [
      {
        block_id: 'block-mounted-1',
        page_id: 'page-reports',
        source_code: 'export default () => <div>Live Mounted</div>;',
        source_sha256: 'sha-mounted-1'
      }
    ]
  });

  window.history.pushState({}, '', '/');
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-mount',
    actor: {
      id: 'actor-mount',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'workspace-mount'
    },
    me: {
      id: 'user-mount',
      account: 'root',
      email: 'root@example.com',
      phone: null,
      nickname: 'Root',
      name: 'Root',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'root',
      policy_fingerprint: 'fixture-policy'
    }
  });
  seedAccessFixture(['frontstage.page.design']);

  const { unmount } = render(<AuthenticatedStartup enforceStartupMountGate />);

  expect(await screen.findByText('Reports Group')).toBeInTheDocument();
  expect(window.location.pathname).toBe('/route/pages/page-reports');
  expect(fixture.bootstrap).toHaveBeenCalledTimes(1);
  expect(fixture.pages).not.toHaveBeenCalled();
  expect(fixture.tabs).not.toHaveBeenCalled();
  expect(fixture.roots).not.toHaveBeenCalled();
  expect(fixture.code).not.toHaveBeenCalled();

  const seededCode = readStartupPrefetchedBlockNodeCode({
    actorId: 'actor-mount',
    workspaceId: 'workspace-mount',
    pageId: 'page-reports',
    blockId: 'block-mounted-1'
  });
  await expect(seededCode).resolves.toMatchObject({
    source_code: 'export default () => <div>Live Mounted</div>;'
  });
  unmount();
});
