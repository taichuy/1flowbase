import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { AppProviders } from '../../../../app/AppProviders';
import { appI18n } from '../../../../shared/i18n/app-i18n';
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
import { FrontStagePage } from '../../pages/FrontStagePage';

vi.setConfig({ testTimeout: 10_000 });

const pageContentSaveHook = vi.hoisted(() => ({
  useFrontstagePageContentSave: vi.fn()
}));
const blockCatalogHook = vi.hoisted(() => ({
  useFrontstageBlockCatalog: vi.fn()
}));
const blockCodeHook = vi.hoisted(() => ({
  useFrontstageBlockCode: vi.fn()
}));

vi.mock(
  '../../hooks/use-frontstage-page-content-save',
  () => pageContentSaveHook
);
vi.mock('../../hooks/use-frontstage-block-catalog', () => blockCatalogHook);
vi.mock('../../hooks/use-frontstage-block-code', () => blockCodeHook);
vi.mock('../../components/jsx-studio/FrontstageJsxStudioDrawer', () => ({
  FrontstageJsxStudioDrawer: ({
    open,
    initialSection,
    workspaceId,
    pageId,
    block
  }: {
    open: boolean;
    initialSection: string;
    workspaceId: string | null | undefined;
    pageId: string | null | undefined;
    block?: { id?: string; codeRef?: string | null } | null;
  }) =>
    open ? (
      <dialog
        open
        aria-label="TSX 编辑器"
        data-initial-section={initialSection}
      >
        <span>workspace:{workspaceId ?? 'none'}</span>
        <span>page:{pageId ?? 'none'}</span>
        <span>block:{block?.id ?? 'none'}</span>
        <span>code:{block?.codeRef ?? 'none'}</span>
      </dialog>
    ) : null
}));

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

function createPageContent(
  overrides: FrontstagePageContentFixtureOverrides = {}
): FrontstagePageContent {
  return createFrontstagePageContentFixture(overrides);
}

function createBlockPayload(blockId: string, order: number) {
  return {
    id: blockId,
    codeRef: `${blockId}-code`,
    catalog: {
      providerCode: null,
      installationId: null
    },
    contribution: {
      pluginId: null,
      pluginVersion: null,
      code: 'frontstage.js-ui-block'
    },
    props: {},
    'x-layout': {
      order,
      region: 'main'
    },
    runtime: {
      kind: 'js-ui',
      entry: null,
      hint: 'js-ui'
    }
  };
}

