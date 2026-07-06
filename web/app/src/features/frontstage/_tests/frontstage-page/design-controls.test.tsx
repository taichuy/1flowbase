import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react';
import { useState } from 'react';
import { expect, vi } from 'vitest';

import { AppProviders } from '../../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import {
  resetFrontstageDesignModeStore,
  useFrontstageDesignModeStore
} from '../../../../state/frontstage-design-mode-store';
import type {
  FrontstagePageContent,
  SaveFrontstagePageContentInput
} from '../../api/page-content';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import { createFrontstageBuiltInJsBlockTemplateCode } from '../../lib/block-templates';
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
  overrides: Partial<FrontstagePageContent> = {}
): FrontstagePageContent {
  return {
    page: {
      id: 'page-1',
      title: 'Landing',
      kind: 'page',
      parentId: null,
      rank: '001000',
      schemaRootUid: 'root-1'
    },
    schema: {
      rootUid: 'root-1',
      payload: {}
    },
    root: {
      uid: 'root-1',
      payload: {}
    },
    ...overrides
  };
}

function createSavedPageContentFromInput(
  input: SaveFrontstagePageContentInput
): FrontstagePageContent {
  return createPageContent({
    schema: {
      rootUid: 'root-1',
      payload: input.schema.payload
    },
    root: {
      uid: 'root-1',
      payload: input.root.payload
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

async function hoverAddMenuAndFlush() {
  fireEvent.mouseEnter(screen.getByRole('button', { name: '添加菜单' }));
}

function activateDesignMode() {
  act(() => {
    useFrontstageDesignModeStore.getState().setDesignMode(true);
  });
}

function exitDesignMode() {
  act(() => {
    useFrontstageDesignModeStore.getState().setDesignMode(false);
  });
}

function mockPageContentSaveState(
  overrides: Partial<FrontstagePageContentSaveState> = {}
): FrontstagePageContentSaveState {
  const state = {
    save: vi.fn((input: SaveFrontstagePageContentInput) =>
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

function getSavedBlocks(input: SaveFrontstagePageContentInput) {
  const payload = input.root.payload;
  if (typeof payload !== 'object' || payload === null) {
    throw new Error('root payload must be an object');
  }

  const blocks = (payload as { blocks?: unknown }).blocks;
  if (!Array.isArray(blocks)) {
    throw new Error('root payload blocks must be an array');
  }

  return blocks as Array<Record<string, unknown>>;
}

describe('FrontStagePage - design controls', () => {
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

  test('shows page context and design mode is unavailable without permission', () => {
    authenticate([]);
    renderPage('page-1');

    expect(screen.getAllByText('页面 page-1').length).toBeGreaterThan(0);
    expect(
      screen.queryByRole('button', { name: '进入设计模式' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '创建区块' })
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('heading', { name: '页面 page-1' })
    ).toBeInTheDocument();
  });

  test('shows design controls from shared design mode state', async () => {
    authenticate(['frontstage.page.design']);
    renderPage('page-1');

    expect(
      screen.queryByRole('button', { name: '创建区块' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'JS Block 试运行' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('页面树已同步')).not.toBeInTheDocument();

    activateDesignMode();
    expect(
      screen.getByRole('button', { name: '创建区块' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'JS Block 试运行' })
    ).not.toBeInTheDocument();
    expect(screen.getByText('页面树已同步')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '添加菜单' })
    ).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: '添加菜单' })).toHaveLength(1);
    expect(
      screen.queryByRole('menuitem', { name: '新增分组' })
    ).not.toBeInTheDocument();
    await hoverAddMenuAndFlush();
    expect(
      await screen.findByRole('menuitem', { name: '新增分组' })
    ).toBeInTheDocument();
    expect(
      await screen.findByRole('menuitem', { name: '新增页面' })
    ).toBeInTheDocument();
    fireEvent.mouseLeave(screen.getByRole('button', { name: '添加菜单' }));
    expect(
      screen.queryByRole('menuitem', { name: '新增分组' })
    ).not.toBeInTheDocument();
    exitDesignMode();
    expect(
      screen.queryByRole('button', { name: '创建区块' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'JS Block 试运行' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('页面树已同步')).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '添加菜单' })
    ).not.toBeInTheDocument();
  });

  test('shows real page tree operation states without local draft wording', () => {
    authenticate(['frontstage.page.design']);
    const view = render(
      <AppProviders>
        <FrontStagePage workspaceId="workspace-1" initialPageTree={[]} />
      </AppProviders>
    );

    activateDesignMode();
    expect(screen.getByText('页面树已同步')).toBeInTheDocument();
    expect(screen.queryByText(/本地草稿/)).not.toBeInTheDocument();

    view.rerender(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          initialPageTree={[]}
          isPageTreeMutating
        />
      </AppProviders>
    );
    expect(screen.getByText('保存中')).toBeInTheDocument();

    view.rerender(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          initialPageTree={[]}
          pageTreeMutationError={new Error('failed')}
        />
      </AppProviders>
    );
    expect(screen.getByText('操作失败')).toBeInTheDocument();
  });

  test('keeps mutation status scoped to design mode controls', () => {
    authenticate(['frontstage.page.design']);
    render(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          initialPageTree={[]}
          isPageTreeMutating
        />
      </AppProviders>
    );

    activateDesignMode();
    expect(screen.getByText('保存中')).toBeInTheDocument();

    exitDesignMode();
    expect(screen.queryByText('保存中')).not.toBeInTheDocument();
    activateDesignMode();

    expect(screen.getByText('保存中')).toBeInTheDocument();
  });

  test('saves selected catalog block and writes blank JS template code', async () => {
    authenticate(['frontstage.page.design']);
    mockFrontstageBlockCatalog([createCatalogEntry()]);
    const saveState = mockPageContentSaveState();

    render(
      <AppProviders>
        <FrontStagePageHarness
          pageId="page-1"
          initialPageTree={[createBackendPage('page-1')]}
          pageContent={createPageContent()}
        />
      </AppProviders>
    );

    activateDesignMode();
    fireEvent.click(screen.getByRole('button', { name: '创建区块' }));
    fireEvent.click(await screen.findByRole('button', { name: '选择' }));

    await waitFor(() => {
      expect(saveState.save).toHaveBeenCalledTimes(1);
    });

    expect(
      pageContentSaveHook.useFrontstagePageContentSave
    ).toHaveBeenLastCalledWith({
      workspaceId: 'workspace-1',
      pageId: 'page-1'
    });

    const [saveInput] = saveState.save.mock.calls[0] as [
      SaveFrontstagePageContentInput
    ];
    const [block] = getSavedBlocks(saveInput);

    expect(block).toMatchObject({
      id: 'frontstage-js-block-1',
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
      props: {},
      'x-layout': {
        order: 0,
        region: 'main'
      },
      runtime: {
        kind: 'iframe',
        entry: 'index.js',
        hint: 'iframe'
      }
    });
    expect(block).not.toHaveProperty('layout');
    expect(blockCodeApi.saveFrontstageBlockCode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      expect.objectContaining({
        codeRef: 'frontstage-js-block-1-code',
        code: createFrontstageBuiltInJsBlockTemplateCode({
          templateId: 'blank',
          blockId: 'frontstage-js-block-1',
          codeRef: 'frontstage-js-block-1-code',
          contributionCode: 'frontstage.js-ui-block'
        })
      }),
      'csrf-123'
    );
  });

  test.each([
    ['Data Table', 'data-table'],
    ['Create Form', 'create-form']
  ] as const)(
    'writes the selected %s JS template code when adding a block',
    async (templateName, templateId) => {
      authenticate(['frontstage.page.design']);
      mockFrontstageBlockCatalog([createCatalogEntry()]);
      mockPageContentSaveState();
      render(
        <AppProviders>
          <FrontStagePageHarness
            pageId="page-1"
            initialPageTree={[createBackendPage('page-1')]}
            pageContent={createPageContent()}
          />
        </AppProviders>
      );
      activateDesignMode();
      fireEvent.click(screen.getByRole('button', { name: '创建区块' }));
      fireEvent.click(await screen.findByRole('radio', { name: templateName }));
      fireEvent.click(screen.getByRole('button', { name: '选择' }));

      await waitFor(() =>
        expect(blockCodeApi.saveFrontstageBlockCode).toHaveBeenCalledTimes(1)
      );

      expect(blockCodeApi.saveFrontstageBlockCode).toHaveBeenCalledWith(
        'workspace-1',
        'page-1',
        expect.objectContaining({
          code: createFrontstageBuiltInJsBlockTemplateCode({
            templateId,
            blockId: 'frontstage-js-block-1',
            codeRef: 'frontstage-js-block-1-code',
            contributionCode: 'frontstage.js-ui-block'
          })
        }),
        'csrf-123'
      );
    }
  );

  test('disables Add Block while page content is saving', () => {
    authenticate(['frontstage.page.design']);
    mockPageContentSaveState({ saving: true, isPending: true });

    render(
      <AppProviders>
        <FrontStagePageHarness
          pageId="page-1"
          initialPageTree={[createBackendPage('page-1')]}
          pageContent={createPageContent()}
        />
      </AppProviders>
    );

    activateDesignMode();

    expect(screen.getByRole('button', { name: '创建区块' })).toBeDisabled();
    expect(screen.getByText('区块保存中')).toBeInTheDocument();
  });

  test('shows a clear Add Block save error in design mode', async () => {
    authenticate(['frontstage.page.design']);
    mockFrontstageBlockCatalog([createCatalogEntry()]);
    mockPageContentSaveState({
      save: vi.fn(() => Promise.reject(new Error('request failed')))
    });

    render(
      <AppProviders>
        <FrontStagePageHarness
          pageId="page-1"
          initialPageTree={[createBackendPage('page-1')]}
          pageContent={createPageContent()}
        />
      </AppProviders>
    );

    activateDesignMode();
    fireEvent.click(screen.getByRole('button', { name: '创建区块' }));
    fireEvent.click(await screen.findByRole('button', { name: '选择' }));

    expect(await screen.findByText('区块保存失败')).toBeInTheDocument();
    expect(screen.getByText('request failed')).toBeInTheDocument();
  });

  test('shows a clear Add Block code template save error', async () => {
    authenticate(['frontstage.page.design']);
    mockFrontstageBlockCatalog([createCatalogEntry()]);
    mockPageContentSaveState();
    blockCodeApi.saveFrontstageBlockCode.mockRejectedValueOnce(
      new Error('code save failed')
    );

    render(
      <AppProviders>
        <FrontStagePageHarness
          pageId="page-1"
          initialPageTree={[createBackendPage('page-1')]}
          pageContent={createPageContent()}
        />
      </AppProviders>
    );

    activateDesignMode();
    fireEvent.click(screen.getByRole('button', { name: '创建区块' }));
    fireEvent.click(await screen.findByRole('button', { name: '选择' }));

    expect(await screen.findByText('区块保存失败')).toBeInTheDocument();
    expect(screen.getByText('code save failed')).toBeInTheDocument();
    expect(screen.queryByText('1 个区块')).not.toBeInTheDocument();
  });

  test('disables Add Block when no page or no page content is available', () => {
    authenticate(['frontstage.page.design']);
    const view = render(
      <AppProviders>
        <FrontStagePageHarness />
      </AppProviders>
    );

    activateDesignMode();
    expect(screen.getByRole('button', { name: '创建区块' })).toBeDisabled();

    view.rerender(
      <AppProviders>
        <FrontStagePageHarness
          pageId="page-1"
          initialPageTree={[createBackendPage('page-1')]}
        />
      </AppProviders>
    );

    expect(screen.getByRole('button', { name: '创建区块' })).toBeDisabled();
  });

  test(
    'renders and selects the new block after Add Block save succeeds',
    async () => {
      authenticate(['frontstage.page.design']);
      mockFrontstageBlockCatalog([createCatalogEntry()]);
      mockPageContentSaveState();

      render(
        <AppProviders>
          <FrontStagePageHarness
            pageId="page-1"
            initialPageTree={[createBackendPage('page-1')]}
            pageContent={createPageContent()}
          />
        </AppProviders>
      );

      activateDesignMode();
      fireEvent.click(screen.getByRole('button', { name: '创建区块' }));
      fireEvent.click(await screen.findByRole('button', { name: '选择' }));

      await waitFor(() => {
        expect(
          screen.getByTestId('block-slot-frontstage-js-block-1')
        ).toBeInTheDocument();
      });

      expect(
        screen.getByRole('button', { name: '区块 frontstage-js-block-1' })
      ).toBeInTheDocument();
    },
    SLOW_FRONTSTAGE_TEST_TIMEOUT
  );
});
