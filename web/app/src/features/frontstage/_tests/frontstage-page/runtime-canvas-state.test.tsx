import { fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { expect, vi } from 'vitest';

import { AppProviders } from '../../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import { resetFrontstageDesignModeStore } from '../../../../state/frontstage-design-mode-store';
import type {
  FrontstagePageContent,
  SaveFrontstageTabDocumentInput
} from '../../api/page-content';
import {
  createFrontstagePageContentFixture,
  type FrontstagePageContentFixtureOverrides
} from '../frontstage-page-content-fixtures';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import type { UseFrontstagePageCanvasRuntimeSessionsResult } from '../../hooks/use-frontstage-page-canvas-runtime-sessions';
import {
  insertPageIntoGroup,
  moveNodeInTree,
  removeNodeFromTree,
  renameNodeInTree
} from '../../lib/page-tree';
import { FrontStagePage } from '../../pages/FrontStagePage';

const pageContentSaveHook = vi.hoisted(() => ({
  useFrontstagePageContentSave: vi.fn()
}));
const blockCatalogHook = vi.hoisted(() => ({
  useFrontstageBlockCatalog: vi.fn()
}));
const blockCodeHook = vi.hoisted(() => ({
  useFrontstageBlockCode: vi.fn()
}));
const runtimeSessionsHook = vi.hoisted(() => ({
  clearFrontstageRuntimeSessionCache: vi.fn(),
  useFrontstagePageCanvasRuntimeSessions: vi.fn()
}));
const blockCodeApi = vi.hoisted(() => ({
  fetchFrontstageBlockCode: vi.fn(
    (_workspaceId: string, pageId: string, codeRef: string) =>
      Promise.resolve({ pageId, codeRef, code: 'export default {}' })
  ),
  frontstageBlockCodeQueryKey: vi.fn(
    (workspaceId: string, pageId: string, codeRef: string) =>
      [
        'frontstage',
        workspaceId,
        'pages',
        pageId,
        'block-code',
        codeRef
      ] as const
  ),
  saveFrontstageBlockCode: vi.fn()
}));

vi.mock(
  '../../hooks/use-frontstage-page-content-save',
  () => pageContentSaveHook
);
vi.mock('../../hooks/use-frontstage-block-catalog', () => blockCatalogHook);
vi.mock('../../hooks/use-frontstage-block-code', () => blockCodeHook);
vi.mock(
  '../../hooks/use-frontstage-page-canvas-runtime-sessions',
  () => runtimeSessionsHook
);
vi.mock('../../api/block-code', () => blockCodeApi);

const SLOW_FRONTSTAGE_TEST_TIMEOUT = 20_000;

vi.setConfig({ testTimeout: SLOW_FRONTSTAGE_TEST_TIMEOUT });

type TestFrontStageTreeNode = {
  id: string;
  title: string | null;
  icon?: string | null;
  tooltip?: string | null;
  is_hidden?: boolean;
  kind: 'group' | 'page';
  children?: TestFrontStageTreeNode[];
};

type FrontstagePageContentSaveState = {
  save: ReturnType<typeof vi.fn>;
  saving: boolean;
  isPending: boolean;
  error: Error | null;
  reset: ReturnType<typeof vi.fn>;
  clearError: ReturnType<typeof vi.fn>;
};

function authenticate(permissions: string[]) {
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
      permissions
    }
  });
}

function createBackendPage(pageId: string): TestFrontStageTreeNode {
  return {
    id: pageId,
    title: `页面 ${pageId}`,
    kind: 'page'
  };
}

function updateNodeMetadataInTree(
  nodes: TestFrontStageTreeNode[],
  nodeId: string,
  input: { icon?: string | null; tooltip?: string | null; isHidden?: boolean }
): TestFrontStageTreeNode[] {
  return nodes.map((node) => {
    const nextNode =
      node.id === nodeId
        ? {
            ...node,
            icon: Object.prototype.hasOwnProperty.call(input, 'icon')
              ? input.icon
              : node.icon,
            tooltip: Object.prototype.hasOwnProperty.call(input, 'tooltip')
              ? input.tooltip
              : node.tooltip,
            is_hidden: Object.prototype.hasOwnProperty.call(input, 'isHidden')
              ? input.isHidden
              : node.is_hidden
          }
        : node;

    return {
      ...nextNode,
      children: nextNode.children
        ? updateNodeMetadataInTree(nextNode.children, nodeId, input)
        : nextNode.children
    };
  });
}

