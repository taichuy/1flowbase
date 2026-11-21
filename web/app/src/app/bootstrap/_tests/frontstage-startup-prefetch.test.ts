import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

const apiMocks = vi.hoisted(() => ({
  fetchPublicSystemMetadata: vi.fn(),
  fetchSettingsConsoleNavigation: vi.fn(),
  fetchFrontstageBlockCatalog: vi.fn(),
  fetchFrontstageRuntimeI18nCatalog: vi.fn(),
  fetchFrontstagePageTree: vi.fn(),
  fetchFrontstagePageTabs: vi.fn(),
  fetchFrontstagePageContent: vi.fn(),
  fetchFrontstageBlockRoots: vi.fn()
}));

vi.mock('@1flowbase/api-client/auth', () => ({
  fetchPublicSystemMetadata: apiMocks.fetchPublicSystemMetadata
}));
vi.mock('../../../features/settings/api/console-navigation', async (importOriginal) => {
  const actual =
    await importOriginal<
      typeof import('../../../features/settings/api/console-navigation')
    >();
  return {
    ...actual,
    settingsConsoleNavigationQueryOptions: (scope: string) => ({
      queryKey: [...actual.settingsConsoleNavigationQueryKey, scope],
      queryFn: () => apiMocks.fetchSettingsConsoleNavigation(),
      staleTime: 30_000,
      retry: false
    })
  };
});
vi.mock('../../../features/frontstage/api/block-catalog', async (importOriginal) => {
  const actual =
    await importOriginal<
      typeof import('../../../features/frontstage/api/block-catalog')
    >();
  return {
    ...actual,
    fetchFrontstageBlockCatalog: apiMocks.fetchFrontstageBlockCatalog
  };
});
vi.mock('../../../features/frontstage/api/runtime-i18n', () => ({
  fetchFrontstageRuntimeI18nCatalog: apiMocks.fetchFrontstageRuntimeI18nCatalog
}));
vi.mock('../../../features/frontstage/api/page-tree', async (importOriginal) => {
  const actual =
    await importOriginal<
      typeof import('../../../features/frontstage/api/page-tree')
    >();
  return {
    ...actual,
    frontstagePageTreeQueryOptions: (workspaceId: string, scope: string) => ({
      queryKey: actual.frontstagePageTreeQueryKey(workspaceId, scope),
      queryFn: () => apiMocks.fetchFrontstagePageTree(workspaceId),
      staleTime: 30_000,
      retry: false
    })
  };
});
vi.mock('../../../features/frontstage/api/page-tabs', async (importOriginal) => {
  const actual =
    await importOriginal<
      typeof import('../../../features/frontstage/api/page-tabs')
    >();
  return {
    ...actual,
    fetchFrontstagePageTabs: apiMocks.fetchFrontstagePageTabs
  };
});
vi.mock('../../../features/frontstage/api/page-content', async (importOriginal) => {
  const actual =
    await importOriginal<
      typeof import('../../../features/frontstage/api/page-content')
    >();
  return {
    ...actual,
    fetchFrontstagePageContent: apiMocks.fetchFrontstagePageContent
  };
});
vi.mock('../../../features/frontstage/api/block-tree', async (importOriginal) => {
  const actual =
    await importOriginal<
      typeof import('../../../features/frontstage/api/block-tree')
    >();
  return {
    ...actual,
    fetchFrontstageBlockRoots: apiMocks.fetchFrontstageBlockRoots
  };
});

import { waitFor } from '@testing-library/react';
import {
  createApplicationQueryClient,
  startAuthenticatedNavigation
} from '../authenticated-navigation';
import {
  claimPrewarmedAuthenticatedQueryClient,
  commitPrewarmedAuthenticatedQueryClient,
  discardPrewarmedAuthenticatedQueryClient,
  isFrontstageCandidatePathname,
  prewarmAuthenticatedStartup,
  startFrontstageStartupPrefetch
} from '../frontstage-startup-prefetch';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';

function authenticateWithMe() {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-1',
    actor: {
      id: 'actor-1',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'user-1',
      account: 'root',
      email: 'root@example.com',
      phone: null,
      nickname: 'Root',
      name: 'Root',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'root',
      permissions: ['frontstage.page.design']
    }
  });
}

