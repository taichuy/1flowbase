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
  SaveFrontstagePageContentInput
} from '../../api/page-content';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';import type { UseFrontstagePageCanvasRuntimeSessionsResult } from '../../hooks/use-frontstage-page-canvas-runtime-sessions';
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

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function getPageTreeItem(title: string) {
  return screen.getByRole('button', {
    name: new RegExp(`${escapeRegExp(title)}\\s+页面节点`)
  });
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

async function clickPageTreeOperationSubmenuItemAndFlush(
  nodeContainer: HTMLElement,
  submenuLabel: string | RegExp,
  label: string | RegExp
) {
  await openPageTreeOperationMenuAndFlush(nodeContainer);
  const submenu = await findLatestVisibleText(submenuLabel);
  const submenuTarget = getPageTreeSubmenuTrigger(submenu);
  fireEvent.pointerEnter(submenuTarget);
  fireEvent.mouseEnter(submenuTarget);
  fireEvent.mouseOver(submenuTarget);
  fireEvent.mouseMove(submenuTarget);
  fireEvent.click(submenuTarget);
  await clickAndFlush(await findLatestVisibleText(label));
}

function getPageTreeSubmenuTrigger(submenu: HTMLElement) {
  return (
    submenu.closest('.ant-dropdown-menu-submenu-title') ??
    submenu.closest('.ant-dropdown-menu-submenu') ??
    submenu
  );
}

async function findLatestVisibleText(label: string | RegExp) {
  let elements: HTMLElement[] = [];
  await waitFor(() => {
    // eslint-disable-next-line testing-library/no-node-access
    const activeDropdowns = document.body.querySelectorAll<HTMLElement>(
      '.ant-dropdown:not(.ant-dropdown-hidden), .ant-dropdown-menu-submenu-popup:not(.ant-dropdown-hidden)'
    );
    elements = Array.from(activeDropdowns).flatMap((dropdown) =>
      within(dropdown).queryAllByText(label)
    );
    expect(elements.length).toBeGreaterThan(0);
  }, { timeout: 5_000 });
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
}describe('FrontStagePage - page tree move', () => {
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

  test('moves nodes through page tree mutation callback', async () => {
    authenticate(['frontstage.page.design']);
    const onMovePageNode = vi.fn().mockResolvedValue(undefined);

    render(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          initialPageTree={[
            createBackendPage('page-1'),
            createBackendPage('page-2')
          ]}
          onMovePageNode={onMovePageNode}
        />
      </AppProviders>
    );

    activateDesignMode();

    const secondPageItem = getPageTreeItem('页面 page-2');
    await clickPageTreeOperationSubmenuItemAndFlush(
      secondPageItem,
      '移动到',
      '上移'
    );

    await waitFor(() => {
      expect(onMovePageNode).toHaveBeenCalledWith('page-2', {
        parentId: null,
        rank: '000000'
      });
    });

    const rows = screen.getAllByRole('button', {
      name: /页面 page-\d+ 页面节点/
    });
    expect(rows[0]).toHaveTextContent('页面 page-2');
    expect(rows[1]).toHaveTextContent('页面 page-1');
  });

  test('moves selected page to another group from the operation menu', async () => {
    authenticate(['frontstage.page.design']);
    const onMovePageNode = vi.fn().mockResolvedValue(undefined);

    render(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          pageId="page-1"
          initialPageTree={[
            {
              id: 'group-1',
              title: '分组 1',
              kind: 'group',
              children: [createBackendPage('page-1')]
            },
            {
              id: 'group-2',
              title: '分组 2',
              kind: 'group',
              children: []
            }
          ]}
          onMovePageNode={onMovePageNode}
        />
      </AppProviders>
    );

    activateDesignMode();

    const selectedPageItem = getPageTreeItem('页面 page-1');
    await clickPageTreeOperationSubmenuItemAndFlush(
      selectedPageItem,
      '移动到',
      '分组 2'
    );

    await waitFor(() => {
      expect(onMovePageNode).toHaveBeenCalledWith('page-1', {
        parentId: 'group-2',
        rank: '001000'
      });
    });
  });

  test('moves page into group by dragging onto the group middle', async () => {
    authenticate(['frontstage.page.design']);
    const onMovePageNode = vi.fn().mockResolvedValue(undefined);

    render(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          initialPageTree={[
            {
              id: 'group-1',
              title: '分组 group-1',
              kind: 'group',
              children: []
            },
            createBackendPage('page-1')
          ]}
          onMovePageNode={onMovePageNode}
        />
      </AppProviders>
    );

    activateDesignMode();

    const pageItem = screen.getByTestId(
      'frontstage-tree-node-page-页面 page-1'
    );
    const groupItem = screen.getByTestId(
      'frontstage-tree-node-group-分组 group-1'
    );

    const rectSpy = vi.spyOn(Element.prototype, 'getBoundingClientRect');
    rectSpy.mockReturnValue({
      x: 0,
      y: 0,
      top: 0,
      left: 0,
      bottom: 100,
      right: 240,
      width: 240,
      height: 100,
      toJSON: () => ({})
    });

    const dragHandle = within(pageItem).getByRole('button', {
      name: '拖拽移动节点'
    });
    const dataTransfer = {
      data: new Map<string, string>(),
      effectAllowed: '',
      dropEffect: '',
      setData(format: string, value: string) {
        this.data.set(format, value);
      },
      getData(format: string) {
        return this.data.get(format) ?? '';
      }
    };

    fireEvent.dragStart(dragHandle, { dataTransfer });
    fireEvent.dragOver(groupItem, { clientY: 50, dataTransfer });
    fireEvent.drop(groupItem, { clientY: 50, dataTransfer });

    await waitFor(() => {
      expect(onMovePageNode).toHaveBeenCalledWith('page-1', {
        parentId: 'group-1',
        rank: '001000'
      });
    });
    rectSpy.mockRestore();
  });

  test(
    'supports page order move controls in design mode',
    async () => {
      authenticate(['frontstage.page.design']);
      renderPage();

      activateDesignMode();
      await clickAddMenuItemAndFlush('新增页面');
      await clickAddMenuItemAndFlush('新增页面');

      const initialTreeRows = screen.getAllByRole('button', {
        name: /页面 新建 \d+ 页面节点/
      });
      expect(initialTreeRows[0]).toHaveTextContent('页面 新建 1');
      expect(initialTreeRows[1]).toHaveTextContent('页面 新建 2');

      await clickPageTreeOperationSubmenuItemAndFlush(
        initialTreeRows[1],
        '移动到',
        '上移'
      );

      const movedUpRows = screen.getAllByRole('button', {
        name: /页面 新建 \d+ 页面节点/
      });
      expect(movedUpRows[0]).toHaveTextContent('页面 新建 2');
      expect(movedUpRows[1]).toHaveTextContent('页面 新建 1');

      await clickPageTreeOperationSubmenuItemAndFlush(
        movedUpRows[0],
        '移动到',
        '下移'
      );

      const movedDownRows = screen.getAllByRole('button', {
        name: /页面 新建 \d+ 页面节点/
      });
      expect(movedDownRows[0]).toHaveTextContent('页面 新建 1');
      expect(movedDownRows[1]).toHaveTextContent('页面 新建 2');
    },
    SLOW_FRONTSTAGE_TEST_TIMEOUT
  );
});
