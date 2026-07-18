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
const dataCapabilitiesHook = vi.hoisted(() => ({
  useFrontstageDataCapabilities: vi.fn()
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
vi.mock(
  '../../hooks/use-frontstage-data-capabilities',
  () => dataCapabilitiesHook
);
vi.mock('@monaco-editor/react', () => ({
  default: ({ value }: { value?: string }) => (
    <textarea aria-label="JSX source" readOnly value={value} />
  )
}));

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
      contentPresentation: 'single' as const
    },
    tab: {
      id: 'tab-1',
      pageId: 'page-1',
      title: '概览',
      rank: '001000',
      isDefault: true,
      routeSegment: null,
      documentRootUid: 'root-1'
    },
    document: { rootUid: 'root-1', payload: { blocks: payload } }
  };
}

function renderFrontStagePage() {
  return render(
    <AppProviders>
      <FrontStagePage
        workspaceId="workspace-1"
        pageId="page-1"
        initialPageTree={[{ id: 'page-1', title: '页面 page-1', kind: 'page' }]}
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
  beforeEach(async () => {
    await appI18n.changeLanguage('zh_Hans');
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
      items: [
        {
          id: '1flowbase:frontstage.js-ui-block',
          runtimeKind: 'iframe',
          installationId: 'builtin-installation',
          providerCode: '1flowbase',
          pluginId: 'builtin-frontstage',
          pluginVersion: '1.0.0',
          contributionCode: 'frontstage.js-ui-block',
          title: 'JSX 区块',
          entry: 'index.js',
          permissions: {
            network: 'none',
            storage: 'none',
            secrets: 'none'
          },
          contextContract: { primitives: [], inputSchema: {} },
          uiCapabilities: ['configurable', 'data_binding'],
          codeCapabilities: {
            template: null,
            allowedImports: ['@1flowbase/block-renderer/antd-facade'],
            monacoExtraLibs: [],
            workerModuleSources: ['@1flowbase/block-renderer/antd-facade']
          },
          raw: {}
        }
      ],
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
      save: vi.fn(),
      permissionDenied: false
    });
    dataCapabilitiesHook.useFrontstageDataCapabilities.mockReturnValue({
      data: { queries: [], actions: [], models: [] },
      loading: false,
      error: null
    });
    runtimeSessionsHook.useFrontstagePageCanvasRuntimeSessions.mockReturnValue({
      entries: [],
      snapshotsBySlot: {},
      running: false,
      hasError: false
    });
  });

  test('opens the run preview inside the shared JSX Studio', async () => {
    authenticate();
    renderFrontStagePage();

    activateDesignMode();
    fireEvent.click(screen.getByRole('button', { name: '区块 cta' }));
    fireEvent.click(screen.getByRole('button', { name: '区块代码' }));
    const studio = await screen.findByRole('dialog', { name: 'JSX Studio' });
    fireEvent.click(
      within(studio).getByRole('button', { name: '运行预览' })
    );

    expect(within(studio).getByText('JS Block 试运行')).toBeInTheDocument();
  }, 10000);

  test('closes the whole JSX Studio when exiting design mode', async () => {
    authenticate();
    renderFrontStagePage();

    activateDesignMode();
    fireEvent.click(screen.getByRole('button', { name: '区块 cta' }));
    fireEvent.click(screen.getByRole('button', { name: '区块代码' }));
    const studio = await screen.findByRole('dialog', { name: 'JSX Studio' });
    fireEvent.click(
      within(studio).getByRole('button', { name: '运行预览' })
    );

    expect(within(studio).getByText('JS Block 试运行')).toBeInTheDocument();

    // Exit design mode — Drawer should close
    exitDesignMode();

    await waitFor(() => {
      expect(
        screen.queryByRole('dialog', { name: 'JSX Studio' })
      ).not.toBeInTheDocument();
    });
  }, 20_000);
});
