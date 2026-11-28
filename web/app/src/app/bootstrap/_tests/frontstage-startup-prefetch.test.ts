import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

const apiMocks = vi.hoisted(() => ({
  fetchPublicSystemMetadata: vi.fn(),
  fetchSettingsConsoleNavigation: vi.fn(),
  loadSettingsShell: vi.fn(),
  loadSettingsMembers: vi.fn(),
  fetchFrontstageBlockCatalog: vi.fn(),
  fetchFrontstageRuntimeI18nCatalog: vi.fn(),
  fetchFrontstagePageTree: vi.fn(),
  fetchFrontstagePageTabs: vi.fn(),
  fetchFrontstagePageContent: vi.fn(),
  fetchFrontstageBlockRoots: vi.fn(),
  getConsoleFrontstageBlockNodeCode: vi.fn(),
  getConsoleFrontstageBootstrap: vi.fn()
}));

vi.mock('../../../features/settings/pages/SettingsPage', () => {
  apiMocks.loadSettingsShell();
  return { SettingsPage: () => null };
});
vi.mock('../../../features/settings/pages/settings-page/MemberSettingsTabs', () => {
  apiMocks.loadSettingsMembers();
  return { MemberSettingsTabs: () => null };
});

vi.mock('@1flowbase/api-client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@1flowbase/api-client')>();
  return {
    ...actual,
    getConsoleFrontstageBlockNodeCode:
      apiMocks.getConsoleFrontstageBlockNodeCode,
    getConsoleFrontstageBootstrap: apiMocks.getConsoleFrontstageBootstrap
  };
});

vi.mock('@1flowbase/api-client/auth', () => ({
  fetchPublicSystemMetadata: apiMocks.fetchPublicSystemMetadata
}));
vi.mock(
  '../../../features/settings/api/console-navigation',
  async (importOriginal) => {
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
  }
);
vi.mock(
  '../../../features/frontstage/api/block-catalog',
  async (importOriginal) => {
    const actual =
      await importOriginal<
        typeof import('../../../features/frontstage/api/block-catalog')
      >();
    return {
      ...actual,
      fetchFrontstageBlockCatalog: apiMocks.fetchFrontstageBlockCatalog
    };
  }
);
vi.mock('../../../features/frontstage/api/runtime-i18n', () => ({
  fetchFrontstageRuntimeI18nCatalog: apiMocks.fetchFrontstageRuntimeI18nCatalog
}));
vi.mock(
  '../../../features/frontstage/api/page-tree',
  async (importOriginal) => {
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
  }
);
vi.mock(
  '../../../features/frontstage/api/page-tabs',
  async (importOriginal) => {
    const actual =
      await importOriginal<
        typeof import('../../../features/frontstage/api/page-tabs')
      >();
    return {
      ...actual,
      fetchFrontstagePageTabs: apiMocks.fetchFrontstagePageTabs
    };
  }
);
vi.mock(
  '../../../features/frontstage/api/page-content',
  async (importOriginal) => {
    const actual =
      await importOriginal<
        typeof import('../../../features/frontstage/api/page-content')
      >();
    return {
      ...actual,
      fetchFrontstagePageContent: apiMocks.fetchFrontstagePageContent
    };
  }
);
vi.mock(
  '../../../features/frontstage/api/block-tree',
  async (importOriginal) => {
    const actual =
      await importOriginal<
        typeof import('../../../features/frontstage/api/block-tree')
      >();
    return {
      ...actual,
      fetchFrontstageBlockRoots: apiMocks.fetchFrontstageBlockRoots
    };
  }
);

