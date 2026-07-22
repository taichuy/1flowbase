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
describe('FrontStagePage - routing fallback', () => {
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
    'falls back to first available page when selected page is deleted by parent group',
    async () => {
      authenticate(['frontstage.page.design']);
      const onNavigatePage = vi.fn();

      renderPageWithInitialTree(
        [
          {
            id: 'group-1',
            title: '分组 1',
            kind: 'group',
            children: [
              {
                id: 'page-in-group',
                title: '页面 分组内',
                kind: 'page'
              }
            ]
          },
          {
            id: 'page-root',
            title: '页面 根',
            kind: 'page'
          }
        ],
        'page-in-group',
        onNavigatePage
      );

      activateDesignMode();

      const groupItemForDelete = getGroupTreeItem('分组 1');

      await clickPageTreeOperationMenuItemAndFlush(groupItemForDelete, '删除');
      await clickConfirmModalButtonAndFlush('删除');

      await waitFor(() => {
        expect(screen.queryByText('分组 1')).not.toBeInTheDocument();
        expect(onNavigatePage).toHaveBeenLastCalledWith('page-root');
      });
    },
    SLOW_FRONTSTAGE_TEST_TIMEOUT
  );

  test(
    'falls back to workspace-level route when selected nested group is deleted and no pages remain',
    async () => {
      authenticate(['frontstage.page.design']);
      const onNavigatePage = vi.fn();

      renderPageWithInitialTree(
        [
          {
            id: 'group-root',
            title: '分组 一级',
            kind: 'group',
            children: [
              {
                id: 'group-inner',
                title: '分组 二级',
                kind: 'group',
                children: [
                  {
                    id: 'page-inside',
                    title: '页面 嵌套',
                    kind: 'page'
                  }
                ]
              }
            ]
          }
        ],
        'page-inside',
        onNavigatePage
      );

      activateDesignMode();

      const rootGroup = getGroupTreeItem('分组 一级');

      await clickPageTreeOperationMenuItemAndFlush(rootGroup, '删除');
      await clickConfirmModalButtonAndFlush('删除');

      await waitFor(() => {
        expect(screen.queryAllByText('页面 嵌套')).toHaveLength(0);
        expect(onNavigatePage).toHaveBeenCalledWith(undefined);
      });
    },
    SLOW_FRONTSTAGE_TEST_TIMEOUT
  );

  test(
    'navigates to created page when entering pageId-less frontstage route',
    async () => {
      authenticate(['frontstage.page.design']);
      const onNavigatePage = vi.fn();

      renderPage(undefined, onNavigatePage);

      activateDesignMode();
      await clickAddMenuItem('新增页面');

      expect(onNavigatePage).toHaveBeenLastCalledWith(
        expect.stringMatching(
          /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
        )
      );
    },
    SLOW_FRONTSTAGE_TEST_TIMEOUT
  );

  test(
    'falls back to first page when deleting selected page',
    async () => {
      authenticate(['frontstage.page.design']);
      const onNavigatePage = vi.fn();

      renderPage(undefined, onNavigatePage);

      activateDesignMode();
      await clickAddMenuItemAndFlush('新增页面');
      await clickAddMenuItemAndFlush('新增页面');
      const firstPageId = onNavigatePage.mock.calls[0]?.[0] as
        | string
        | undefined;

      const secondPageItem = getPageTreeItem('页面 新建 2');

      await clickPageTreeOperationMenuItemAndFlush(secondPageItem, '删除');
      await clickConfirmModalButtonAndFlush('删除');
      await waitFor(() => {
        expect(onNavigatePage).toHaveBeenCalledWith(firstPageId);
      });
    },
    SLOW_FRONTSTAGE_TEST_TIMEOUT
  );

  test(
    'navigates to workspace-level frontstage route when all pages are deleted',
    async () => {
      authenticate(['frontstage.page.design']);
      const onNavigatePage = vi.fn();

      renderPage('page-1', onNavigatePage);

      activateDesignMode();
      const pageItem = getPageTreeItem('页面 page-1');
      await clickPageTreeOperationMenuItemAndFlush(pageItem, '删除');
      await clickConfirmModalButtonAndFlush('删除');

      await waitFor(() => {
        expect(onNavigatePage).toHaveBeenCalledWith(undefined);
      });
    },
    SLOW_FRONTSTAGE_TEST_TIMEOUT
  );

  test('keeps an explicit deep link when route pageId is missing from the cropped tree', () => {
    authenticate(['frontstage.page.design']);
    const onNavigatePage = vi.fn();
    const backendTree = [createBackendPage('page-1')];

    const view = render(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          pageId="page-1"
          onNavigatePage={onNavigatePage}
          initialPageTree={backendTree}
        />
      </AppProviders>
    );

    onNavigatePage.mockReset();
    view.rerender(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          pageId="non-existent-page"
          onNavigatePage={onNavigatePage}
          initialPageTree={backendTree}
        />
      </AppProviders>
    );

    expect(onNavigatePage).not.toHaveBeenCalled();
  });

  test('keeps an explicit deep link when the accessible tree is empty', () => {
    authenticate(['frontstage.page.design']);
    const onNavigatePage = vi.fn();

    renderPageWithInitialTree([], 'invalid-page-id', onNavigatePage);

    expect(onNavigatePage).not.toHaveBeenCalled();
  });

  test('synchronizes page tree when initialPageTree updates', () => {
    authenticate(['frontstage.page.design']);
    const onNavigatePage = vi.fn();
    const view = render(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          onNavigatePage={onNavigatePage}
        />
      </AppProviders>
    );
    const workspace = screen.getByTestId('frontstage-page-workspace');

    expect(
      workspace.querySelector('.ant-empty')
    ).toBeInTheDocument();
    expect(
      workspace.querySelector('.frontstage-page-workspace__header')
    ).not.toBeInTheDocument();
    expect(screen.queryByText('分组 一级')).not.toBeInTheDocument();
    expect(onNavigatePage).not.toHaveBeenCalled();

    view.rerender(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          onNavigatePage={onNavigatePage}
          initialPageTree={[
            {
              id: 'group-root',
              title: '分组 一级',
              kind: 'group',
              children: [
                {
                  id: 'group-inner',
                  title: '分组 二级',
                  kind: 'group',
                  children: [
                    {
                      id: 'page-1',
                      title: '页面 内页',
                      kind: 'page'
                    }
                  ]
                }
              ]
            }
          ]}
        />
      </AppProviders>
    );

    expect(screen.getByText('分组 一级')).toBeInTheDocument();
    expect(screen.queryByText('分组 二级')).not.toBeInTheDocument();
    expect(screen.getAllByText('页面 内页').length).toBeGreaterThan(0);
    expect(
      screen.getByRole('heading', { name: '页面 内页' })
    ).toBeInTheDocument();
    expect(onNavigatePage).toHaveBeenCalledWith('page-1');
  });
});
