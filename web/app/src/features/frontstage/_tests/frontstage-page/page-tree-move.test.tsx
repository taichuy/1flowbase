import { createNativePreparationSource } from '../page-canvas/fixtures/native-preparation-source';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
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
  useFrontstagePageCanvasNativePreparations: vi.fn(() => ({
    preparations: createNativePreparationSource([]),
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

vi.mock(
  '../../hooks/use-frontstage-page-content-save',
  () => pageContentSaveHook
);
vi.mock('../../hooks/use-frontstage-block-catalog', () => blockCatalogHook);
vi.mock('../../hooks/use-frontstage-block-code', () => blockCodeHook);
vi.mock(
  '../../hooks/use-frontstage-page-canvas-native-preparations',
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

async function findLatestVisibleText(label: string | RegExp) {
  let elements: HTMLElement[] = [];
  await waitFor(
    () => {
      const activeDropdowns = document.body.querySelectorAll<HTMLElement>(
        '.ant-dropdown:not(.ant-dropdown-hidden), .ant-dropdown-menu-submenu-popup:not(.ant-dropdown-hidden)'
      );
      elements = Array.from(activeDropdowns).flatMap((dropdown) =>
        within(dropdown).queryAllByText(label)
      );
      expect(elements.length).toBeGreaterThan(0);
    },
    { timeout: 5_000 }
  );
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

describe('FrontStagePage - page tree move', () => {
  beforeEach(() => {
    resetAuthStore();
    resetFrontstageDesignModeStore();
    vi.clearAllMocks();
    mockPageContentSaveState();
    mockFrontstageBlockCatalog();
    mockFrontstageBlockCode();
    blockCodeApi.saveFrontstageBlockCode.mockResolvedValue({
      pageId: 'page-1',
      codeRef: 'frontstage-js-block-1-code',
      code: 'saved template'
    });
  });

  function renderMovePage(
    onMovePageNode = vi.fn().mockResolvedValue(undefined)
  ) {
    authenticate(['frontstage.page.design']);
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
              children: [
                createBackendPage('page-1'),
                createBackendPage('page-2')
              ]
            },
            {
              id: 'group-2',
              title: '分组 2',
              kind: 'group',
              children: [
                {
                  id: 'group-3',
                  title: '嵌套分组',
                  kind: 'group',
                  children: [createBackendPage('page-3')]
                }
              ]
            }
          ]}
          onMovePageNode={onMovePageNode}
        />
      </AppProviders>
    );
    activateDesignMode();
    return { onMovePageNode };
  }

  async function openMoveDialog(pageTitle = '页面 page-2') {
    await openPageTreeOperationMenuAndFlush(getPageTreeItem(pageTitle));
    expect(screen.queryByText('上移')).not.toBeInTheDocument();
    expect(screen.queryByText('下移')).not.toBeInTheDocument();
    await clickAndFlush(await findLatestVisibleText('移动到'));
    return screen.findByRole('dialog');
  }

  test('sidebar group operations move the whole group into another group', async () => {
    const { onMovePageNode } = renderMovePage();
    const group = screen.getByTestId('frontstage-tree-node-group-分组 1');
    await openPageTreeOperationMenuAndFlush(group);
    await clickAndFlush(await findLatestVisibleText('移动到'));
    const dialog = await screen.findByRole('dialog');
    await clickAndFlush(within(dialog).getByText('分组 2'));
    await clickAndFlush(
      within(dialog).getByRole('button', { name: /确\s*定/ })
    );
    await waitFor(() =>
      expect(onMovePageNode).toHaveBeenCalledWith('group-1', {
        parentId: 'group-2',
        after_id: 'group-3'
      })
    );
  });

  test('moves the operated unselected page to a nested group after confirmation', async () => {
    const { onMovePageNode } = renderMovePage();
    const dialog = await openMoveDialog();
    expect(dialog).toHaveTextContent('移动“页面 page-2”到');
    const confirm = within(dialog).getByRole('button', { name: /确\s*定/ });
    expect(confirm).toBeDisabled();
    await clickAndFlush(within(dialog).getByText('分组 1'));
    expect(confirm).toBeEnabled();
    expect(within(dialog).queryByText('嵌套分组')).not.toBeInTheDocument();
    await clickAndFlush(within(dialog).getByText('分组 2'));
    expect(within(dialog).queryByText('页面 page-3')).not.toBeInTheDocument();
    await clickAndFlush(within(dialog).getByText('嵌套分组'));
    await clickAndFlush(within(dialog).getByText('页面 page-3'));
    expect(confirm).toBeEnabled();
    expect(onMovePageNode).not.toHaveBeenCalled();
    await clickAndFlush(confirm);
    await waitFor(() =>
      expect(onMovePageNode).toHaveBeenCalledWith('page-2', {
        parentId: 'group-3',
        after_id: 'page-3'
      })
    );
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
    expect(
      getPageTreeItem('页面 page-1').querySelector(
        '.frontstage-page-tree-sidebar__node-row'
      )
    ).toHaveClass('frontstage-page-tree-sidebar__node-row--selected');
  });

  test('allows moving to the root and cancelling without saving', async () => {
    const { onMovePageNode } = renderMovePage();
    let dialog = await openMoveDialog();
    await clickAndFlush(within(dialog).getByText('顶部导航栏'));
    await clickAndFlush(
      within(dialog).getByRole('button', { name: /取\s*消/ })
    );
    expect(onMovePageNode).not.toHaveBeenCalled();
    dialog = await openMoveDialog();
    expect(
      within(dialog).getByRole('button', { name: /确\s*定/ })
    ).toBeDisabled();
    await clickAndFlush(within(dialog).getByText('顶部导航栏'));
    await clickAndFlush(
      within(dialog).getByRole('button', { name: /确\s*定/ })
    );
    await waitFor(() =>
      expect(onMovePageNode).toHaveBeenCalledWith('page-2', {
        parentId: null,
        after_id: 'group-2'
      })
    );
  });

  test('retains destination on failure and closes only after a successful retry', async () => {
    let finishMove!: () => void;
    const onMovePageNode = vi
      .fn()
      .mockRejectedValueOnce(new Error('move failed'))
      .mockImplementationOnce(
        () =>
          new Promise<void>((resolve) => {
            finishMove = resolve;
          })
      );
    renderMovePage(onMovePageNode);
    const dialog = await openMoveDialog('页面 page-1');
    await clickAndFlush(within(dialog).getByText('分组 2'));
    await clickAndFlush(
      within(dialog).getByRole('button', { name: /确\s*定/ })
    );
    expect(await within(dialog).findByRole('alert')).toHaveTextContent(
      '操作失败'
    );
    expect(
      within(dialog).getByRole('button', { name: /确\s*定/ })
    ).toBeEnabled();
    await clickAndFlush(
      within(dialog).getByRole('button', { name: /确\s*定/ })
    );
    expect(
      within(dialog).getByRole('button', { name: /取\s*消/ })
    ).toBeDisabled();
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(onMovePageNode).toHaveBeenNthCalledWith(2, 'page-1', {
      parentId: 'group-2',
      after_id: 'group-3'
    });
    await act(async () => finishMove());
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
  });

  test('shows the full route tree outside the sidebar and submits the actual root destination', async () => {
    authenticate(['frontstage.page.design']);
    const onMovePageNode = vi.fn().mockResolvedValue(undefined);
    const page = createBackendPage('page-1');
    render(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          pageId="page-1"
          initialPageTree={[page]}
          pageTreeRootId="route-1"
          navigationPageTree={[
            {
              id: 'route-1',
              title: '当前路由分组',
              kind: 'group',
              children: [page]
            },
            {
              id: 'route-2',
              title: '其他路由分组',
              kind: 'group',
              children: [createBackendPage('page-2')]
            }
          ]}
          onMovePageNode={onMovePageNode}
        />
      </AppProviders>
    );
    activateDesignMode();
    let dialog = await openMoveDialog('页面 page-1');
    const confirm = within(dialog).getByRole('button', { name: /确\s*定/ });
    await clickAndFlush(within(dialog).getByText('当前路由分组'));
    expect(confirm).toBeEnabled();
    await clickAndFlush(within(dialog).getByText('其他路由分组'));
    await clickAndFlush(confirm);
    expect(onMovePageNode).toHaveBeenLastCalledWith('page-1', {
      parentId: 'route-2',
      after_id: 'page-2'
    });
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
    dialog = await openMoveDialog('页面 page-1');
    await clickAndFlush(within(dialog).getByText('顶部导航栏'));
    await clickAndFlush(
      within(dialog).getByRole('button', { name: /确\s*定/ })
    );
    expect(onMovePageNode).toHaveBeenLastCalledWith('page-1', {
      parentId: null,
      after_id: 'route-2'
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
      setDragImage: vi.fn(),
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
    const over = new Event('dragover', { bubbles: true, cancelable: true });
    Object.defineProperties(over, {
      clientY: { value: 50 },
      dataTransfer: { value: dataTransfer }
    });
    fireEvent(groupItem, over);
    fireEvent.drop(groupItem, { clientY: 50, dataTransfer });

    await waitFor(() => {
      expect(onMovePageNode).toHaveBeenCalledWith('page-1', {
        parentId: 'group-1',
        rank: '001000'
      });
    });
    rectSpy.mockRestore();
  });
});