function createPageContent(
  overrides: FrontstagePageContentFixtureOverrides = {}
): FrontstagePageContent {
  return createFrontstagePageContentFixture(overrides);
}

function createSavedPageContentFromInput(
  input: SaveFrontstageTabDocumentInput
): FrontstagePageContent {
  return createPageContent({
    schema: {
      rootUid: 'root-1',
      payload: input.payload
    },
    root: {
      uid: 'root-1',
      payload: input.payload
    }
  });
}

function createTestNodeId() {
  return crypto.randomUUID();
}

function FrontStagePageHarness({
  workspaceId = 'workspace-1',
  pageId,
  onNavigatePage,
  initialPageTree,
  pageContent,
  isPageContentLoading,
  hasPageContentLoadError
}: {
  workspaceId?: string;
  pageId?: string;
  onNavigatePage?: (pageId?: string) => void;
  initialPageTree?: TestFrontStageTreeNode[];
  pageContent?: FrontstagePageContent;
  isPageContentLoading?: boolean;
  hasPageContentLoadError?: boolean;
}) {
  const [pageTree, setPageTree] = useState<TestFrontStageTreeNode[]>(
    initialPageTree ?? []
  );

  return (
    <FrontStagePage
      workspaceId={workspaceId}
      pageId={pageId}
      onNavigatePage={onNavigatePage}
      initialPageTree={pageTree}
      pageContent={pageContent}
      isPageContentLoading={isPageContentLoading}
      hasPageContentLoadError={hasPageContentLoadError}
      onCreateGroupNode={(input) => {
        const groupNode = {
          id: createTestNodeId(),
          title: input.title,
          icon: input.icon,
          tooltip: input.tooltip,
          kind: 'group' as const,
          children: []
        };
        setPageTree((currentTree) => [...currentTree, groupNode]);
        return Promise.resolve({ id: groupNode.id, kind: groupNode.kind });
      }}
      onCreatePageNode={(input) => {
        const pageNode = {
          id: createTestNodeId(),
          title: input.title,
          icon: input.icon,
          tooltip: input.tooltip,
          kind: 'page' as const
        };
        setPageTree((currentTree) =>
          input.parentId
            ? insertPageIntoGroup(currentTree, input.parentId, pageNode)
            : [...currentTree, pageNode]
        );
        return Promise.resolve({ id: pageNode.id, kind: pageNode.kind });
      }}
      onRenamePageNode={(nodeId, input) => {
        setPageTree((currentTree) =>
          updateNodeMetadataInTree(
            renameNodeInTree(currentTree, nodeId, input.title ?? ''),
            nodeId,
            {
              icon: input.icon,
              tooltip: input.tooltip
            }
          )
        );
        return Promise.resolve({ id: nodeId, kind: 'page' });
      }}
      onMovePageNode={(nodeId, input) => {
        setPageTree((currentTree) =>
          moveNodeInTree(currentTree, nodeId, input.rank === '000000' ? -1 : 1)
        );
        return Promise.resolve({ id: nodeId, kind: 'page' });
      }}
      onDeletePageNode={(nodeId) => {
        setPageTree((currentTree) => removeNodeFromTree(currentTree, nodeId));
        return Promise.resolve();
      }}
    />
  );
}

function renderPage(
  pageId?: string,
  onNavigatePage?: (pageId?: string) => void
) {
  return render(
    <AppProviders>
      <FrontStagePageHarness
        pageId={pageId}
        onNavigatePage={onNavigatePage}
        initialPageTree={pageId ? [createBackendPage(pageId)] : undefined}
      />
    </AppProviders>
  );
}

function renderPageWithInitialTree(
  pageTree: TestFrontStageTreeNode[],
  pageId?: string,
  onNavigatePage?: (pageId?: string) => void
) {
  return render(
    <AppProviders>
      <FrontStagePageHarness
        pageId={pageId}
        onNavigatePage={onNavigatePage}
        initialPageTree={pageTree}
      />
    </AppProviders>
  );
}

function mockPageContentSaveState(
  overrides: Partial<FrontstagePageContentSaveState> = {}
): FrontstagePageContentSaveState {
  const state = {
    save: vi.fn((input: SaveFrontstageTabDocumentInput) =>
      Promise.resolve(createSavedPageContentFromInput(input))
    ),
    saving: false,
    isPending: false,
    error: null,
    reset: vi.fn(),
    clearError: vi.fn(),
    ...overrides
  };

  pageContentSaveHook.useFrontstagePageContentSave.mockReturnValue(state);
  return state;
}

