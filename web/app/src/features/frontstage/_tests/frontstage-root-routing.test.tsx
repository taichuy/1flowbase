import { act, render, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const pageTreeApi = vi.hoisted(() => ({
  frontstagePageTreeQueryKey: (workspaceId: string) => [
    'frontstage',
    workspaceId,
    'page-tree'
  ],
  fetchFrontstagePageTree: vi.fn()
}));

const pageTabsApi = vi.hoisted(() => ({
  frontstagePageTabsQueryKey: (workspaceId: string, pageId: string) => [
    'frontstage',
    workspaceId,
    'pages',
    pageId,
    'tabs'
  ],
  fetchFrontstagePageTabs: vi.fn(() => new Promise(() => undefined))
}));

const pageContentApi = vi.hoisted(() => ({
  frontstagePageContentQueryKey: vi.fn(),
  fetchFrontstagePageContent: vi.fn()
}));

const pageTreeMutations = vi.hoisted(() => {
  const moveNode = vi.fn();
  return {
    moveNode,
    useFrontstagePageTreeMutations: vi.fn(() => ({
      isPending: false,
      error: null,
      createGroup: vi.fn(),
      createPage: vi.fn(),
      renameNode: vi.fn(),
      updateNodeMetadata: vi.fn(),
      moveNode,
      deleteNode: vi.fn()
    }))
  };
});

const frontStagePageView = vi.hoisted(() => ({
  props: null as null | {
    onMovePageNode?: (
      nodeId: string,
      input: { parentId: string | null; rank: string }
    ) => Promise<unknown>;
  }
}));

const consoleNavigationApi = vi.hoisted(() => ({
  settingsConsoleNavigationQueryKey: ['settings', 'console-navigation'],
  fetchSettingsConsoleNavigation: vi.fn(() =>
    Promise.resolve({
      route_definitions: [],
      navigation_items: [],
      permission_bindings: []
    })
  )
}));

vi.mock('../api/page-tree', () => pageTreeApi);
vi.mock('../api/page-tabs', () => pageTabsApi);
vi.mock('../api/page-content', () => pageContentApi);
vi.mock('../hooks/use-frontstage-page-tree-mutations', () => pageTreeMutations);
vi.mock('../../settings/api/console-navigation', () => consoleNavigationApi);
vi.mock('../pages/FrontStagePage', () => ({
  FrontStagePage: (props: NonNullable<typeof frontStagePageView.props>) => {
    frontStagePageView.props = props;
    return <div data-testid="frontstage-page" />;
  }
}));

import { AppProviders } from '../../../app/AppProviders';
import { AppRouterProvider } from '../../../app/router';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';

const topbarGroup = {
  id: 'group-sales',
  title: 'Sales',
  kind: 'group' as const,
  placement: 'topbar' as const,
  slug: 'sales',
  content_presentation: 'single' as const,
  children: [
    {
      id: 'group-collapsed',
      title: 'Collapsed',
      kind: 'group' as const,
      placement: 'sidebar' as const,
      content_presentation: 'single' as const,
      children: [
        {
          id: 'page-in-group',
          title: 'Grouped page',
          kind: 'page' as const,
          placement: 'sidebar' as const,
          content_presentation: 'single' as const,
          children: []
        }
      ]
    },
    {
      id: 'page-top-level',
      title: 'Top-level page',
      kind: 'page' as const,
      placement: 'sidebar' as const,
      content_presentation: 'single' as const,
      children: []
    }
  ]
};

describe('frontstage topbar root routing', () => {
  beforeEach(() => {
    resetAuthStore();
    vi.clearAllMocks();
    frontStagePageView.props = null;
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-123',
      actor: {
        id: 'actor-1',
        account: 'root',
        effective_display_role: 'root',
        current_workspace_id: 'workspace-1'
      },
      me: null
    });
  });

  test('AC-001 redirects a topbar group root to its first direct page', async () => {
    pageTreeApi.fetchFrontstagePageTree.mockResolvedValue([topbarGroup]);
    window.history.pushState({}, '', '/sales');

    render(
      <AppProviders>
        <AppRouterProvider />
      </AppProviders>
    );

    await waitFor(() => {
      expect(window.location.pathname).toBe('/sales/pages/page-top-level');
    });
  });

  test('AC-002 keeps the group root when it only contains grouped pages', async () => {
    pageTreeApi.fetchFrontstagePageTree.mockResolvedValue([
      {
        ...topbarGroup,
        children: topbarGroup.children.slice(0, 1)
      }
    ]);
    window.history.pushState({}, '', '/sales');

    render(
      <AppProviders>
        <AppRouterProvider />
      </AppProviders>
    );

    await waitFor(() => {
      expect(pageTreeApi.fetchFrontstagePageTree).toHaveBeenCalled();
    });
    expect(window.location.pathname).toBe('/sales');
  });

  test('AC-003 preserves an explicit deep link to a grouped page', async () => {
    pageTreeApi.fetchFrontstagePageTree.mockResolvedValue([topbarGroup]);
    window.history.pushState({}, '', '/sales/pages/page-in-group');

    render(
      <AppProviders>
        <AppRouterProvider />
      </AppProviders>
    );

    await waitFor(() => {
      expect(pageTreeApi.fetchFrontstagePageTree).toHaveBeenCalled();
    });
    expect(window.location.pathname).toBe('/sales/pages/page-in-group');
  });

  test('AC-004 keeps an ungrouped page inside the scoped topbar root', async () => {
    pageTreeApi.fetchFrontstagePageTree.mockResolvedValue([topbarGroup]);
    window.history.pushState({}, '', '/sales/pages/page-top-level');

    render(
      <AppProviders>
        <AppRouterProvider />
      </AppProviders>
    );

    await waitFor(() => {
      expect(frontStagePageView.props?.onMovePageNode).toBeDefined();
    });
    await act(async () => {
      await frontStagePageView.props?.onMovePageNode?.('page-in-group', {
        parentId: null,
        rank: '001500'
      });
    });

    expect(pageTreeMutations.moveNode).toHaveBeenCalledWith('page-in-group', {
      parentId: 'group-sales',
      rank: '001500'
    });

    await act(async () => {
      await frontStagePageView.props?.onMovePageNode?.('page-top-level', {
        parentId: 'group-collapsed',
        rank: '002000'
      });
    });
    expect(pageTreeMutations.moveNode).toHaveBeenLastCalledWith(
      'page-top-level',
      {
        parentId: 'group-collapsed',
        rank: '002000'
      }
    );
  });
});
