import { act, render, screen, waitFor } from '@testing-library/react';
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
  frontstagePageContentQueryKey: vi.fn(
    (workspaceId: string, pageId: string, tabReference: string) => [
      'frontstage',
      workspaceId,
      'pages',
      pageId,
      'tabs',
      tabReference,
      'content'
    ]
  ),
  fetchFrontstagePageContent: vi.fn()
}));

const blockApi = vi.hoisted(() => ({
  frontstageBlockTreeQueryKeys: {
    roots: (workspaceId: string, pageId: string, query: unknown) => [
      'frontstage',
      workspaceId,
      pageId,
      'roots',
      query
    ],
    runtimeAssembly: (workspaceId: string, pageId: string, blockId: string) => [
      'frontstage',
      workspaceId,
      pageId,
      blockId,
      'runtime-assembly'
    ]
  },
  fetchFrontstageBlockRoots: vi.fn(),
  fetchFrontstageBlockRuntimeAssembly: vi.fn()
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
    blockRoots?: Array<{ block_id: string; tab_id: string }>;
    blockRuntimeAssembly?: { layers: Array<{ block_id: string }> };
    isBlockRuntimeRoute?: boolean;
    isBlockRuntimeLoading?: boolean;
    hasBlockRuntimeLoadError?: boolean;
    isBlockRuntimePermissionDenied?: boolean;
    onRetryLoadBlockRuntime?: () => void;
    onNavigateBlock?: (blockId: string | null, replace?: boolean) => void;
    onMovePageNode?: (
      nodeId: string,
      input: { parentId: string | null; rank: string }
    ) => Promise<unknown>;
    onNavigateTab?: (tab: {
      id: string;
      page_id: string;
      title: string | null;
      rank: string;
      is_default: boolean;
      route_segment: string | null;
      document_root_uid: string;
    }) => void;
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
vi.mock('../api/block-tree', () => blockApi);
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
    blockApi.fetchFrontstageBlockRoots.mockResolvedValue([]);
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
      expect(frontStagePageView.props?.onMovePageNode).toEqual(
        expect.any(Function)
      );
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

  test('AC-004 preserves default and non-default tab navigation URLs', async () => {
    const defaultTab = {
      id: 'tab-overview',
      page_id: 'page-top-level',
      title: 'Overview',
      rank: '001000',
      is_default: true,
      route_segment: null,
      document_root_uid: 'root-overview'
    };
    const analyticsTab = {
      id: 'tab-analytics',
      page_id: 'page-top-level',
      title: 'Analytics',
      rank: '002000',
      is_default: false,
      route_segment: 'analytics',
      document_root_uid: 'root-analytics'
    };
    pageTreeApi.fetchFrontstagePageTree.mockResolvedValue([topbarGroup]);
    pageTabsApi.fetchFrontstagePageTabs.mockResolvedValue([
      defaultTab,
      analyticsTab
    ]);
    pageContentApi.fetchFrontstagePageContent.mockReturnValue(
      new Promise(() => undefined)
    );
    window.history.pushState({}, '', '/sales/pages/page-top-level');

    render(
      <AppProviders>
        <AppRouterProvider />
      </AppProviders>
    );

    await waitFor(() => {
      expect(frontStagePageView.props?.onNavigateTab).toEqual(
        expect.any(Function)
      );
    });
    act(() => {
      frontStagePageView.props?.onNavigateTab?.(analyticsTab);
    });
    await waitFor(() => {
      expect(window.location.pathname).toBe(
        '/sales/pages/page-top-level/tabs/analytics'
      );
    });

    act(() => {
      frontStagePageView.props?.onNavigateTab?.(defaultTab);
    });
    await waitFor(() => {
      expect(window.location.pathname).toBe('/sales/pages/page-top-level');
    });
  });

  test('keeps the page URL and loads roots from the active tab', async () => {
    const defaultTab = {
      id: 'tab-overview',
      page_id: 'page-top-level',
      title: 'Overview',
      rank: '001000',
      is_default: true,
      route_segment: null,
      document_root_uid: 'root-overview'
    };
    const roots = [
      {
        block_id: 'root-block',
        tab_id: 'tab-overview'
      }
    ];
    pageTreeApi.fetchFrontstagePageTree.mockResolvedValue([topbarGroup]);
    pageTabsApi.fetchFrontstagePageTabs.mockResolvedValue([defaultTab]);
    pageContentApi.fetchFrontstagePageContent.mockResolvedValue({
      page: { id: 'page-top-level' },
      tab: defaultTab,
      document: { rootUid: 'root-overview', payload: {} }
    });
    blockApi.fetchFrontstageBlockRoots.mockResolvedValue(roots);
    window.history.pushState({}, '', '/sales/pages/page-top-level');

    render(
      <AppProviders>
        <AppRouterProvider />
      </AppProviders>
    );

    await waitFor(() => {
      expect(frontStagePageView.props?.blockRoots).toEqual(roots);
    });
    expect(blockApi.fetchFrontstageBlockRoots).toHaveBeenCalledWith(
      'workspace-1',
      'page-top-level',
      { tab_id: 'tab-overview' }
    );
    expect(window.location.pathname).toBe('/sales/pages/page-top-level');
  });

  test('resolves a non-default tab route segment before loading roots', async () => {
    const analyticsTab = {
      id: 'tab-analytics',
      page_id: 'page-top-level',
      title: 'Analytics',
      rank: '002000',
      is_default: false,
      route_segment: 'analytics',
      document_root_uid: 'root-analytics'
    };
    pageTreeApi.fetchFrontstagePageTree.mockResolvedValue([topbarGroup]);
    pageTabsApi.fetchFrontstagePageTabs.mockResolvedValue([analyticsTab]);
    pageContentApi.fetchFrontstagePageContent.mockResolvedValue({
      page: { id: 'page-top-level' },
      tab: analyticsTab,
      document: { rootUid: 'root-analytics', payload: {} }
    });
    blockApi.fetchFrontstageBlockRoots.mockResolvedValue([]);
    window.history.pushState(
      {},
      '',
      '/sales/pages/page-top-level/tabs/analytics'
    );

    render(
      <AppProviders>
        <AppRouterProvider />
      </AppProviders>
    );

    await waitFor(() => {
      expect(blockApi.fetchFrontstageBlockRoots).toHaveBeenCalledWith(
        'workspace-1',
        'page-top-level',
        { tab_id: 'tab-analytics' }
      );
    });
  });

  test('AC-006 restores a canonical block deep link and browser history from one assembly query', async () => {
    pageTreeApi.fetchFrontstagePageTree.mockResolvedValue([topbarGroup]);
    blockApi.fetchFrontstageBlockRuntimeAssembly.mockResolvedValue({
      layers: [
        {
          block_id: 'block-parent',
          tab_id: 'tab-overview',
          parent_block_id: null,
          presentation: 'page',
          title: 'Parent',
          schema_version: 1,
          input_mapping: {},
          output_mapping: {},
          runtime_descriptor: {},
          source_code: 'export default function Parent() { return null; }',
          source_sha256: 'a'.repeat(64),
          dependency_lock: [],
          tailwind_toolchain_lock: { package: 'tailwindcss' },
          generated_css: '',
          generated_css_sha256:
            'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
          compiler_identity: { name: 'tailwindcss' },
          executable_state: 'ready'
        },
        {
          block_id: 'block-child',
          tab_id: 'tab-overview',
          parent_block_id: 'block-parent',
          presentation: 'drawer',
          title: 'Child',
          schema_version: 1,
          input_mapping: {},
          output_mapping: {},
          runtime_descriptor: {},
          source_code: 'export default function Child() { return null; }',
          source_sha256: 'b'.repeat(64),
          dependency_lock: [],
          tailwind_toolchain_lock: { package: 'tailwindcss' },
          generated_css: '',
          generated_css_sha256:
            'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
          compiler_identity: { name: 'tailwindcss' },
          executable_state: 'ready'
        }
      ]
    });
    pageContentApi.fetchFrontstagePageContent.mockResolvedValue({
      page: {
        id: 'page-top-level',
        title: 'Top-level page',
        kind: 'page',
        parentId: 'group-sales',
        rank: '001000',
        contentPresentation: 'single'
      },
      tab: {
        id: 'tab-overview',
        pageId: 'page-top-level',
        title: 'Overview',
        rank: '001000',
        isDefault: true,
        routeSegment: null,
        documentRootUid: 'root-overview'
      },
      document: { rootUid: 'root-overview', payload: { blocks: [] } }
    });
    window.history.pushState(
      {},
      '',
      '/sales/pages/page-top-level/blocks/block-child'
    );

    render(
      <AppProviders>
        <AppRouterProvider />
      </AppProviders>
    );

    await waitFor(() => {
      expect(
        frontStagePageView.props?.blockRuntimeAssembly?.layers.at(-1)?.block_id
      ).toBe('block-child');
    });
    expect(frontStagePageView.props?.isBlockRuntimeRoute).toBe(true);
    expect(blockApi.fetchFrontstageBlockRuntimeAssembly).toHaveBeenCalledWith(
      'workspace-1',
      'page-top-level',
      'block-child'
    );
    expect(pageTabsApi.fetchFrontstagePageTabs).not.toHaveBeenCalled();
    expect(pageContentApi.fetchFrontstagePageContent).not.toHaveBeenCalled();

    act(() => frontStagePageView.props?.onNavigateBlock?.('block-parent'));
    await waitFor(() => {
      expect(window.location.pathname).toBe(
        '/sales/pages/page-top-level/blocks/block-parent'
      );
    });
    await waitFor(() => {
      expect(blockApi.fetchFrontstageBlockRuntimeAssembly).toHaveBeenCalledWith(
        'workspace-1',
        'page-top-level',
        'block-parent'
      );
    });
    act(() => window.history.back());
    await waitFor(() => {
      expect(window.location.pathname).toBe(
        '/sales/pages/page-top-level/blocks/block-child'
      );
    });
    await waitFor(() => {
      expect(
        frontStagePageView.props?.blockRuntimeAssembly?.layers.at(-1)?.block_id
      ).toBe('block-child');
    });
  });

  test('passes assembly loading and forbidden errors through one query state', async () => {
    pageTreeApi.fetchFrontstagePageTree.mockResolvedValue([topbarGroup]);
    let rejectAssembly:
      | ((error: Error & { status: number }) => void)
      | undefined;
    blockApi.fetchFrontstageBlockRuntimeAssembly.mockImplementation(
      () =>
        new Promise((_resolve, reject) => {
          rejectAssembly = reject;
        })
    );
    window.history.pushState(
      {},
      '',
      '/sales/pages/page-top-level/blocks/forbidden-block'
    );

    render(
      <AppProviders>
        <AppRouterProvider />
      </AppProviders>
    );

    await waitFor(() => {
      expect(frontStagePageView.props?.isBlockRuntimeLoading).toBe(true);
    });
    act(() => {
      rejectAssembly?.(Object.assign(new Error('forbidden'), { status: 403 }));
    });
    await waitFor(() => {
      expect(frontStagePageView.props?.hasBlockRuntimeLoadError).toBe(true);
      expect(frontStagePageView.props?.isBlockRuntimeLoading).toBe(false);
      expect(frontStagePageView.props?.isBlockRuntimePermissionDenied).toBe(
        true
      );
      expect(frontStagePageView.props?.onRetryLoadBlockRuntime).toEqual(
        expect.any(Function)
      );
    });
    expect(pageContentApi.fetchFrontstagePageContent).not.toHaveBeenCalled();
  });

  test('AC-008 renders controlled NotFound for a missing block deep link', async () => {
    pageTreeApi.fetchFrontstagePageTree.mockResolvedValue([topbarGroup]);
    blockApi.fetchFrontstageBlockRuntimeAssembly.mockRejectedValue(
      Object.assign(new Error('block_node_not_found'), { status: 404 })
    );
    window.history.pushState(
      {},
      '',
      '/sales/pages/page-top-level/blocks/missing-block'
    );

    render(
      <AppProviders>
        <AppRouterProvider />
      </AppProviders>
    );

    expect(await screen.findByText('页面不存在')).toBeInTheDocument();
    expect(window.location.pathname).toBe(
      '/sales/pages/page-top-level/blocks/missing-block'
    );
  });
});