function createCatalogEntry(
  overrides: Partial<NormalizedFrontstageBlockCatalogEntry> = {}
): NormalizedFrontstageBlockCatalogEntry {
  return {
    id: '1flowbase:frontstage.js-ui-block',
    runtimeKind: 'iframe',
    installationId: 'builtin-installation',
    providerCode: '1flowbase',
    pluginId: 'builtin-frontstage',
    pluginVersion: '1.0.0',
    contributionCode: 'frontstage.js-ui-block',
    title: '空白 JS Block',
    entry: 'index.js',
    permissions: {
      network: 'none',
      storage: 'none',
      secrets: 'none'
    },
    contextContract: {
      primitives: [],
      inputSchema: {}
    },
    uiCapabilities: [],
    raw: {} as NormalizedFrontstageBlockCatalogEntry['raw'],
    ...overrides
  };
}

function createCatalogMatchedBlockPayload(
  overrides: Record<string, unknown> = {}
): Record<string, unknown> {
  return {
    id: 'frontstage-js-block-1',
    renderer_version: 'v1',
    codeRef: 'frontstage-js-block-1-code',
    catalog: {
      providerCode: '1flowbase',
      installationId: 'builtin-installation'
    },
    contribution: {
      pluginId: 'builtin-frontstage',
      pluginVersion: '1.0.0',
      code: 'frontstage.js-ui-block'
    },
    props: {
      title: 'Landing hero'
    },
    'x-layout': {
      order: 0,
      region: 'main'
    },
    runtime: {
      kind: 'iframe',
      entry: 'index.js',
      hint: 'iframe'
    },
    ...overrides
  };
}

function mockFrontstageBlockCatalog(
  items: NormalizedFrontstageBlockCatalogEntry[] = []
) {
  blockCatalogHook.useFrontstageBlockCatalog.mockReturnValue({
    items,
    diagnostics: [],
    loading: false,
    error: null
  });
}

function mockFrontstageBlockCode() {
  blockCodeHook.useFrontstageBlockCode.mockReturnValue({
    code: '',
    draft: '',
    dirty: false,
    loading: false,
    saving: false,
    error: null,
    setDraft: vi.fn(),
    reset: vi.fn(),
    save: vi.fn()
  });
}

