import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const pageTreeApi = vi.hoisted(() => ({
  createFrontstagePageGroupNode: vi.fn(),
  createFrontstagePageNode: vi.fn(),
  deleteFrontstageNode: vi.fn(),
  fetchFrontstagePageTree: vi.fn(),
  frontstagePageTreeQueryKey: vi.fn((workspaceId: string) => [
    'frontstage',
    workspaceId,
    'page-tree'
  ]),
  moveFrontstageNode: vi.fn(),
  renameFrontstagePageNode: vi.fn()
}));

const pageContentApi = vi.hoisted(() => ({
  fetchFrontstagePageContent: vi.fn(),
  frontstagePageContentQueryKey: vi.fn(
    (workspaceId: string, pageId: string, tabId: string) => [
      'frontstage',
      workspaceId,
      'pages',
      pageId,
      'tabs',
      tabId,
      'content'
    ]
  )
}));
const pageTabsApi = vi.hoisted(() => ({
  fetchFrontstagePageTabs: vi.fn(),
  frontstagePageTabsQueryKey: vi.fn((workspaceId: string, pageId: string) => [
    'frontstage',
    workspaceId,
    'pages',
    pageId,
    'tabs'
  ])
}));
const consoleNavigationApi = vi.hoisted(() => ({
  settingsConsoleNavigationQueryKey: ['settings', 'console-navigation'],
  fetchSettingsConsoleNavigation: vi.fn()
}));

vi.mock('../api/page-tree', () => pageTreeApi);
vi.mock('../api/page-content', () => pageContentApi);
vi.mock('../api/page-tabs', () => pageTabsApi);
vi.mock('../../settings/api/console-navigation', () => consoleNavigationApi);

import { AppProviders } from '../../../app/AppProviders';
import { AppRouterProvider } from '../../../app/router';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';

const FRONTSTAGE_ROUTE_WIRING_TEST_TIMEOUT = 15_000;

function authenticate() {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'actor-1',
      account: 'normal-user',
      effective_display_role: 'developer',
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'user-1',
      account: 'normal-user',
      email: 'user@example.com',
      phone: null,
      nickname: 'Normal User',
      name: 'Normal User',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'developer',
      permissions: []
    }
  });
}

function createPageNode(pageId: string, title = `页面 ${pageId}`) {
  return {
    id: pageId,
    title,
    kind: 'page' as const,
    parent_id: null,
    rank: '001000',
    schema_root_uid: `root-${pageId}`,
    placement: 'topbar' as const,
    slug: 'frontstage'
  };
}

function createPageContent(pageId: string) {
  return {
    page: {
      id: pageId,
      title: `页面 ${pageId}`,
      kind: 'page' as const,
      parentId: null,
      rank: '001000',
    },
    schema: {
      rootUid: `root-${pageId}`,
      payload: { blocks: [] }
    },
    root: {
      uid: `root-${pageId}`,
      payload: { kind: 'frontstage.page.root' }
    }
  };
}

function renderApp(pathname: string) {
  window.history.pushState({}, '', pathname);

  return render(
    <AppProviders>
      <AppRouterProvider />
    </AppProviders>
  );
}

