import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
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
  SaveFrontstageTabDocumentInput
} from '../../api/page-content';
import {
  createFrontstagePageContentFixture,
  type FrontstagePageContentFixtureOverrides
} from '../frontstage-page-content-fixtures';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import type { FrontstagePageTab } from '../../api/page-tabs';
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
const dataCapabilitiesHook = vi.hoisted(() => ({
  useFrontstageDataCapabilities: vi.fn()
}));
const runtimeSessionsHook = vi.hoisted(() => ({
  useFrontstagePageCanvasNativePreparations: vi.fn(() => ({
    preparations: [],
    retryBlock: vi.fn()
  }))
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
const pageTabsApi = vi.hoisted(() => ({
  createFrontstagePageTab: vi.fn(),
  deleteFrontstagePageTab: vi.fn(),
  fetchFrontstagePageTabs: vi.fn(),
  moveFrontstagePageTab: vi.fn(),
  renameFrontstagePageTab: vi.fn(),
  frontstagePageTabsQueryKey: vi.fn((workspaceId: string, pageId: string) => [
    'frontstage',
    workspaceId,
    'pages',
    pageId,
    'tabs'
  ])
}));
const trialPanel = vi.hoisted(() => ({
  render: vi.fn()
}));

vi.mock(
  '../../hooks/use-frontstage-page-content-save',
  () => pageContentSaveHook
);
vi.mock('../../hooks/use-frontstage-block-catalog', () => blockCatalogHook);
vi.mock('../../hooks/use-frontstage-block-code', () => blockCodeHook);
vi.mock(
  '../../hooks/use-frontstage-data-capabilities',
  () => dataCapabilitiesHook
);
vi.mock(
  '../../hooks/use-frontstage-page-canvas-native-preparations',
  () => runtimeSessionsHook
);
vi.mock('../../api/block-code', () => blockCodeApi);
vi.mock('../../api/page-tabs', () => pageTabsApi);
vi.mock('../../components/JsBlockTrialPanel', () => ({
  JsBlockTrialPanel: (props: unknown) => {
    trialPanel.render(props);
    return <div data-testid="captured-js-block-trial-panel" />;
  }
}));

const SLOW_FRONTSTAGE_TEST_TIMEOUT = 20_000;
const PLUGIN_CODE_TEMPLATE = `
export default function PluginBlock() {
  return <p>Plugin template ready</p>;
}
`.trim();

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
  tabId,
  onNavigatePage,
  onNavigateTab,
  initialPageTree,
  pageContent,
  isPageContentLoading,
  hasPageContentLoadError
}: {
  workspaceId?: string;
  pageId?: string;
  tabId?: string;
  onNavigatePage?: (pageId?: string) => void;
  onNavigateTab?: (tab: FrontstagePageTab) => void;
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
      tabId={tabId}
      onNavigatePage={onNavigatePage}
      onNavigateTab={onNavigateTab}
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
    runtimeKind: 'native_react',
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
    codeCapabilities: {
      template: {
        source: PLUGIN_CODE_TEMPLATE,
        version: '2.4.0',
        language: 'tsx'
      },
      allowedImports: [],
      monacoExtraLibs: []
    },
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

function mockFrontstageDataCapabilities() {
  dataCapabilitiesHook.useFrontstageDataCapabilities.mockReturnValue({
    data: { queries: [], actions: [], models: [] },
    loading: false,
    error: null
  });
}

function getSavedBlocks(input: SaveFrontstageTabDocumentInput) {
  const payload = input.payload;
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
    mockFrontstageDataCapabilities();
    pageTabsApi.fetchFrontstagePageTabs.mockResolvedValue([
      {
        id: 'tab-1',
        page_id: 'page-1',
        title: '概览',
        rank: '001000',
        is_default: true,
        document_root_uid: 'frontstage.tab.tab-1.root'
      }
    ]);
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
      screen.queryByRole('button', { name: '代码区块试运行' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('页面树已同步')).not.toBeInTheDocument();

    activateDesignMode();
    expect(
      screen.getByRole('button', { name: '创建区块' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '代码区块试运行' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('页面树已同步')).not.toBeInTheDocument();
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
      screen.queryByRole('button', { name: '代码区块试运行' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('页面树已同步')).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '添加菜单' })
    ).not.toBeInTheDocument();
  });

  test('AC-001 renders only Ant Empty when no page is bound', () => {
    authenticate(['frontstage.page.design']);
    renderPage();

    activateDesignMode();

    const workspace = screen.getByTestId('frontstage-page-workspace');
    expect(workspace.querySelector('.ant-empty')).toBeInTheDocument();
    expect(
      workspace.querySelector('.ant-empty-description')
    ).not.toBeInTheDocument();
    expect(
      workspace.querySelector('.frontstage-page-workspace__header')
    ).not.toBeInTheDocument();
    expect(workspace.querySelector('.ant-divider')).not.toBeInTheDocument();
    expect(
      within(workspace).queryByRole('button', { name: '创建区块' })
    ).not.toBeInTheDocument();
    expect(
      within(workspace).queryByText('未选择页面内容')
    ).not.toBeInTheDocument();
    expect(
      within(workspace).queryByText('选择页面后将显示页面预览。')
    ).not.toBeInTheDocument();
  });

  test('AC-002 places Add menu as the first tree row when the tree is empty', () => {
    authenticate(['frontstage.page.design']);
    renderPage();

    activateDesignMode();

    const sidebar = document.querySelector('.frontstage-page-tree-sidebar');
    expect(sidebar).toBeInTheDocument();
    expect(sidebar?.querySelector('.ant-empty')).not.toBeInTheDocument();

    const tree = sidebar?.querySelector('.frontstage-page-tree-sidebar__tree');
    expect(tree).toBeInTheDocument();
    expect(tree?.children).toHaveLength(1);
    expect(
      within(tree as HTMLElement).getByRole('button', { name: '添加菜单' })
    ).toHaveClass(
      'frontstage-add-action-button',
      'frontstage-add-action-button--full'
    );
  });

  test('AC-003 places Add menu after existing top-level nodes', () => {
    authenticate(['frontstage.page.design']);
    renderPage('page-1');

    activateDesignMode();

    const tree = document.querySelector('.frontstage-page-tree-sidebar__tree');
    expect(tree).toBeInTheDocument();
    expect(tree?.children).toHaveLength(2);
    expect(
      within(tree?.lastElementChild as HTMLElement).getByRole('button', {
        name: '添加菜单'
      })
    ).toBeInTheDocument();
  });

  test('AC-001/009 opens page settings and persists the selected layout mode', async () => {
    authenticate(['frontstage.page.design']);
    const saveState = mockPageContentSaveState();
    render(
      <AppProviders>
        <FrontStagePageHarness
          pageId="page-1"
          tabId="tab-1"
          initialPageTree={[createBackendPage('page-1')]}
          pageContent={createPageContent()}
        />
      </AppProviders>
    );

    activateDesignMode();

    const workspace = screen.getByTestId('frontstage-page-workspace');
    expect(workspace).toHaveAttribute('data-design-selected', 'true');
    const configurePage = within(workspace).getByRole('button', {
      name: '配置页面'
    });
    expect(configurePage).toBeInTheDocument();
    expect(
      screen.getByRole('heading', { name: '页面 page-1' }).parentElement
    ).toHaveClass('frontstage-page-workspace__header');

    fireEvent.click(configurePage);

    const pageMenu = await screen.findByRole('menu');
    expect(within(pageMenu).getAllByRole('menuitem')).toHaveLength(3);
    expect(within(pageMenu).getByText('编辑')).toBeInTheDocument();
    const layoutMode = within(pageMenu).getByRole('combobox', {
      name: '布局方式'
    });
    expect(within(pageMenu).getByText('自动布局')).toBeInTheDocument();
    expect(
      within(pageMenu).getByRole('switch', { name: '开启 Tabs' })
    ).not.toBeChecked();

    fireEvent.mouseDown(layoutMode);
    fireEvent.click(await screen.findByText('自由网格'));
    await waitFor(() => expect(saveState.save).toHaveBeenCalledTimes(1));
    const [layoutModeSaveInput] = saveState.save.mock.calls[0] as [
      SaveFrontstageTabDocumentInput
    ];
    expect(layoutModeSaveInput.payload).toMatchObject({
      'x-layout-mode': 'free'
    });

    fireEvent.click(configurePage);
    fireEvent.click(within(pageMenu).getByText('编辑'));
    expect(
      await screen.findByRole('dialog', { name: '配置页面' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('combobox', { name: '内容呈现方式' })
    ).not.toBeInTheDocument();
  });

  test('#1300 keeps the canvas and Add Block action inside the active tab container', async () => {
    authenticate(['frontstage.page.design']);
    render(
      <AppProviders>
        <FrontStagePageHarness
          pageId="page-1"
          tabId="tab-1"
          onNavigateTab={vi.fn()}
          initialPageTree={[createBackendPage('page-1')]}
          pageContent={createPageContent({
            page: { contentPresentation: 'tabs' }
          })}
        />
      </AppProviders>
    );

    activateDesignMode();

    const tabContent = await screen.findByTestId('frontstage-tab-content');
    expect(tabContent).toHaveAttribute('data-design-selected', 'true');
    expect(
      within(tabContent).getByRole('button', { name: '创建区块' })
    ).toBeInTheDocument();
    expect(
      within(tabContent).getByTestId('page-canvas-design-empty-state')
    ).toBeEmptyDOMElement();
  });

  test('opens configuration and code in the same resizable JSX Studio', async () => {
    authenticate(['frontstage.page.design']);
    mockFrontstageBlockCatalog([createCatalogEntry()]);
    const blockPayload = {
      id: 'orders-block',
      renderer_version: 'v1',
      codeRef: 'orders-code',
      catalog: {
        providerCode: '1flowbase',
        installationId: 'builtin-installation'
      },
      contribution: {
        pluginId: 'builtin-frontstage',
        pluginVersion: '1.0.0',
        code: 'frontstage.js-ui-block'
      },
      props: { title: 'Orders' },
      'x-layout': { order: 0, region: 'main' },
      runtime: { kind: 'native_react', entry: 'index.js', hint: 'native_react' }
    };

    render(
      <AppProviders>
        <FrontStagePageHarness
          pageId="page-1"
          tabId="tab-1"
          onNavigateTab={vi.fn()}
          initialPageTree={[createBackendPage('page-1')]}
          pageContent={createPageContent({
            schema: {
              rootUid: 'root-1',
              payload: { blocks: [blockPayload] }
            },
            root: {
              uid: 'root-1',
              payload: { blocks: [blockPayload] }
            }
          })}
        />
      </AppProviders>
    );

    activateDesignMode();
    const blockSlot = await screen.findByTestId('block-slot-orders-block');
    fireEvent.mouseEnter(blockSlot);
    fireEvent.click(
      within(blockSlot).getByRole('button', { name: '编辑区块' })
    );

    const studio = await screen.findByRole('dialog', { name: 'TSX 编辑器' });
    fireEvent.click(within(studio).getByRole('button', { name: '区块设置' }));
    expect(
      within(studio).getAllByText('区块设置').length
    ).toBeGreaterThanOrEqual(1);
    expect(
      screen.queryByRole('dialog', { name: '区块配置' })
    ).not.toBeInTheDocument();
  });

  test('D2-P2F wires an inserted Surface source and its catalog lock into the production trial path', async () => {
    authenticate(['frontstage.page.design']);
    mockFrontstageBlockCatalog([
      createCatalogEntry({
        codeModules: [
          {
            source: '@1flowbase/native-components',
            version: '1.0.0',
            browser_asset: {
              sha256:
                'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
            },
            exports: ['Surface'],
            type_declarations:
              "declare module '@1flowbase/native-components' { export const Surface: unknown; }"
          }
        ]
      })
    ]);
    blockCodeHook.useFrontstageBlockCode.mockReturnValue({
      code: 'import { Surface } from \'@1flowbase/native-components\';\nexport default () => <Surface className="card" />;',
      draft:
        'import { Surface } from \'@1flowbase/native-components\';\nexport default () => <Surface className="card" />;',
      dirty: false,
      loading: false,
      saving: false,
      error: null,
      setDraft: vi.fn(),
      reset: vi.fn(),
      save: vi.fn()
    });
    const blockPayload = {
      id: 'surface-block',
      renderer_version: 'v1',
      codeRef: 'surface-code',
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
      'x-layout': { order: 0, region: 'main' },
      runtime: { kind: 'native_react', entry: 'index.js', hint: 'native_react' }
    };

    render(
      <AppProviders>
        <FrontStagePageHarness
          pageId="page-1"
          tabId="tab-1"
          onNavigateTab={vi.fn()}
          initialPageTree={[createBackendPage('page-1')]}
          pageContent={createPageContent({
            schema: {
              rootUid: 'root-1',
              payload: { blocks: [blockPayload] }
            },
            root: {
              uid: 'root-1',
              payload: { blocks: [blockPayload] }
            }
          })}
        />
      </AppProviders>
    );

    activateDesignMode();
    const blockSlot = await screen.findByTestId('block-slot-surface-block');
    fireEvent.mouseEnter(blockSlot);
    fireEvent.click(
      within(blockSlot).getByRole('button', { name: '编辑区块' })
    );
    const studio = await screen.findByRole('dialog', { name: 'TSX 编辑器' });
    fireEvent.click(within(studio).getByRole('button', { name: '运行' }));

    await waitFor(() => expect(trialPanel.render).toHaveBeenCalled());
    expect(trialPanel.render).toHaveBeenLastCalledWith(
      expect.objectContaining({
        code: expect.stringContaining('<Surface className="card" />'),
        nativeDependencyLock: [
          {
            module_source: '@1flowbase/native-components',
            module_version: '1.0.0',
            browser_asset: {
              sha256:
                'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
              url: '/api/console/frontstage/workspace-1/component-module-assets/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
            },
            exports: ['Surface']
          }
        ],
        nativeDependencyLockError: null
      })
    );
  });

  test('shows real page tree operation states without local draft wording', () => {
    authenticate(['frontstage.page.design']);
    const view = render(
      <AppProviders>
        <FrontStagePage workspaceId="workspace-1" initialPageTree={[]} />
      </AppProviders>
    );

    activateDesignMode();
    expect(screen.queryByText('页面树已同步')).not.toBeInTheDocument();
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
    expect(screen.getByText('failed')).toBeInTheDocument();
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

  test('AC-011 creates the official default JSX block directly with auditable catalog metadata', async () => {
    authenticate(['frontstage.page.design']);
    mockFrontstageBlockCatalog([
      createCatalogEntry({
        id: 'third-party:other-block',
        providerCode: 'third-party',
        contributionCode: 'other-block'
      }),
      createCatalogEntry()
    ]);
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

    await waitFor(() => {
      expect(saveState.save).toHaveBeenCalledTimes(1);
    });
    expect(
      screen.queryByRole('button', { name: '选择' })
    ).not.toBeInTheDocument();

    expect(
      pageContentSaveHook.useFrontstagePageContentSave
    ).toHaveBeenLastCalledWith({
      workspaceId: 'workspace-1',
      pageId: 'page-1'
    });

    const [saveInput] = saveState.save.mock.calls[0] as [
      SaveFrontstageTabDocumentInput
    ];
    const [block] = getSavedBlocks(saveInput);

    expect(block).toMatchObject({
      renderer_version: 'v1',
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
        kind: 'native_react',
        entry: 'index.js',
        hint: 'native_react',
        code_template_version: '2.4.0',
        code_template_language: 'tsx'
      }
    });
    expect(block.id).toMatch(/^frontstage-js-block-[0-9a-f-]{36}$/);
    expect(block.codeRef).toBe(`${String(block.id)}-code`);
    expect(block).not.toHaveProperty('layout');
    expect(blockCodeApi.saveFrontstageBlockCode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      {
        codeRef: block.codeRef,
        code: PLUGIN_CODE_TEMPLATE
      },
      'csrf-123'
    );
  });

  test('AC-011 rejects a catalog entry without a code template before saving', async () => {
    authenticate(['frontstage.page.design']);
    mockFrontstageBlockCatalog([
      createCatalogEntry({ codeCapabilities: undefined })
    ]);
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

    expect(
      await screen.findByText('Catalog entry 缺少代码模板')
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '选择' })
    ).not.toBeInTheDocument();
    expect(saveState.save).not.toHaveBeenCalled();
    expect(blockCodeApi.saveFrontstageBlockCode).not.toHaveBeenCalled();
  });

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

    expect(await screen.findByText('区块保存失败')).toBeInTheDocument();
    expect(screen.getByText('request failed')).toBeInTheDocument();
  });

  test('shows a clear Add Block code template save error', async () => {
    authenticate(['frontstage.page.design']);
    mockFrontstageBlockCatalog([createCatalogEntry()]);
    const saveState = mockPageContentSaveState();
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

    expect(await screen.findByText('区块保存失败')).toBeInTheDocument();
    expect(screen.getByText('code save failed')).toBeInTheDocument();
    await waitFor(() => {
      expect(saveState.save).toHaveBeenCalledTimes(2);
    });
    const [rollbackInput] = saveState.save.mock.calls[1] as [
      SaveFrontstageTabDocumentInput
    ];
    expect(getSavedBlocks(rollbackInput)).toEqual([]);
    expect(screen.queryByText('1 个区块')).not.toBeInTheDocument();
  });

  test('hides Add Block without a page and disables it without page content', () => {
    authenticate(['frontstage.page.design']);
    const view = render(
      <AppProviders>
        <FrontStagePageHarness />
      </AppProviders>
    );

    activateDesignMode();
    expect(
      screen.queryByRole('button', { name: '创建区块' })
    ).not.toBeInTheDocument();

    view.unmount();
    render(
      <AppProviders>
        <FrontStagePageHarness
          pageId="page-1"
          initialPageTree={[createBackendPage('page-1')]}
        />
      </AppProviders>
    );

    activateDesignMode();
    expect(screen.getByRole('button', { name: '创建区块' })).toBeDisabled();
  });

  test(
    'renders and selects the new block after Add Block save succeeds',
    async () => {
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

      await waitFor(() => {
        expect(saveState.save).toHaveBeenCalledTimes(1);
      });
      const [saveInput] = saveState.save.mock.calls[0] as [
        SaveFrontstageTabDocumentInput
      ];
      const [createdBlock] = getSavedBlocks(saveInput);
      const createdBlockId = String(createdBlock?.id);

      await waitFor(() => {
        expect(
          screen.getByTestId(`block-slot-${createdBlockId}`)
        ).toBeInTheDocument();
      });

      expect(
        screen.getByRole('button', { name: `区块 ${createdBlockId}` })
      ).toBeInTheDocument();
      expect(
        screen.queryByRole('dialog', { name: '区块代码' })
      ).not.toBeInTheDocument();
    },
    SLOW_FRONTSTAGE_TEST_TIMEOUT
  );
});