function createPageContentWithBlocks(
  blockIds: string[]
): FrontstagePageContent {
  const blocks = blockIds.map((blockId, index) =>
    createBlockPayload(blockId, index)
  );

  return createPageContent({
    schema: {
      rootUid: 'root-1',
      payload: { blocks }
    },
    root: {
      uid: 'root-1',
      payload: { blocks }
    }
  });
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

function getSavedBlockIds(input: SaveFrontstageTabDocumentInput): unknown[] {
  return getSavedBlocks(input).map((block) => block.id);
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

function mockFrontstageBlockCatalog() {
  blockCatalogHook.useFrontstageBlockCatalog.mockReturnValue({
    items: [],
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

function renderFrontStagePage(pageContent: FrontstagePageContent) {
  return render(
    <AppProviders>
      <FrontStagePage
        workspaceId="workspace-1"
        pageId="page-1"
        initialPageTree={[
          {
            id: 'page-1',
            title: '页面 page-1',
            kind: 'page'
          }
        ]}
        pageContent={pageContent}
      />
    </AppProviders>
  );
}

function getBlockRow(blockId: string) {
  return screen.getByTestId(`block-slot-${blockId}`);
}

async function clickAndFlush(element: HTMLElement) {
  await act(async () => {
    element.click();
  });
}

function clickBlockToolbar(blockId: string, buttonName: string) {
  fireEvent.click(
    within(screen.getByTestId(`block-slot-${blockId}`)).getByRole('button', {
      name: buttonName
    })
  );
}

async function confirmBlockDelete(blockId: string) {
  clickBlockToolbar(blockId, '更多区块操作');
  const deleteAction = (await screen.findByText('删除')).closest('button');
  if (!deleteAction) {
    throw new Error('Missing block delete action');
  }
  await clickAndFlush(deleteAction);
  await clickAndFlush(await screen.findByRole('button', { name: /删\s*除/ }));
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

describe('FrontStagePage block arrange actions', () => {
  beforeEach(async () => {
    await appI18n.changeLanguage('zh_Hans');
    resetAuthStore();
    resetFrontstageDesignModeStore();
    vi.clearAllMocks();
    mockPageContentSaveState();
    mockFrontstageBlockCatalog();
    mockFrontstageBlockCode();
  });

  test('saves selected block deletion and falls back to the next block', async () => {
    authenticate(['frontstage.page.design']);
    const saveState = mockPageContentSaveState();
    renderFrontStagePage(
      createPageContentWithBlocks(['hero', 'feature', 'cta'])
    );

    await activateDesignMode();
    await clickAndFlush(getBlockRow('feature'));
    await confirmBlockDelete('feature');

    await waitFor(() => {
      expect(saveState.save).toHaveBeenCalledTimes(1);
    });

    const [saveInput] = saveState.save.mock.calls[0] as [
      SaveFrontstageTabDocumentInput
    ];
    expect(getSavedBlockIds(saveInput)).toEqual(['hero', 'cta']);

    await waitFor(() => {
      expect(
        screen.queryByTestId('block-slot-feature')
      ).not.toBeInTheDocument();
      expect(screen.getByTestId('block-slot-hero')).toBeInTheDocument();
      expect(screen.getByTestId('block-slot-cta')).toBeInTheDocument();
    });
  });

  test('clears selected block when deleting the only block', async () => {
    authenticate(['frontstage.page.design']);
    const saveState = mockPageContentSaveState();
    renderFrontStagePage(createPageContentWithBlocks(['hero']));

    await activateDesignMode();
    await clickAndFlush(getBlockRow('hero'));
    await confirmBlockDelete('hero');

    await waitFor(() => {
      expect(saveState.save).toHaveBeenCalledTimes(1);
    });

    const [saveInput] = saveState.save.mock.calls[0] as [
      SaveFrontstageTabDocumentInput
    ];
    expect(getSavedBlockIds(saveInput)).toEqual([]);

    await waitFor(() => {
      expect(
        screen.getByTestId('page-canvas-design-empty-state')
      ).toBeEmptyDOMElement();
    });
  });

  test('disables selected block arrange actions while page content is saving', async () => {
    authenticate(['frontstage.page.design']);
    mockPageContentSaveState({ saving: true, isPending: true });
    renderFrontStagePage(
      createPageContentWithBlocks(['hero', 'feature', 'cta'])
    );

    await activateDesignMode();
    await clickAndFlush(getBlockRow('feature'));

    expect(
      within(getBlockRow('feature')).getByRole('button', {
        name: '移动或排序区块'
      })
    ).toBeDisabled();
    expect(
      within(getBlockRow('feature')).getByRole('button', { name: '编辑区块' })
    ).toBeDisabled();
    expect(
      within(getBlockRow('feature')).getByRole('button', {
        name: '更多区块操作'
      })
    ).toBeDisabled();
    expect(screen.getByText('区块保存中')).toBeInTheDocument();
  });

  test('opens JSX Studio on the code section for the selected block in design mode', async () => {
    authenticate(['frontstage.page.design']);
    renderFrontStagePage(createPageContentWithBlocks(['hero', 'cta']));

    await activateDesignMode();
    await clickAndFlush(getBlockRow('hero'));
    clickBlockToolbar('hero', '编辑区块');

    const dialog = await screen.findByRole('dialog', { name: 'TSX 编辑器' });
    expect(dialog).toHaveAttribute('data-initial-section', 'code');
    expect(
      within(dialog).getByText('workspace:workspace-1')
    ).toBeInTheDocument();
    expect(within(dialog).getByText('page:page-1')).toBeInTheDocument();
    expect(within(dialog).getByText('block:hero')).toBeInTheDocument();
    expect(within(dialog).getByText('code:hero-code')).toBeInTheDocument();
  });

  test('#1300 exposes one JSX Studio edit action for a selected block', async () => {
    authenticate(['frontstage.page.design']);
    renderFrontStagePage(createPageContentWithBlocks(['hero']));

    await activateDesignMode();
    await clickAndFlush(getBlockRow('hero'));

    const block = within(getBlockRow('hero'));
    const editButton = block.getByRole('button', { name: '编辑区块' });
    expect(
      block.queryByRole('button', { name: '区块配置' })
    ).not.toBeInTheDocument();
    expect(
      block.queryByRole('button', { name: '区块代码' })
    ).not.toBeInTheDocument();

    fireEvent.click(editButton);
    const studio = await screen.findByRole('dialog', { name: 'TSX 编辑器' });
    expect(studio).toHaveAttribute('data-initial-section', 'code');
  });

  test('hides block editor entry outside design mode and without design permission', async () => {
    authenticate(['frontstage.page.design']);
    const view = renderFrontStagePage(createPageContentWithBlocks(['hero']));

    await clickAndFlush(getBlockRow('hero'));
    expect(
      screen.queryByRole('button', { name: '编辑区块' })
    ).not.toBeInTheDocument();

    await activateDesignMode();
    await clickAndFlush(getBlockRow('hero'));
    expect(
      within(getBlockRow('hero')).getByRole('button', { name: '编辑区块' })
    ).toBeVisible();

    await exitDesignMode();
    expect(
      screen.queryByRole('button', { name: '编辑区块' })
    ).not.toBeInTheDocument();

    await act(async () => {
      resetAuthStore();
      authenticate([]);
    });
    view.rerender(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          pageId="page-1"
          initialPageTree={[
            {
              id: 'page-1',
              title: '页面 page-1',
              kind: 'page'
            }
          ]}
          pageContent={createPageContentWithBlocks(['hero'])}
        />
      </AppProviders>
    );

    expect(
      screen.queryByRole('button', { name: '编辑区块' })
    ).not.toBeInTheDocument();
  });

  test('closes Studio when exiting design mode or switching pages', async () => {
    authenticate(['frontstage.page.design']);
    const pageTree = [
      {
        id: 'page-1',
        title: '页面 page-1',
        kind: 'page' as const
      },
      {
        id: 'page-2',
        title: '页面 page-2',
        kind: 'page' as const
      }
    ];
    const view = render(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          pageId="page-1"
          initialPageTree={pageTree}
          pageContent={createPageContentWithBlocks(['hero'])}
        />
      </AppProviders>
    );

    await activateDesignMode();
    await clickAndFlush(getBlockRow('hero'));
    clickBlockToolbar('hero', '编辑区块');
    expect(
      await screen.findByRole('dialog', { name: 'TSX 编辑器' })
    ).toBeInTheDocument();

    await exitDesignMode();
    await waitFor(() => {
      expect(
        screen.queryByRole('dialog', { name: 'TSX 编辑器' })
      ).not.toBeInTheDocument();
    });

    await activateDesignMode();
    await clickAndFlush(getBlockRow('hero'));
    clickBlockToolbar('hero', '编辑区块');
    expect(
      await screen.findByRole('dialog', { name: 'TSX 编辑器' })
    ).toBeInTheDocument();

    fireEvent.click(
      screen.getByRole('button', {
        name: /页面 page-2/
      })
    );
    view.rerender(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          pageId="page-2"
          initialPageTree={pageTree}
          pageContent={createPageContent({
            page: {
              id: 'page-2',
              title: 'Second',
              kind: 'page',
              parentId: null,
              rank: '002000'
            },
            schema: {
              rootUid: 'root-2',
              payload: {}
            },
            root: {
              uid: 'root-2',
              payload: {}
            }
          })}
        />
      </AppProviders>
    );

    await waitFor(() => {
      expect(
        screen.queryByRole('dialog', { name: 'TSX 编辑器' })
      ).not.toBeInTheDocument();
    });
  });

  test('shows a clear block save error in design mode', async () => {
    authenticate(['frontstage.page.design']);
    mockPageContentSaveState({
      save: vi.fn(() => Promise.reject(new Error('arrange failed')))
    });
    renderFrontStagePage(createPageContentWithBlocks(['hero', 'cta']));

    await activateDesignMode();
    await clickAndFlush(getBlockRow('cta'));
    await confirmBlockDelete('cta');

    expect(await screen.findByText('区块保存失败')).toBeInTheDocument();
    expect(screen.getByText('arrange failed')).toBeInTheDocument();
  });

  test('does not show block action toolbar in browsing mode or without design permission', async () => {
    authenticate(['frontstage.page.design']);
    const view = renderFrontStagePage(
      createPageContentWithBlocks(['hero', 'cta'])
    );

    await clickAndFlush(getBlockRow('hero'));
    expect(
      screen.queryByRole('button', { name: '更多区块操作' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '删除' })
    ).not.toBeInTheDocument();

    await act(async () => {
      resetAuthStore();
      authenticate([]);
    });
    view.rerender(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          pageId="page-1"
          initialPageTree={[
            {
              id: 'page-1',
              title: '页面 page-1',
              kind: 'page'
            }
          ]}
          pageContent={createPageContentWithBlocks(['hero', 'cta'])}
        />
      </AppProviders>
    );

    expect(
      screen.queryByRole('button', { name: '进入设计模式' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '更多区块操作' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '删除' })
    ).not.toBeInTheDocument();
  });
});