describe('frontstage page content query route wiring', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetAuthStore();
    authenticate();
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue({
      route_definitions: [
        {
          route_id: 'frontstage',
          surface_key: 'frontstage',
          path: '/frontstage',
          surface_kind: 'system'
        }
      ],
      navigation_items: [
        {
          item_id: 'frontstage',
          route_id: 'frontstage',
          parent_item_id: null,
          label_key: 'auto.frontstage',
          navigation_slot: 'primary',
          order: 1
        }
      ],
      permission_bindings: []
    });
    pageTabsApi.fetchFrontstagePageTabs.mockImplementation(
      (_workspaceId: string, pageId: string) =>
        Promise.resolve([
          {
            id: `tab-${pageId}`,
            page_id: pageId,
            title: '概览',
            rank: '001000',
            is_default: true,
            document_root_uid: `root-${pageId}`
          }
        ])
    );
  });

  test(
    'loads page detail content after resolving a route pageId to the selected page',
    async () => {
      pageTreeApi.fetchFrontstagePageTree.mockResolvedValue([
        createPageNode('page-1')
      ]);
      pageContentApi.fetchFrontstagePageContent.mockResolvedValue(
        createPageContent('page-1')
      );

      renderApp('/frontstage/pages/page-1');

      await waitFor(() => {
        expect(pageContentApi.fetchFrontstagePageContent).toHaveBeenCalledWith(
          'workspace-1',
          'page-1',
          'tab-page-1'
        );
      });
      expect((await screen.findAllByText('页面 page-1')).length).toBeGreaterThan(
        0
      );
    },
    FRONTSTAGE_ROUTE_WIRING_TEST_TIMEOUT
  );

  test.each([
    [
      'no default tab',
      [
        {
          id: 'tab-non-default',
          page_id: 'page-1',
          title: 'Non-default',
          rank: 'a',
          is_default: false,
          document_root_uid: 'root-non-default'
        }
      ]
    ],
    [
      'multiple default tabs',
      [
        {
          id: 'tab-first',
          page_id: 'page-1',
          title: 'First',
          rank: 'a',
          is_default: true,
          document_root_uid: 'root-first'
        },
        {
          id: 'tab-second',
          page_id: 'page-1',
          title: 'Second',
          rank: 'b',
          is_default: true,
          document_root_uid: 'root-second'
        }
      ]
    ]
  ])('shows an unavailable state when the page has %s', async (_case, tabs) => {
    pageTreeApi.fetchFrontstagePageTree.mockResolvedValue([
      createPageNode('page-1')
    ]);
    pageTabsApi.fetchFrontstagePageTabs.mockResolvedValue(tabs);

    renderApp('/frontstage/pages/page-1');

    expect(
      await screen.findByText('页面标签页配置不可用')
    ).toBeInTheDocument();
    expect(
      screen.getByText('当前页面必须且只能有一个默认标签页。')
    ).toBeInTheDocument();
    expect(window.location.pathname).toBe('/frontstage/pages/page-1');
    expect(pageContentApi.fetchFrontstagePageContent).not.toHaveBeenCalled();
  });

  test('passes page detail loading state to the frontstage page container', async () => {
    pageTreeApi.fetchFrontstagePageTree.mockResolvedValue([
      createPageNode('page-1')
    ]);
    pageContentApi.fetchFrontstagePageContent.mockReturnValue(
      new Promise(() => {})
    );

    renderApp('/frontstage/pages/page-1');

    await waitFor(() =>
      expect(pageContentApi.fetchFrontstagePageContent).toHaveBeenCalledWith(
        'workspace-1',
        'page-1',
        'tab-page-1'
      )
    );
  });

  test('passes page detail error state to the frontstage page container', async () => {
    pageTreeApi.fetchFrontstagePageTree.mockResolvedValue([
      createPageNode('page-1')
    ]);
    pageContentApi.fetchFrontstagePageContent.mockRejectedValue(
      new Error('load failed')
    );

    renderApp('/frontstage/pages/page-1');

    await waitFor(() =>
      expect(pageContentApi.fetchFrontstagePageContent).toHaveBeenCalledWith(
        'workspace-1',
        'page-1',
        'tab-page-1'
      )
    );
  });

  test('does not request page detail when no route pageId or selected page exists', async () => {
    pageTreeApi.fetchFrontstagePageTree.mockResolvedValue([]);
    pageContentApi.fetchFrontstagePageContent.mockResolvedValue(
      createPageContent('page-1')
    );

    renderApp('/frontstage');

    await waitFor(() =>
      expect(pageTreeApi.fetchFrontstagePageTree).toHaveBeenCalledWith(
        'workspace-1'
      )
    );
    expect(pageContentApi.fetchFrontstagePageContent).not.toHaveBeenCalled();
  });

  test(
    'normalizes page-less frontstage routes through clean page URLs',
    async () => {
      pageTreeApi.fetchFrontstagePageTree.mockResolvedValue([
        createPageNode('page-1'),
        createPageNode('page-2')
      ]);
      pageContentApi.fetchFrontstagePageContent.mockImplementation(
        (_workspaceId: string, pageId: string) =>
          Promise.resolve(createPageContent(pageId))
      );

      renderApp('/frontstage');

      await waitFor(() => {
        expect(window.location.pathname).toBe(
          '/frontstage/pages/page-1/tabs/tab-page-1'
        );
      });
      expect(window.location.pathname).not.toContain('workspace-1');
      expect(pageContentApi.fetchFrontstagePageContent).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'tab-page-1'
      );
    },
    FRONTSTAGE_ROUTE_WIRING_TEST_TIMEOUT
  );
});
