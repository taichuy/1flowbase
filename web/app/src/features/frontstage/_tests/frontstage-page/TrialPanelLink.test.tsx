import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { AppProviders } from '../../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import {
  resetFrontstageDesignModeStore,
  useFrontstageDesignModeStore
} from '../../../../state/frontstage-design-mode-store';
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

function authenticate() {
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
      permissions: ['frontstage.page.design']
    }
  });
}

function createPageContent(blocks?: Array<Record<string, unknown>>) {
  const payload = blocks ?? [
    {
      id: 'cta',
      codeRef: 'cta-code',
      contribution: { code: 'blocks.cta' },
      props: {},
      'x-layout': { order: 1, region: 'main' },
      runtime: { kind: 'js-ui', entry: null, hint: 'js-ui' }
    }
  ];
  return {
    page: {
      id: 'page-1',
      title: 'Landing',
      kind: 'page' as const,
      parentId: null,
      rank: '001000',
      schemaRootUid: 'root-1'
    },
    schema: { rootUid: 'root-1', payload: { blocks: payload } },
    root: { uid: 'root-1', payload: { blocks: payload } }
  };
}

function renderFrontStagePage() {
  return render(
    <AppProviders>
      <FrontStagePage
        workspaceId="workspace-1"
        pageId="page-1"
        initialPageTree={[
          { id: 'page-1', title: '页面 page-1', kind: 'page' }
        ]}
        pageContent={createPageContent()}
      />
    </AppProviders>
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

describe('FrontStagePage trial panel link', () => {
  beforeEach(() => {
    resetAuthStore();
    resetFrontstageDesignModeStore();
    vi.clearAllMocks();
    pageContentSaveHook.useFrontstagePageContentSave.mockReturnValue({
      save: vi.fn(() => Promise.resolve(createPageContent())),
      saving: false,
      isPending: false,
      error: null,
      reset: vi.fn(),
      clearError: vi.fn()
    });
    blockCatalogHook.useFrontstageBlockCatalog.mockReturnValue({
      items: [],
      diagnostics: [],
      loading: false,
      error: null
    });
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
    runtimeSessionsHook.useFrontstagePageCanvasRuntimeSessions.mockReturnValue({
      entries: [],
      snapshotsBySlot: {},
      running: false,
      hasError: false
    });
  });

  test('opens JS Block Trial panel from the Write JavaScript drawer', async () => {
    authenticate();
    renderFrontStagePage();

    activateDesignMode();
    fireEvent.click(screen.getByRole('button', { name: '区块 cta' }));
    fireEvent.click(screen.getByRole('button', { name: '编辑区块' }));
    fireEvent.click(
      screen.getByRole('button', { name: 'JS Block 试运行' })
    );

    expect(
      await screen.findByText('JS 区块试运行')
    ).toBeInTheDocument();
  }, 10000);

  test('closes JS Block Trial panel when exiting design mode', async () => {
    authenticate();
    renderFrontStagePage();

    activateDesignMode();
    fireEvent.click(screen.getByRole('button', { name: '区块 cta' }));
    fireEvent.click(screen.getByRole('button', { name: '编辑区块' }));
    fireEvent.click(
      screen.getByRole('button', { name: 'JS Block 试运行' })
    );

    expect(
      await screen.findByText('JS 区块试运行')
    ).toBeInTheDocument();

    // Exit design mode — Drawer should close
    exitDesignMode();

    await waitFor(() => {
      expect(
        screen.queryByText('JS 区块试运行')
      ).not.toBeInTheDocument();
    });
  }, 20_000);
});