describe('frontstage startup prefetch', () => {
  beforeEach(() => {
    discardPrewarmedAuthenticatedQueryClient();
    resetAuthStore();
    vi.clearAllMocks();
    apiMocks.fetchSettingsConsoleNavigation.mockResolvedValue({
      navigation_items: [],
      route_definitions: [],
      permission_bindings: []
    });
    apiMocks.fetchPublicSystemMetadata.mockResolvedValue({ brand_name: '1flowbase' });
    apiMocks.fetchFrontstageBlockCatalog.mockResolvedValue({ entries: [] });
    apiMocks.fetchFrontstageRuntimeI18nCatalog.mockResolvedValue({
      locale: 'zh_Hans',
      messages: {},
      catalog_revision: 1,
      digest: 'd1'
    });
    apiMocks.fetchFrontstagePageTree.mockResolvedValue([
      {
        id: 'group-route',
        title: 'Reports Group',
        kind: 'group',
        placement: 'topbar',
        slug: 'route',
        content_presentation: 'single',
        children: [
          {
            id: 'page-reports',
            title: 'Reports',
            kind: 'page',
            placement: 'sidebar',
            content_presentation: 'single',
            children: []
          }
        ]
      }
    ]);
    apiMocks.fetchFrontstagePageTabs.mockResolvedValue([
      {
        id: 'tab-default',
        page_id: 'page-reports',
        title: 'Overview',
        rank: '001000',
        is_default: true,
        route_segment: null,
        document_root_uid: 'root-1'
      }
    ]);
    apiMocks.fetchFrontstagePageContent.mockResolvedValue({
      page: { id: 'page-reports' },
      tab: { id: 'tab-default' },
      document: { rootUid: 'root-1', payload: {} }
    });
    apiMocks.fetchFrontstageBlockRoots.mockResolvedValue([]);
  });

  afterEach(() => {
    resetAuthStore();
  });

  test('identifies frontstage vs console/public pathnames accurately', () => {
    expect(isFrontstageCandidatePathname('/')).toBe(true);
    expect(isFrontstageCandidatePathname('/route')).toBe(true);
    expect(
      isFrontstageCandidatePathname('/route/pages/01a073f3-03e9-7030-86a4-371f80ebf522')
    ).toBe(true);
    expect(isFrontstageCandidatePathname('/settings/general')).toBe(false);
    expect(isFrontstageCandidatePathname('/applications')).toBe(false);
    expect(isFrontstageCandidatePathname('/sign-in')).toBe(false);
    expect(isFrontstageCandidatePathname('/me')).toBe(false);
  });

  test('prefetches blockCatalog, systemMetadata, runtime-i18n, and chains pageTabs -> parallel pageContent + blockRoots on frontstage paths', async () => {
    authenticateWithMe();
    const client = createApplicationQueryClient();
    const preloadWorkspaceRoute = vi.fn(async () => ({}));
    const preloadHomeRoute = vi.fn(async () => ({}));

    let resolveContent!: (value: unknown) => void;
    apiMocks.fetchFrontstagePageContent.mockReturnValue(
      new Promise((resolve) => {
        resolveContent = resolve;
      })
    );

    const stop = startFrontstageStartupPrefetch(client, {
      getPathname: () => '/route/pages/page-reports',
      preloadWorkspaceRoute,
      preloadHomeRoute
    });

    expect(preloadWorkspaceRoute).toHaveBeenCalledTimes(1);
    expect(preloadHomeRoute).not.toHaveBeenCalled();
    expect(apiMocks.fetchPublicSystemMetadata).toHaveBeenCalledTimes(1);
    expect(apiMocks.fetchFrontstageBlockCatalog).toHaveBeenCalledTimes(1);
    expect(apiMocks.fetchFrontstageRuntimeI18nCatalog).toHaveBeenCalledTimes(1);

    await waitFor(() => {
      expect(apiMocks.fetchFrontstagePageTabs).toHaveBeenCalledWith(
        'workspace-1',
        'page-reports'
      );
      expect(apiMocks.fetchFrontstagePageContent).toHaveBeenCalledWith(
        'workspace-1',
        'page-reports',
        'tab-default'
      );
      expect(apiMocks.fetchFrontstageBlockRoots).toHaveBeenCalledWith(
        'workspace-1',
        'page-reports',
        { tab_id: 'tab-default' }
      );
    });

    resolveContent({
      page: { id: 'page-reports' },
      tab: { id: 'tab-default' },
      document: { rootUid: 'root-1', payload: {} }
    });
    stop();
  });

  test('does nothing on console paths or when me is null', () => {
    authenticateWithMe();
    const client = createApplicationQueryClient();
    const preloadWorkspaceRoute = vi.fn(async () => ({}));

    const stopSettings = startFrontstageStartupPrefetch(client, {
      getPathname: () => '/settings/users',
      preloadWorkspaceRoute
    });
    expect(preloadWorkspaceRoute).not.toHaveBeenCalled();
    expect(apiMocks.fetchFrontstageBlockCatalog).not.toHaveBeenCalled();
    stopSettings();

    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-1',
      actor: {
        id: 'actor-1',
        account: 'root',
        effective_display_role: 'root',
        current_workspace_id: 'workspace-1'
      },
      me: null
    });
    const stopNullMe = startFrontstageStartupPrefetch(client, {
      getPathname: () => '/route/pages/page-reports',
      preloadWorkspaceRoute
    });
    expect(preloadWorkspaceRoute).not.toHaveBeenCalled();
    expect(apiMocks.fetchFrontstageBlockCatalog).not.toHaveBeenCalled();
    stopNullMe();
  });

  test('prewarmAuthenticatedStartup hands the same QueryClient across StrictMode double-init without duplicate requests', async () => {
    const snapshot = {
      csrfToken: 'csrf-1',
      actor: {
        id: 'actor-1',
        account: 'root',
        effective_display_role: 'root' as const,
        current_workspace_id: 'workspace-1'
      },
      me: {
        id: 'user-1',
        account: 'root',
        email: 'root@example.com',
        phone: null,
        nickname: 'Root',
        name: 'Root',
        avatar_url: null,
        introduction: '',
        effective_display_role: 'root' as const,
        permissions: ['frontstage.page.design']
      }
    };
    const preloadWorkspaceRoute = vi.fn(async () => ({}));
    const prewarmed = prewarmAuthenticatedStartup(snapshot, {
      getPathname: () => '/route/pages/page-reports',
      preloadWorkspaceRoute
    });

    await waitFor(() => {
      expect(apiMocks.fetchSettingsConsoleNavigation).toHaveBeenCalledTimes(1);
      expect(apiMocks.fetchFrontstagePageTree).toHaveBeenCalledTimes(1);
      expect(apiMocks.fetchFrontstageBlockCatalog).toHaveBeenCalledTimes(1);
      expect(apiMocks.fetchFrontstagePageTabs).toHaveBeenCalledTimes(1);
      expect(apiMocks.fetchFrontstageBlockRoots).toHaveBeenCalledTimes(1);
    });

    useAuthStore.getState().setAuthenticated(snapshot);

    // StrictMode invokes useState initializer twice before useLayoutEffect commits
    expect(claimPrewarmedAuthenticatedQueryClient()).toBe(prewarmed);
    expect(claimPrewarmedAuthenticatedQueryClient()).toBe(prewarmed);
    commitPrewarmedAuthenticatedQueryClient(prewarmed);
    expect(claimPrewarmedAuthenticatedQueryClient()).toBeNull();

    const stopNav = startAuthenticatedNavigation(prewarmed);
    const stopPrefetch = startFrontstageStartupPrefetch(prewarmed, {
      getPathname: () => '/route/pages/page-reports',
      preloadWorkspaceRoute
    });

    // Zero duplicate requests on AuthenticatedStartup mount handoff!
    expect(apiMocks.fetchSettingsConsoleNavigation).toHaveBeenCalledTimes(1);
    expect(apiMocks.fetchFrontstagePageTree).toHaveBeenCalledTimes(1);
    expect(apiMocks.fetchFrontstageBlockCatalog).toHaveBeenCalledTimes(1);
    expect(apiMocks.fetchFrontstagePageTabs).toHaveBeenCalledTimes(1);
    expect(apiMocks.fetchFrontstageBlockRoots).toHaveBeenCalledTimes(1);

    stopPrefetch();
    stopNav();
  });
});