function mockRuntimeSessions(
  overrides: Partial<UseFrontstagePageCanvasRuntimeSessionsResult> = {}
) {
  runtimeSessionsHook.useFrontstagePageCanvasRuntimeSessions.mockReturnValue({
    entries: [],
    snapshotsBySlot: {},
    running: false,
    hasError: false,
    ...overrides
  });
}
describe('FrontStagePage - runtime canvas state', () => {
  beforeEach(() => {
    resetAuthStore();
    resetFrontstageDesignModeStore();
    vi.clearAllMocks();
    mockPageContentSaveState();
    mockFrontstageBlockCatalog();
    mockFrontstageBlockCode();
    mockRuntimeSessions();
    blockCodeApi.saveFrontstageBlockCode.mockResolvedValue({
      pageId: 'page-1',
      codeRef: 'frontstage-js-block-1-code',
      code: 'saved template'
    });
  });

  test('shows manager shell and canvas placeholders', () => {
    authenticate(['frontstage.page.design']);
    renderPage('page-1');

    expect(screen.getByTestId('section-page-layout')).toBeInTheDocument();
    expect(
      screen.queryByRole('heading', { name: '前台' })
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('heading', { name: '页面 page-1' })
    ).toBeInTheDocument();
    expect(screen.queryByText('当前页面：页面 page-1')).not.toBeInTheDocument();
    expect(
      screen
        .getByTestId('frontstage-page-workspace')
        .querySelector('.ant-empty')
    ).toBeInTheDocument();
    expect(screen.getAllByText('页面 page-1').length).toBeGreaterThan(0);
  });

  test('connects ready runtime run plan state into the PageCanvas slot', async () => {
    authenticate([]);
    mockFrontstageBlockCatalog([createCatalogEntry()]);

    render(
      <AppProviders>
        <FrontStagePageHarness
          pageId="page-1"
          initialPageTree={[createBackendPage('page-1')]}
          pageContent={createPageContent({
            root: {
              uid: 'root-1',
              payload: {
                blocks: [createCatalogMatchedBlockPayload()]
              }
            }
          })}
        />
      </AppProviders>
    );

    expect(
      await screen.findByTestId('block-slot-frontstage-js-block-1')
    ).toBeInTheDocument();
    expect(screen.queryByText('区块加载中...')).not.toBeInTheDocument();
    expect(blockCodeApi.fetchFrontstageBlockCode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'frontstage-js-block-1-code'
    );
  });

  test('surfaces catalog-missing runtime run plan state from the PageCanvas container', async () => {
    authenticate([]);
    mockFrontstageBlockCatalog([]);

    render(
      <AppProviders>
        <FrontStagePageHarness
          pageId="page-1"
          initialPageTree={[createBackendPage('page-1')]}
          pageContent={createPageContent({
            root: {
              uid: 'root-1',
              payload: {
                blocks: [createCatalogMatchedBlockPayload()]
              }
            }
          })}
        />
      </AppProviders>
    );

    expect(
      await screen.findByTestId('block-slot-frontstage-js-block-1')
    ).toBeInTheDocument();
    expect(screen.queryByText('区块加载中...')).not.toBeInTheDocument();
  });

  test('keeps the empty page tree free of placeholder content', () => {
    authenticate(['frontstage.page.design']);
    renderPage();

    expect(
      screen
        .getByTestId('frontstage-page-workspace')
        .querySelector('.ant-empty')
    ).toBeInTheDocument();
    expect(
      screen
        .queryByTestId('frontstage-page-workspace')
        ?.querySelector('.frontstage-page-workspace__header')
    ).not.toBeInTheDocument();
    expect(
      document
        .querySelector('.frontstage-page-tree-sidebar')
        ?.querySelector('.ant-empty')
    ).not.toBeInTheDocument();
  });

  test('supports nullable page title from initial tree', () => {
    authenticate(['frontstage.page.design']);

    renderPageWithInitialTree([
      {
        id: 'page-null-title',
        title: null,
        kind: 'page'
      }
    ]);

    expect(screen.getAllByText('未命名页面').length).toBeGreaterThan(0);
  });

  test('uses tree page title as current page label and page header title', () => {
    authenticate(['frontstage.page.design']);

    renderPageWithInitialTree([
      {
        id: 'page-custom-title',
        title: '我的自定义主页',
        kind: 'page'
      }
    ]);

    expect(screen.getByRole('list')).toHaveTextContent('我的自定义主页');
    expect(
      screen.getByRole('heading', { name: '我的自定义主页' })
    ).toBeInTheDocument();
    expect(screen.getAllByText('我的自定义主页').length).toBeGreaterThan(0);
  });

  test('shows loading state when page tree is being loaded for the first time', () => {
    authenticate(['frontstage.page.design']);

    render(
      <AppProviders>
        <FrontStagePage workspaceId="workspace-1" isPageTreeLoading />
      </AppProviders>
    );

    expect(screen.getByText('页面树加载中…')).toBeInTheDocument();
    expect(screen.getByText('正在加载页面树，请稍后...')).toBeInTheDocument();
  });

  test('shows error state with retry when page tree load fails before any cached tree is available', () => {
    authenticate(['frontstage.page.design']);

    const onRetryLoadPageTree = vi.fn();

    render(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          hasPageTreeLoadError
          onRetryLoadPageTree={onRetryLoadPageTree}
        />
      </AppProviders>
    );

    expect(screen.getByText('页面树加载失败')).toBeInTheDocument();
    expect(
      screen.getByText(
        '页面树加载失败，请检查网络后重试。点击“重试”按钮重新发起加载。'
      )
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /重\s*试/ }));
    expect(onRetryLoadPageTree).toHaveBeenCalledTimes(1);
  });

  test('shows partial error banner when page tree load fails but cached tree exists', () => {
    authenticate(['frontstage.page.design']);

    const onRetryLoadPageTree = vi.fn();

    render(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          pageId="page-1"
          initialPageTree={[
            {
              id: 'page-1',
              title: '页面 内页',
              kind: 'page'
            }
          ]}
          hasPageTreeLoadError
          onRetryLoadPageTree={onRetryLoadPageTree}
        />
      </AppProviders>
    );

    expect(screen.getByText('页面树加载失败')).toBeInTheDocument();
    expect(
      screen.getByText(
        '页面树加载失败，当前页面树仍可查看；请点击“重试”恢复最新数据。'
      )
    ).toBeInTheDocument();
    expect(
      screen.getByRole('heading', { name: '页面 内页' })
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /重\s*试/ }));
    expect(onRetryLoadPageTree).toHaveBeenCalledTimes(1);
  });
});
