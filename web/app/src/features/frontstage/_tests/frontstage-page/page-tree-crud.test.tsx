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
  content_presentation?: 'single' | 'tabs';
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

function createBackendPage(
  pageId: string,
  contentPresentation?: 'single' | 'tabs'
): TestFrontStageTreeNode {
  return {
    id: pageId,
    title: `页面 ${pageId}`,
    kind: 'page',
    ...(contentPresentation
      ? { content_presentation: contentPresentation }
      : {})
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

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function getPageTreeItem(title: string) {
  return screen.getByRole('button', {
    name: new RegExp(`${escapeRegExp(title)}\\s+页面节点`)
  });
}

function getGroupTreeItem(title: string) {
  return screen.getByTestId(`frontstage-tree-node-group-${title}`);
}

async function clickAndFlush(element: HTMLElement) {
  await act(async () => {
    element.click();
  });
}

async function clickLatestButtonAndFlush(label: string | RegExp) {
  const buttonName =
    typeof label === 'string'
      ? new RegExp(label.split('').map(escapeRegExp).join('\\s*'))
      : label;
  const buttons = await screen.findAllByRole('button', {
    name: buttonName
  });
  await clickAndFlush(buttons[buttons.length - 1]);
}

async function clickConfirmModalButtonAndFlush(label: string | RegExp) {
  const buttonName =
    typeof label === 'string'
      ? new RegExp(label.split('').map(escapeRegExp).join('\\s*'))
      : label;
  const dialogs = await screen.findAllByRole('dialog');
  const buttons = await within(dialogs[dialogs.length - 1]).findAllByRole(
    'button',
    { name: buttonName }
  );
  await clickAndFlush(buttons[buttons.length - 1]);
}

async function hoverAddMenuAndFlush() {
  fireEvent.mouseEnter(screen.getByRole('button', { name: '添加菜单' }));
}

function getNextDefaultNodeTitle(label: '新增分组' | '新增页面') {
  const titlePattern =
    label === '新增分组' ? /^分组 (\d+)$/ : /^页面 新建 (\d+)$/;
  const existingIndexes = screen
    .queryAllByText(titlePattern)
    .map((element) => element.textContent?.match(titlePattern)?.[1])
    .filter((index): index is string => Boolean(index))
    .map((index) => Number.parseInt(index, 10))
    .filter(Number.isFinite);
  const nextIndex =
    existingIndexes.length > 0 ? Math.max(...existingIndexes) + 1 : 1;

  return label === '新增分组' ? `分组 ${nextIndex}` : `页面 新建 ${nextIndex}`;
}

async function clickAddMenuItemAndFlush(label: '新增分组' | '新增页面') {
  await hoverAddMenuAndFlush();
  await clickAndFlush(await screen.findByRole('menuitem', { name: label }));
  fireEvent.change(await screen.findByLabelText('名称'), {
    target: { value: getNextDefaultNodeTitle(label) }
  });
  await clickLatestButtonAndFlush('确定');
}

async function clickAddMenuItem(label: '新增分组' | '新增页面') {
  await clickAddMenuItemAndFlush(label);
}

async function clickAddPageInGroupAndFlush(groupContainer: HTMLElement) {
  await clickPageTreeOperationMenuItemAndFlush(groupContainer, '在里面插入');
  fireEvent.change(await screen.findByLabelText('名称'), {
    target: { value: getNextDefaultNodeTitle('新增页面') }
  });
  await clickLatestButtonAndFlush('确定');
}

async function openPageTreeOperationMenuAndFlush(nodeContainer: HTMLElement) {
  const menuButtons = within(nodeContainer).getAllByRole('button', {
    name: '页面操作菜单'
  });
  const menuButton = menuButtons[0];
  if (!menuButton) {
    throw new Error('expected page tree operation menu button');
  }
  await clickAndFlush(menuButton);
}

async function clickPageTreeOperationMenuItemAndFlush(
  nodeContainer: HTMLElement,
  label: string | RegExp
) {
  await openPageTreeOperationMenuAndFlush(nodeContainer);
  await clickAndFlush(await findLatestVisibleText(label));
}

async function findLatestVisibleText(label: string | RegExp) {
  let elements: HTMLElement[] = [];
  await waitFor(() => {
    const activeDropdowns = document.body.querySelectorAll<HTMLElement>(
      '.ant-dropdown:not(.ant-dropdown-hidden), .ant-dropdown-menu-submenu-popup:not(.ant-dropdown-hidden)'
    );
    elements = Array.from(activeDropdowns).flatMap((dropdown) =>
      within(dropdown).queryAllByText(label)
    );
    expect(elements.length).toBeGreaterThan(0);
  });
  const element = elements[elements.length - 1];
  if (!element) {
    throw new Error(`expected visible text for ${String(label)}`);
  }
  return element;
}

function activateDesignMode() {
  act(() => {
    useFrontstageDesignModeStore.getState().setDesignMode(true);
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
describe('FrontStagePage - page tree CRUD', () => {
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

  test(
    'supports adding and deleting page tree nodes in design mode',
    async () => {
      authenticate(['frontstage.page.design']);
      renderPage();

      activateDesignMode();
      await clickAddMenuItemAndFlush('新增分组');
      await clickAddMenuItemAndFlush('新增页面');

      expect(screen.getByText('分组 1')).toBeInTheDocument();
      expect(screen.getAllByText('页面 新建 1').length).toBeGreaterThan(0);

      const pageListItem = getPageTreeItem('页面 新建 1');
      await clickPageTreeOperationMenuItemAndFlush(pageListItem, '删除');
      await clickConfirmModalButtonAndFlush('删除');

      expect(screen.queryByText('页面 新建 1')).not.toBeInTheDocument();
    },
    SLOW_FRONTSTAGE_TEST_TIMEOUT
  );

  test('creates page through page tree mutation callback without local fake node', async () => {
    authenticate(['frontstage.page.design']);
    let resolveCreatePage: (() => void) | undefined;
    const createPagePromise = new Promise<void>((resolve) => {
      resolveCreatePage = resolve;
    });
    const onCreatePageNode = vi.fn(() => createPagePromise);

    render(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          initialPageTree={[]}
          onCreatePageNode={onCreatePageNode}
        />
      </AppProviders>
    );

    activateDesignMode();
    await clickAddMenuItem('新增页面');

    await waitFor(() => {
      expect(onCreatePageNode).toHaveBeenCalledWith({
        title: '页面 新建 1',
        icon: '',
        tooltip: '',
        parentId: null,
        rank: '001000'
      });
    });
    expect(screen.getByText('保存中')).toBeInTheDocument();
    expect(screen.queryByText('页面 新建 1')).not.toBeInTheDocument();

    await act(async () => {
      resolveCreatePage?.();
      await createPagePromise;
    });

    expect(screen.queryByText('保存中')).not.toBeInTheDocument();
    expect(screen.queryByText('页面 新建 1')).not.toBeInTheDocument();
  });

  test(
    'renames and deletes through page tree mutation callbacks',
    async () => {
      authenticate(['frontstage.page.design']);
      const onRenamePageNode = vi.fn().mockResolvedValue(undefined);
      const onDeletePageNode = vi.fn().mockResolvedValue(undefined);

      render(
        <AppProviders>
          <FrontStagePage
            workspaceId="workspace-1"
            pageId="page-1"
            initialPageTree={[createBackendPage('page-1')]}
            onRenamePageNode={onRenamePageNode}
            onDeletePageNode={onDeletePageNode}
          />
        </AppProviders>
      );

      activateDesignMode();

      const pageItem = getPageTreeItem('页面 page-1');

      await clickPageTreeOperationMenuItemAndFlush(pageItem, '编辑');
      fireEvent.change(screen.getByLabelText('名称'), {
        target: { value: '页面-已重命名' }
      });
      await clickLatestButtonAndFlush('确定');
      await waitFor(() => {
        expect(onRenamePageNode).toHaveBeenCalledWith('page-1', {
          icon: '',
          tooltip: '',
          title: '页面-已重命名'
        });
      });
      await waitFor(() => {
        expect(screen.queryByText('保存中')).not.toBeInTheDocument();
      });
      expect(screen.getAllByText('页面-已重命名').length).toBeGreaterThan(0);

      await clickPageTreeOperationMenuItemAndFlush(pageItem, '删除');
      await clickConfirmModalButtonAndFlush('删除');
      await waitFor(() => {
        expect(onDeletePageNode).toHaveBeenCalledWith('page-1');
      });
    },
    SLOW_FRONTSTAGE_TEST_TIMEOUT
  );

  test(
    'AC-001 persists the configured presentation and keeps Page settings open when the backend rejects the transition',
    async () => {
      authenticate(['frontstage.page.design']);
      const onRenamePageNode = vi
        .fn()
        .mockResolvedValueOnce(undefined)
        .mockRejectedValueOnce(
          new Error(
            'frontstage_page_single_presentation_requires_default_tab_only'
          )
        );

      render(
        <AppProviders>
          <FrontStagePage
            workspaceId="workspace-1"
            pageId="page-1"
            initialPageTree={[createBackendPage('page-1', 'single')]}
            onRenamePageNode={onRenamePageNode}
          />
        </AppProviders>
      );

      activateDesignMode();
      const pageItem = getPageTreeItem('页面 page-1');

      await clickPageTreeOperationMenuItemAndFlush(pageItem, '编辑');
      fireEvent.mouseDown(
        screen.getByRole('combobox', { name: '内容呈现方式' })
      );
      await clickAndFlush(await screen.findByText('标签页'));
      await clickLatestButtonAndFlush('确定');

      await waitFor(() => {
        expect(onRenamePageNode).toHaveBeenNthCalledWith(1, 'page-1', {
          title: '页面 page-1',
          icon: '',
          tooltip: '',
          contentPresentation: 'tabs'
        });
      });
      await waitFor(() => {
        expect(
          screen.queryByRole('dialog', { name: '配置页面' })
        ).not.toBeInTheDocument();
      });

      await clickPageTreeOperationMenuItemAndFlush(pageItem, '编辑');
      fireEvent.mouseDown(
        screen.getByRole('combobox', { name: '内容呈现方式' })
      );
      await clickAndFlush(await screen.findByText('单页（不显示标签页）'));
      await clickLatestButtonAndFlush('确定');

      await waitFor(() => {
        expect(onRenamePageNode).toHaveBeenNthCalledWith(2, 'page-1', {
          title: '页面 page-1',
          icon: '',
          tooltip: '',
          contentPresentation: 'single'
        });
      });
      expect(
        await screen.findByRole('dialog', { name: '配置页面' })
      ).toBeInTheDocument();
      expect(screen.getByText('操作失败')).toBeInTheDocument();
    },
    SLOW_FRONTSTAGE_TEST_TIMEOUT
  );

  test('opens page tree operation menu on hover', async () => {
    authenticate(['frontstage.page.design']);
    const onUpdatePageNodeMetadata = vi.fn().mockResolvedValue(undefined);

    render(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          pageId="page-1"
          initialPageTree={[createBackendPage('page-1')]}
          onUpdatePageNodeMetadata={onUpdatePageNodeMetadata}
        />
      </AppProviders>
    );

    activateDesignMode();

    const pageItem = getPageTreeItem('页面 page-1');
    expect(screen.queryByText('编辑描述')).not.toBeInTheDocument();

    await clickAndFlush(
      within(pageItem).getByRole('button', { name: '页面操作菜单' })
    );

    expect(await screen.findByText('编辑描述')).toBeInTheDocument();
    expect(await screen.findByText('移动到')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('switch'));
    await waitFor(() => {
      expect(onUpdatePageNodeMetadata).toHaveBeenCalledWith('page-1', {
        isHidden: true
      });
    });

    fireEvent.click(await screen.findByText('编辑描述'));
    fireEvent.change(screen.getByLabelText('描述'), {
      target: { value: '展示在页面树' }
    });
    await clickLatestButtonAndFlush('确定');
    await waitFor(() => {
      expect(onUpdatePageNodeMetadata).toHaveBeenCalledWith('page-1', {
        tooltip: '展示在页面树'
      });
    });
  });

  test(
    'does not delete node when delete confirmation is canceled',
    async () => {
      authenticate(['frontstage.page.design']);
      renderPage();

      activateDesignMode();
      await clickAddMenuItemAndFlush('新增分组');
      await clickAddMenuItemAndFlush('新增页面');

      const pageItem = getPageTreeItem('页面 新建 1');

      await clickPageTreeOperationMenuItemAndFlush(pageItem, '删除');
      await clickConfirmModalButtonAndFlush('取消');

      expect(screen.getAllByText('页面 新建 1').length).toBeGreaterThan(0);
      expect(screen.getByText('分组 1')).toBeInTheDocument();
    },
    SLOW_FRONTSTAGE_TEST_TIMEOUT
  );

  test('generates unique page id when existing page ids conflict', async () => {
    authenticate(['frontstage.page.design']);

    renderPageWithInitialTree([
      {
        id: 'page-1',
        title: '页面 page-1',
        kind: 'page'
      }
    ]);

    activateDesignMode();
    await clickAddMenuItemAndFlush('新增页面');

    expect(screen.getAllByText('页面 新建 1').length).toBeGreaterThan(0);
  });

  test('adds page under group in design mode', async () => {
    authenticate(['frontstage.page.design']);
    renderPage();

    activateDesignMode();
    await clickAddMenuItemAndFlush('新增分组');

    const groupContainer = getGroupTreeItem('分组 1');

    await clickAddPageInGroupAndFlush(groupContainer);

    expect(screen.getAllByText('页面 新建 1').length).toBeGreaterThan(0);
  });

  test('generates unique group id when existing group ids conflict', async () => {
    authenticate(['frontstage.page.design']);

    renderPageWithInitialTree([
      {
        id: 'group-1',
        title: '分组 1',
        kind: 'group',
        children: []
      }
    ]);

    activateDesignMode();
    await clickAddMenuItemAndFlush('新增分组');

    expect(screen.getByText('分组 2')).toBeInTheDocument();
  });

  test('only allows adding a page into top-level groups', async () => {
    authenticate(['frontstage.page.design']);

    renderPageWithInitialTree([
      {
        id: 'group-root',
        title: '分组 一级',
        kind: 'group',
        children: [
          {
            id: 'group-nested',
            title: '分组 二级',
            kind: 'group',
            children: [
              {
                id: 'page-inside-nested',
                title: '页面 嵌套',
                kind: 'page'
              }
            ]
          }
        ]
      }
    ]);

    activateDesignMode();

    const rootGroupItem = getGroupTreeItem('分组 一级');

    await openPageTreeOperationMenuAndFlush(rootGroupItem);
    expect(await findLatestVisibleText('在里面插入')).toBeInTheDocument();
    expect(screen.queryByText('分组 二级')).not.toBeInTheDocument();
    expect(screen.getAllByText('页面 嵌套').length).toBeGreaterThan(0);
  });

  test(
    'deletes group and cascades child pages',
    async () => {
      authenticate(['frontstage.page.design']);
      renderPage();

      activateDesignMode();
      await clickAddMenuItemAndFlush('新增分组');

      const groupItem = getGroupTreeItem('分组 1');

      await clickAddPageInGroupAndFlush(groupItem);
      expect(screen.getAllByText('页面 新建 1').length).toBeGreaterThan(0);

      await clickPageTreeOperationMenuItemAndFlush(groupItem, '删除');
      await clickConfirmModalButtonAndFlush('删除');

      expect(screen.queryByText('分组 1')).not.toBeInTheDocument();
      expect(screen.queryByText('页面 新建 1')).not.toBeInTheDocument();
    },
    SLOW_FRONTSTAGE_TEST_TIMEOUT
  );

  test(
    'renames node title in design mode',
    async () => {
      authenticate(['frontstage.page.design']);
      renderPage();

      activateDesignMode();
      await clickAddMenuItem('新增页面');

      const pageItem = getPageTreeItem('页面 新建 1');

      await clickPageTreeOperationMenuItemAndFlush(pageItem, '编辑');
      fireEvent.change(screen.getByLabelText('名称'), {
        target: { value: '页面-已重命名' }
      });
      await clickLatestButtonAndFlush('确定');
      expect(screen.getAllByText('页面-已重命名').length).toBeGreaterThan(0);
    },
    SLOW_FRONTSTAGE_TEST_TIMEOUT
  );

  test(
    'requires a non-empty node title when renaming',
    async () => {
      authenticate(['frontstage.page.design']);
      renderPage();

      activateDesignMode();
      await clickAddMenuItem('新增页面');

      const pageItem = getPageTreeItem('页面 新建 1');

      await clickPageTreeOperationMenuItemAndFlush(pageItem, '编辑');
      fireEvent.change(screen.getByLabelText('名称'), {
        target: { value: '' }
      });
      await clickLatestButtonAndFlush('确定');
      expect(await screen.findByText('请输入名称')).toBeInTheDocument();
      expect(screen.getAllByText('页面 新建 1').length).toBeGreaterThan(0);
    },
    SLOW_FRONTSTAGE_TEST_TIMEOUT
  );

  test('renaming a node prefills current title in the form', async () => {
    authenticate(['frontstage.page.design']);
    renderPage();

    activateDesignMode();
    await clickAddMenuItem('新增页面');

    const pageItem = getPageTreeItem('页面 新建 1');

    await clickPageTreeOperationMenuItemAndFlush(pageItem, '编辑');
    expect(screen.getByLabelText('名称')).toHaveValue('页面 新建 1');
  });
});
