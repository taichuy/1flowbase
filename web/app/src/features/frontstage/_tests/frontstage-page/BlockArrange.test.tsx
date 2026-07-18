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
  SaveFrontstagePageContentInput
} from '../../api/page-content';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
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
        aria-label="JSX Studio"
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
  overrides: Partial<FrontstagePageContent> = {}
): FrontstagePageContent {
  return {
    page: {
      id: 'page-1',
      title: 'Landing',
      kind: 'page',
      parentId: null,
      rank: '001000'
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

function createConfigurableBlockPayload() {
  return {
    id: 'hero',
    codeRef: 'hero-code',
    catalog: {
      providerCode: 'official',
      installationId: 'installation-1'
    },
    contribution: {
      pluginId: 'official.blocks',
      pluginVersion: '1.0.0',
      code: 'hero.banner'
    },
    props: {
      title: 'Configured title',
      data: {
        model: 'orders',
        fields: ['id', 'title'],
        operations: {
          query: true,
          update: true
        },
        pagination: { pageSize: 20 }
      }
    },
    'x-layout': {
      order: 3,
      width: 12,
      height: 5,
      region: 'main'
    },
    runtime: {
      kind: 'js-ui',
      entry: 'blocks/hero/index.js',
      hint: 'js-ui'
    }
  };
}

function createCatalogEntry(): NormalizedFrontstageBlockCatalogEntry {
  return {
    id: 'official:hero.banner',
    runtimeKind: 'iframe',
    installationId: 'installation-1',
    providerCode: 'official',
    pluginId: 'official.blocks',
    pluginVersion: '1.0.0',
    contributionCode: 'hero.banner',
    title: 'Hero Banner',
    entry: 'blocks/hero/index.js',
    permissions: {
      network: 'none',
      storage: 'none',
      secrets: 'none'
    },
    contextContract: {
      primitives: ['text', 'data_record'],
      inputSchema: {
        type: 'object',
        properties: {
          recordId: { type: 'string' }
        }
      }
    },
    uiCapabilities: ['configurable', 'data_binding'],
    raw: {
      installation_id: 'installation-1',
      provider_code: 'official',
      plugin_id: 'official.blocks',
      plugin_version: '1.0.0',
      contribution_code: 'hero.banner',
      title: 'Hero Banner',
      runtime: 'iframe',
      entry: 'blocks/hero/index.js',
      context_contract: {
        primitives: ['text', 'data_record'],
        input_schema: {
          type: 'object'
        }
      },
      permissions: {
        network: 'none',
        storage: 'none',
        secrets: 'none'
      },
      ui_capabilities: ['configurable', 'data_binding']
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

function createPageContentWithBlockPayloads(
  blocks: Array<Record<string, unknown>>
): FrontstagePageContent {
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

function getSavedBlockIds(input: SaveFrontstagePageContentInput): unknown[] {
  return getSavedBlocks(input).map((block) => block.id);
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

async function clickBlockMoveAction(blockId: string, actionName: string) {
  clickBlockToolbar(blockId, '更多区块操作');
  const action = (await screen.findByText(actionName)).closest('button');
  if (!action) {
    throw new Error(`Missing block action button: ${actionName}`);
  }
  await clickAndFlush(action);
}

async function confirmBlockDelete(blockId: string) {
  clickBlockToolbar(blockId, '更多区块操作');
  const deleteAction = (await screen.findByText('删除')).closest('button');
  if (!deleteAction) {
    throw new Error('Missing block delete action');
  }
  await clickAndFlush(deleteAction);
  await clickAndFlush(
    await screen.findByRole('button', { name: /删\s*除/ })
  );
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
      SaveFrontstagePageContentInput
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
      SaveFrontstagePageContentInput
    ];
    expect(getSavedBlockIds(saveInput)).toEqual([]);

    await waitFor(() => {
      expect(screen.getByText('页面内容为空')).toBeInTheDocument();
    });
  });

  test('saves selected block move down and move up', async () => {
    authenticate(['frontstage.page.design']);
    const saveState = mockPageContentSaveState();
    renderFrontStagePage(
      createPageContentWithBlocks(['hero', 'feature', 'cta'])
    );

    await activateDesignMode();
    await clickAndFlush(getBlockRow('feature'));
    await clickBlockMoveAction('feature', '下移区块');

    await waitFor(() => {
      expect(saveState.save).toHaveBeenCalledTimes(1);
    });

    const [moveDownInput] = saveState.save.mock.calls[0] as [
      SaveFrontstagePageContentInput
    ];
    expect(getSavedBlockIds(moveDownInput)).toEqual(['hero', 'cta', 'feature']);

    await clickBlockMoveAction('feature', '上移区块');

    await waitFor(() => {
      expect(saveState.save).toHaveBeenCalledTimes(2);
    });

    const [moveUpInput] = saveState.save.mock.calls[1] as [
      SaveFrontstagePageContentInput
    ];
    expect(getSavedBlockIds(moveUpInput)).toEqual(['hero', 'feature', 'cta']);
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
      within(getBlockRow('feature')).getByRole('button', { name: '区块配置' })
    ).toBeDisabled();
    expect(
      within(getBlockRow('feature')).getByRole('button', { name: '区块代码' })
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
    clickBlockToolbar('hero', '区块代码');

    const dialog = await screen.findByRole('dialog', { name: 'JSX Studio' });
    expect(dialog).toHaveAttribute('data-initial-section', 'code');
    expect(
      within(dialog).getByText('workspace:workspace-1')
    ).toBeInTheDocument();
    expect(within(dialog).getByText('page:page-1')).toBeInTheDocument();
    expect(within(dialog).getByText('block:hero')).toBeInTheDocument();
    expect(within(dialog).getByText('code:hero-code')).toBeInTheDocument();
  });

  test('#1300 exposes adjacent configuration and JSX editor actions for a selected block', async () => {
    authenticate(['frontstage.page.design']);
    renderFrontStagePage(createPageContentWithBlocks(['hero']));

    await activateDesignMode();
    await clickAndFlush(getBlockRow('hero'));

    const block = within(getBlockRow('hero'));
    const configurationButton = block.getByRole('button', {
      name: '区块配置'
    });
    const codeButton = block.getByRole('button', { name: '区块代码' });
    const buttons = block.getAllByRole('button');

    expect(buttons.indexOf(codeButton)).toBe(
      buttons.indexOf(configurationButton) + 1
    );
    expect(
      block.queryByRole('button', { name: '编辑区块' })
    ).not.toBeInTheDocument();

    fireEvent.click(configurationButton);
    const studio = await screen.findByRole('dialog', { name: 'JSX Studio' });
    expect(studio).toHaveAttribute('data-initial-section', 'configuration');
  });

  test('hides block code editor entry outside design mode and without design permission', async () => {
    authenticate(['frontstage.page.design']);
    const view = renderFrontStagePage(createPageContentWithBlocks(['hero']));

    await clickAndFlush(getBlockRow('hero'));
    expect(
      screen.queryByRole('button', { name: '区块代码' })
    ).not.toBeInTheDocument();

    await activateDesignMode();
    await clickAndFlush(getBlockRow('hero'));
    expect(
      within(getBlockRow('hero')).getByRole('button', { name: '区块代码' })
    ).toBeVisible();

    await exitDesignMode();
    expect(
      screen.queryByRole('button', { name: '区块代码' })
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
      screen.queryByRole('button', { name: '区块代码' })
    ).not.toBeInTheDocument();
  });

  test('shows block configuration entry only for a selected block in design mode with design permission', async () => {
    authenticate(['frontstage.page.design']);
    const view = renderFrontStagePage(createPageContentWithBlocks(['hero']));

    await clickAndFlush(getBlockRow('hero'));
    expect(
      screen.queryByRole('button', { name: '区块配置' })
    ).not.toBeInTheDocument();

    await activateDesignMode();
    expect(
      screen.queryByRole('button', { name: '区块配置' })
    ).not.toBeInTheDocument();

    await clickAndFlush(getBlockRow('hero'));
    expect(
      within(getBlockRow('hero')).getByRole('button', { name: '区块配置' })
    ).toBeInTheDocument();

    await exitDesignMode();
    expect(
      screen.queryByRole('button', { name: '区块配置' })
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
      screen.queryByRole('button', { name: '区块配置' })
    ).not.toBeInTheDocument();
  });

  test('opens the shared JSX Studio on the configuration section', async () => {
    authenticate(['frontstage.page.design']);
    mockFrontstageBlockCatalog([createCatalogEntry()]);
    renderFrontStagePage(
      createPageContentWithBlockPayloads([createConfigurableBlockPayload()])
    );

    await activateDesignMode();
    await clickAndFlush(getBlockRow('hero'));
    clickBlockToolbar('hero', '区块配置');

    const dialog = await screen.findByRole('dialog', { name: 'JSX Studio' });
    expect(dialog).toHaveAttribute('data-initial-section', 'configuration');
    expect(within(dialog).getByText('block:hero')).toBeInTheDocument();
    expect(within(dialog).getByText('code:hero-code')).toBeInTheDocument();
  });

  test('closes configuration Studio when exiting design mode, switching pages, or clearing selection', async () => {
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
    clickBlockToolbar('hero', '区块配置');
    expect(
      await screen.findByRole('dialog', { name: 'JSX Studio' })
    ).toBeInTheDocument();

    await exitDesignMode();
    await waitFor(() => {
      expect(
        screen.queryByRole('dialog', { name: 'JSX Studio' })
      ).not.toBeInTheDocument();
    });

    await activateDesignMode();
    await clickAndFlush(getBlockRow('hero'));
    clickBlockToolbar('hero', '区块配置');
    expect(
      await screen.findByRole('dialog', { name: 'JSX Studio' })
    ).toBeInTheDocument();

    await clickAndFlush(getBlockRow('hero'));
    await waitFor(() => {
      expect(
        screen.queryByRole('dialog', { name: 'JSX Studio' })
      ).not.toBeInTheDocument();
    });

    await clickAndFlush(getBlockRow('hero'));
    clickBlockToolbar('hero', '区块配置');
    expect(
      await screen.findByRole('dialog', { name: 'JSX Studio' })
    ).toBeInTheDocument();

    // Switch page
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
        screen.queryByRole('dialog', { name: 'JSX Studio' })
      ).not.toBeInTheDocument();
    });
  }, 25000);

  test('closes code Studio when exiting design mode or switching pages', async () => {
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
    clickBlockToolbar('hero', '区块代码');
    expect(
      await screen.findByRole('dialog', { name: 'JSX Studio' })
    ).toBeInTheDocument();

    await exitDesignMode();
    await waitFor(() => {
      expect(
        screen.queryByRole('dialog', { name: 'JSX Studio' })
      ).not.toBeInTheDocument();
    });

    await activateDesignMode();
    await clickAndFlush(getBlockRow('hero'));
    clickBlockToolbar('hero', '区块代码');
    expect(
      await screen.findByRole('dialog', { name: 'JSX Studio' })
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
        screen.queryByRole('dialog', { name: 'JSX Studio' })
      ).not.toBeInTheDocument();
    });
  });

  test('shows a clear block arrange save error in design mode', async () => {
    authenticate(['frontstage.page.design']);
    mockPageContentSaveState({
      save: vi.fn(() => Promise.reject(new Error('arrange failed')))
    });
    renderFrontStagePage(createPageContentWithBlocks(['hero', 'cta']));

    await activateDesignMode();
    await clickAndFlush(getBlockRow('cta'));
    await clickBlockMoveAction('cta', '上移区块');

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