import { waitFor } from '@testing-library/react';
import { frontstagePageContentQueryKey } from '../../../features/frontstage/api/page-content';
import {
  createApplicationQueryClient,
  startAuthenticatedNavigation
} from '../authenticated-navigation';
import {
  claimPrewarmedAuthenticatedQueryClient,
  clearFrontstageStartupHydrationState,
  commitPrewarmedAuthenticatedQueryClient,
  discardPrewarmedAuthenticatedQueryClient,
  isFrontstageBootstrapOwnedForState,
  isFrontstageCandidatePathname,
  prepareAuthenticatedStartupMount,
  prewarmAuthenticatedStartup,
  startFrontstageStartupPrefetch
} from '../frontstage-startup-prefetch';
import { resolveRuntimeConcurrencyProfile } from '../../../shared/utils/runtime-concurrency-profile';
import {
  clearStartupPrefetchedBlockNodeCodes,
  prefetchStartupBlockCodeEntries,
  readStartupPrefetchedBlockNodeCode
} from '../../../features/frontstage/api/startup-block-code-cache';
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
    apiMocks.fetchPublicSystemMetadata.mockResolvedValue({
      brand_name: '1flowbase'
    });
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
      isFrontstageCandidatePathname(
        '/route/pages/01a073f3-03e9-7030-86a4-371f80ebf522'
      )
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

  test.each([
    ['nested', 'nested'],
    ['elsewhere', 'landing'],
    ['group-only', 'landing']
  ])(
    'prefetches the scoped page or canonical fallback for /other/pages/%s',
    async (requested, expected) => {
      authenticateWithMe();
      const page = (id: string) => ({ kind: 'page', id, children: [] });
      apiMocks.fetchFrontstagePageTree.mockResolvedValue([
        { ...page('landing'), placement: 'topbar', slug: 'first' },
        { ...page('elsewhere'), placement: 'topbar', slug: 'elsewhere' },
        {
          kind: 'group',
          id: 'other',
          placement: 'topbar',
          slug: 'other',
          children: [
            { kind: 'group', id: 'group-only', children: [page('nested')] }
          ]
        }
      ]);
      const stop = startFrontstageStartupPrefetch(
        createApplicationQueryClient(),
        {
          getPathname: () => `/other/pages/${requested}`,
          preloadWorkspaceRoute: async () => ({}),
          useBootstrapEndpoint: false
        }
      );
      try {
        await waitFor(() =>
          expect(apiMocks.fetchFrontstagePageTabs.mock.calls).toEqual([
            ['workspace-1', expected]
          ])
        );
      } finally {
        stop();
      }
    }
  );

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

  test('prewarms the Settings shell and pathname section during session discovery, before navigation resolves', async () => {
    let completeNavigation!: (result: { navigation_items: []; route_definitions: [] }) => void;
    apiMocks.fetchSettingsConsoleNavigation.mockReturnValueOnce(
      new Promise((resolve) => { completeNavigation = resolve; })
    );
    try {
      prewarmAuthenticatedStartup({
        csrfToken: 'csrf-1',
        actor: {
          id: 'actor-1',
          account: 'root',
          current_workspace_id: 'workspace-1',
          effective_display_role: 'root'
        },
        me: null
      }, { getPathname: () => '/settings/members' });

      expect(apiMocks.fetchSettingsConsoleNavigation).toHaveBeenCalledTimes(1);
      await waitFor(() => {
        expect(apiMocks.loadSettingsShell).toHaveBeenCalledTimes(1);
        expect(apiMocks.loadSettingsMembers).toHaveBeenCalledTimes(1);
      });
      expect(apiMocks.getConsoleFrontstageBootstrap).not.toHaveBeenCalled();
    } finally {
      completeNavigation({ navigation_items: [], route_definitions: [] });
    }
  });

  test('prewarmAuthenticatedStartup uses 1-RTT /frontstage/bootstrap and hands the same QueryClient to AuthenticatedStartup without duplicate or legacy waterfall requests', async () => {
    clearStartupPrefetchedBlockNodeCodes();
    apiMocks.getConsoleFrontstageBootstrap.mockResolvedValue({
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
              title: 'Reports',
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
      resolved_tab_id: 'tab-default',
      tabs: [
        {
          id: 'tab-default',
          page_id: 'page-reports',
          title: 'Overview',
          rank: '001000',
          is_default: true,
          route_segment: 'overview',
          document_root_uid: 'root-1'
        }
      ],
      page_detail: {
        page: {
          id: 'page-reports',
          title: 'Reports',
          kind: 'page',
          parent_id: 'group-route',
          rank: '001000',
          content_presentation: 'single',
          show_title: true
        },
        tab: {
          id: 'tab-default',
          page_id: 'page-reports',
          title: 'Overview',
          rank: '001000',
          is_default: true,
          route_segment: 'overview',
          document_root_uid: 'root-1'
        },
        document: { root_uid: 'root-1', payload: { version: 1 } }
      },
      block_roots: [{ block_id: 'block-prewarm-1' }],
      block_codes: [
        {
          block_id: 'block-prewarm-1',
          page_id: 'page-reports',
          source_code: 'export default () => <div>Prewarm</div>;',
          source_sha256: 'sha-prewarm-1'
        }
      ]
    });

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
      getPathname: () => '/route/pages/page-reports/tabs/overview',
      preloadWorkspaceRoute
    });

    await waitFor(() => {
      expect(apiMocks.fetchSettingsConsoleNavigation).toHaveBeenCalledTimes(1);
      expect(apiMocks.getConsoleFrontstageBootstrap).toHaveBeenCalledTimes(1);
      expect(apiMocks.fetchFrontstageBlockCatalog).toHaveBeenCalledTimes(1);
    });
    expect(apiMocks.fetchFrontstagePageTree).not.toHaveBeenCalled();
    expect(apiMocks.fetchFrontstagePageTabs).not.toHaveBeenCalled();
    expect(apiMocks.fetchFrontstagePageContent).not.toHaveBeenCalled();
    expect(apiMocks.fetchFrontstageBlockRoots).not.toHaveBeenCalled();
    expect(apiMocks.getConsoleFrontstageBlockNodeCode).not.toHaveBeenCalled();

    useAuthStore.getState().setAuthenticated(snapshot);

    // StrictMode invokes useState initializer twice before useLayoutEffect commits
    expect(claimPrewarmedAuthenticatedQueryClient()).toBe(prewarmed);
    expect(claimPrewarmedAuthenticatedQueryClient()).toBe(prewarmed);
    commitPrewarmedAuthenticatedQueryClient(prewarmed);
    expect(claimPrewarmedAuthenticatedQueryClient()).toBeNull();

    const stopNav = startAuthenticatedNavigation(prewarmed, {
      shouldSkipPageTreePrefetch: (state) =>
        isFrontstageBootstrapOwnedForState(state, '/route/pages/page-reports'),
      onClear: clearFrontstageStartupHydrationState
    });
    const stopPrefetch = startFrontstageStartupPrefetch(prewarmed, {
      getPathname: () => '/route/pages/page-reports',
      preloadWorkspaceRoute
    });
    const mounted = await prepareAuthenticatedStartupMount(
      prewarmed,
      () => false,
      {
        getPathname: () => '/route/pages/page-reports',
        preloadShellAndRoute: async () => undefined,
        preloadWorkspaceRoute
      }
    );
    expect(mounted.targetPath).toBe('/route/pages/page-reports');

    for (const reference of ['tab-default', 'overview']) {
      const key = frontstagePageContentQueryKey(
        'workspace-1',
        'page-reports',
        reference
      );
      expect(prewarmed.getQueryData(key)).toMatchObject({
        tab: { id: 'tab-default' },
        document: { rootUid: 'root-1' }
      });
      expect(prewarmed.getQueryDefaults(key).staleTime).toBe(30_000);
    }

    // Zero duplicate /bootstrap and zero legacy waterfall requests across prewarm -> layout -> effect!
    expect(apiMocks.fetchSettingsConsoleNavigation).toHaveBeenCalledTimes(1);
    expect(apiMocks.getConsoleFrontstageBootstrap).toHaveBeenCalledTimes(1);
    expect(apiMocks.fetchFrontstagePageTree).not.toHaveBeenCalled();
    expect(apiMocks.fetchFrontstagePageTabs).not.toHaveBeenCalled();
    expect(apiMocks.fetchFrontstagePageContent).not.toHaveBeenCalled();
    expect(apiMocks.fetchFrontstageBlockRoots).not.toHaveBeenCalled();
    expect(apiMocks.getConsoleFrontstageBlockNodeCode).not.toHaveBeenCalled();

    stopPrefetch();
    stopNav();
  });

  test('startup-block-code-cache shares a single in-flight/settled code fetch across multiple reads without duplicate HTTP calls', async () => {
    clearStartupPrefetchedBlockNodeCodes();
    apiMocks.getConsoleFrontstageBlockNodeCode.mockResolvedValue({
      block_id: 'block-1',
      source_code: 'export default function Demo() { return null; }',
      source_sha256: 'abc123'
    });

    const entries = prefetchStartupBlockCodeEntries({
      actorId: 'actor-1',
      workspaceId: 'workspace-1',
      pageId: 'page-reports',
      blockIds: ['block-1', 'block-1', 'block-2']
    });
    expect(entries).toHaveLength(2);
    expect(apiMocks.getConsoleFrontstageBlockNodeCode).toHaveBeenCalledTimes(2);

    const readA = readStartupPrefetchedBlockNodeCode({
      actorId: 'actor-1',
      workspaceId: 'workspace-1',
      pageId: 'page-reports',
      blockId: 'block-1'
    });
    const readB = readStartupPrefetchedBlockNodeCode({
      actorId: 'actor-1',
      workspaceId: 'workspace-1',
      pageId: 'page-reports',
      blockId: 'block-1'
    });
    expect(readA).not.toBeNull();
    expect(readB).toBe(readA);
    await expect(readA).resolves.toMatchObject({ block_id: 'block-1' });
    expect(apiMocks.getConsoleFrontstageBlockNodeCode).toHaveBeenCalledTimes(2);
  });

  test('prepareAuthenticatedStartupMount enforces Wave 0 serial control gate in DEV and parallel t=0 launch in PROD', async () => {
    clearStartupPrefetchedBlockNodeCodes();
    authenticateWithMe();
    const client = createApplicationQueryClient();
    const events: string[] = [];
    apiMocks.getConsoleFrontstageBootstrap.mockRejectedValueOnce(
      new Error('unupgraded backend fallback')
    );
    apiMocks.fetchFrontstagePageTree.mockImplementation(async () => {
      events.push('pageTree');
      return [
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
      ];
    });
    apiMocks.fetchFrontstagePageTabs.mockImplementation(async () => {
      events.push('pageTabs');
      return [
        {
          id: 'tab-default',
          page_id: 'page-reports',
          title: 'Overview',
          rank: '001000',
          is_default: true,
          route_segment: null,
          document_root_uid: 'root-1'
        }
      ];
    });
    apiMocks.fetchFrontstageBlockRoots.mockImplementation(async () => {
      events.push('blockRoots');
      return [{ block_id: 'block-hero' }, { block_id: 'block-table' }];
    });
    apiMocks.getConsoleFrontstageBlockNodeCode.mockImplementation(
      async (_pageId: string, blockId: string) => {
        events.push(`code:${blockId}`);
        return {
          block_id: blockId,
          source_code: 'export default function B() { return null; }',
          source_sha256: `sha-${blockId}`
        };
      }
    );

    const preloadShellAndRoute = vi.fn(async (targetPath: string) => {
      events.push(`preloadModules:${targetPath}`);
    });

    const devResult = await prepareAuthenticatedStartupMount(
      client,
      () => false,
      {
        getPathname: () => '/',
        concurrencyProfile: resolveRuntimeConcurrencyProfile({ isDev: true }),
        preloadShellAndRoute,
        preloadWorkspaceRoute: async () => ({}),
        preloadHomeRoute: async () => ({})
      }
    );

    expect(devResult.targetPath).toBe('/route/pages/page-reports');
    expect(preloadShellAndRoute).toHaveBeenCalledWith(
      '/route/pages/page-reports'
    );
    // Strict DEV Wave 0 invariant: control data precedes heavy JS preload!
    expect(events.indexOf('pageTree')).toBeLessThan(events.indexOf('pageTabs'));
    expect(events.indexOf('pageTabs')).toBeLessThan(
      events.indexOf('blockRoots')
    );
    expect(events.indexOf('code:block-hero')).toBeLessThan(
      events.indexOf('preloadModules:/route/pages/page-reports')
    );

    // PROD parallel t=0 invariant: preloadShellAndRoute launches BEFORE bootstrap resolves!
    const prodClient = createApplicationQueryClient();
    const prodEvents: string[] = [];
    let resolveBootstrap!: (value: unknown) => void;
    apiMocks.getConsoleFrontstageBootstrap.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveBootstrap = resolve;
      })
    );
    const prodFlight = prepareAuthenticatedStartupMount(
      prodClient,
      () => false,
      {
        getPathname: () => '/',
        concurrencyProfile: resolveRuntimeConcurrencyProfile({
          isDev: false,
          hardwareConcurrency: 8
        }),
        preloadShellAndRoute: async (path) => {
          prodEvents.push(`prodPreload:${path}`);
        },
        preloadWorkspaceRoute: async () => ({}),
        preloadHomeRoute: async () => ({})
      }
    );
    expect(prodEvents).toEqual(['prodPreload:/']);
    resolveBootstrap({
      pages: [
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
      ],
      resolved_page_id: 'page-reports',
      resolved_tab_id: null,
      tabs: [],
      page_detail: null,
      block_roots: null,
      block_codes: null
    });
    const prodResult = await prodFlight;
    expect(prodResult.targetPath).toBe('/route/pages/page-reports');
  });

  test('hydrates pageTree, tabs, pageContent, blockRoots, and live block source_code in 1 RTT via /frontstage/bootstrap with zero N+1 code requests', async () => {
    clearStartupPrefetchedBlockNodeCodes();
    authenticateWithMe();
    const client = createApplicationQueryClient();
    apiMocks.getConsoleFrontstageBootstrap.mockResolvedValue({
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
              title: 'Reports',
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
      resolved_tab_id: 'tab-default',
      tabs: [
        {
          id: 'tab-default',
          page_id: 'page-reports',
          title: 'Overview',
          rank: '001000',
          is_default: true,
          route_segment: null,
          document_root_uid: 'root-1'
        }
      ],
      page_detail: {
        page: {
          id: 'page-reports',
          title: 'Reports',
          kind: 'page',
          parent_id: 'group-route',
          rank: '001000',
          content_presentation: 'single',
          show_title: true
        },
        tab: {
          id: 'tab-default',
          page_id: 'page-reports',
          title: 'Overview',
          rank: '001000',
          is_default: true,
          route_segment: null,
          document_root_uid: 'root-1'
        },
        document: {
          root_uid: 'root-1',
          payload: { version: 1 }
        }
      },
      block_roots: [{ block_id: 'block-ai-1' }, { block_id: 'block-ai-2' }],
      block_codes: [
        {
          block_id: 'block-ai-1',
          page_id: 'page-reports',
          source_code: 'export default () => <div>AI 1</div>;',
          source_sha256: 'sha-ai-1'
        },
        {
          block_id: 'block-ai-2',
          page_id: 'page-reports',
          source_code: 'export default () => <div>AI 2</div>;',
          source_sha256: 'sha-ai-2'
        }
      ]
    });

    const stopNav = startAuthenticatedNavigation(client, {
      shouldSkipPageTreePrefetch: (state) =>
        isFrontstageBootstrapOwnedForState(state, '/'),
      onClear: clearFrontstageStartupHydrationState
    });
    const stopPrefetch = startFrontstageStartupPrefetch(client, {
      getPathname: () => '/',
      preloadWorkspaceRoute: async () => ({}),
      preloadHomeRoute: async () => ({})
    });
    const result = await prepareAuthenticatedStartupMount(client, () => false, {
      getPathname: () => '/',
      preloadShellAndRoute: async () => undefined,
      preloadWorkspaceRoute: async () => ({}),
      preloadHomeRoute: async () => ({})
    });

    expect(result.targetPath).toBe('/route/pages/page-reports');
    expect(apiMocks.getConsoleFrontstageBootstrap).toHaveBeenCalledTimes(1);
    expect(apiMocks.fetchFrontstagePageTree).not.toHaveBeenCalled();
    expect(apiMocks.fetchFrontstagePageTabs).not.toHaveBeenCalled();
    expect(apiMocks.fetchFrontstagePageContent).not.toHaveBeenCalled();
    expect(apiMocks.fetchFrontstageBlockRoots).not.toHaveBeenCalled();
    expect(apiMocks.getConsoleFrontstageBlockNodeCode).not.toHaveBeenCalled();

    const code1 = readStartupPrefetchedBlockNodeCode({
      actorId: 'actor-1',
      workspaceId: 'workspace-1',
      pageId: 'page-reports',
      blockId: 'block-ai-1'
    });
    const code2 = readStartupPrefetchedBlockNodeCode({
      actorId: 'actor-1',
      workspaceId: 'workspace-1',
      pageId: 'page-reports',
      blockId: 'block-ai-2'
    });
    await expect(code1).resolves.toMatchObject({
      source_code: 'export default () => <div>AI 1</div>;'
    });
    await expect(code2).resolves.toMatchObject({
      source_code: 'export default () => <div>AI 2</div>;'
    });

    stopPrefetch();
    stopNav();
  });

  test('does not pollute default landing page tabs cache with empty array when bootstrap resolved_page_id is null', async () => {
    authenticateWithMe();
    const client = createApplicationQueryClient();
    apiMocks.fetchSettingsConsoleNavigation.mockResolvedValue({ items: [] });
    apiMocks.fetchFrontstageBlockCatalog.mockResolvedValue([]);
    apiMocks.fetchFrontstageRuntimeI18nCatalog.mockResolvedValue({});
    apiMocks.getConsoleFrontstageBootstrap.mockResolvedValue({
      pages: [
        {
          id: 'group-route',
          title: 'route',
          kind: 'group',
          placement: 'topbar',
          slug: 'route',
          content_presentation: 'single',
          show_title: true,
          children: [
            {
              id: 'page-landing',
              title: 'Landing',
              kind: 'page',
              placement: 'sidebar',
              content_presentation: 'single',
              show_title: true,
              children: []
            }
          ]
        }
      ],
      resolved_page_id: null,
      resolved_tab_id: null,
      tabs: [],
      page_detail: null,
      block_roots: null,
      block_codes: null
    });
    const landingTabs = [
      {
        id: 'tab-landing-default',
        page_id: 'page-landing',
        title: 'Home',
        rank: '001000',
        is_default: true,
        route_segment: null,
        document_root_uid: 'root-landing'
      }
    ];
    apiMocks.fetchFrontstagePageTabs.mockResolvedValue(landingTabs);
    apiMocks.fetchFrontstagePageContent.mockResolvedValue({
      page: {
        id: 'page-landing',
        title: 'Landing',
        kind: 'page',
        parent_id: 'group-route',
        rank: '001000',
        content_presentation: 'single',
        show_title: true
      },
      tab: landingTabs[0],
      document: { root_uid: 'root-landing', payload: { version: 1 } }
    });
    apiMocks.fetchFrontstageBlockRoots.mockResolvedValue([]);

    const result = await prepareAuthenticatedStartupMount(client, () => false, {
      getPathname: () => '/route/pages/invisible-or-deleted-page',
      preloadShellAndRoute: async () => undefined,
      preloadWorkspaceRoute: async () => ({}),
      preloadHomeRoute: async () => ({})
    });

    expect(result.targetPath).toBe('/route/pages/page-landing');
    // Reused seeded pageTree from bootstrap (0 extra /pages fetch) and fetched real landing tabs!
    expect(apiMocks.fetchFrontstagePageTree).not.toHaveBeenCalled();
    expect(apiMocks.fetchFrontstagePageTabs).toHaveBeenCalledWith(
      'workspace-1',
      'page-landing'
    );
    const { frontstagePageTabsQueryKey } =
      await import('../../../features/frontstage/api/page-tabs');
    expect(
      client.getQueryData(
        frontstagePageTabsQueryKey('workspace-1', 'page-landing')
      )
    ).toEqual(landingTabs);
  });
});
