import { render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { AppProviders } from '../../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import type { FrontstagePageContent } from '../../api/page-content';
import type {
  FrontstagePageCanvasRuntimeSessionEntry,
  UseFrontstagePageCanvasRuntimeSessionsResult
} from '../../hooks/use-frontstage-page-canvas-runtime-sessions';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import type { RestrictedBlockRuntimeHostSnapshot } from '../../lib/restricted-block-runtime-host';
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
  fetchFrontstageBlockCode: vi.fn(),
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
      permissions: []
    }
  });
}

function createPageContent(): FrontstagePageContent {
  return {
    page: {
      id: 'page-1',
      title: 'Landing',
      kind: 'page',
      parentId: null,
      rank: '001000',
      contentPresentation: 'single'
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
    document: {
      rootUid: 'root-1',
      payload: {
        blocks: [
          {
            id: 'hero',
            codeRef: 'hero-code',
            contributionCode: 'official.hero',
            runtime: { kind: 'iframe', entry: 'blocks/hero.js' },
            layout: { order: 0, region: 'main' }
          }
        ]
      }
    }
  };
}

function createCatalogMatchedPageContent(): FrontstagePageContent {
  return {
    page: {
      id: 'page-1',
      title: 'Landing',
      kind: 'page',
      parentId: null,
      rank: '001000',
      contentPresentation: 'single'
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
    document: {
      rootUid: 'root-1',
      payload: {
        blocks: [
          {
            id: 'hero',
            codeRef: 'hero-code',
            catalog: {
              providerCode: 'official',
              installationId: 'installation-1'
            },
            contribution: {
              pluginId: 'official.blocks',
              pluginVersion: '1.0.0',
              code: 'hero'
            },
            runtime: { kind: 'iframe', entry: 'blocks/hero.js' },
            layout: { order: 0, region: 'main' }
          }
        ]
      }
    }
  };
}

function createCatalogEntry(): NormalizedFrontstageBlockCatalogEntry {
  return {
    id: 'official:hero',
    runtimeKind: 'iframe',
    installationId: 'installation-1',
    providerCode: 'official',
    pluginId: 'official.blocks',
    pluginVersion: '1.0.0',
    contributionCode: 'hero',
    title: 'Hero',
    entry: 'blocks/hero.js',
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
    raw: {} as NormalizedFrontstageBlockCatalogEntry['raw']
  };
}

function createRuntimeSnapshot(
  overrides: Partial<RestrictedBlockRuntimeHostSnapshot> = {}
): RestrictedBlockRuntimeHostSnapshot {
  return {
    status: 'ready',
    requestId: 'restricted-block:hero:hero-code',
    blockId: 'hero',
    schemaValidationOptions: {
      maxDepth: 8,
      maxNodes: 250,
      allowedActions: [],
      allowedEvents: [],
      allowedDataPermissions: []
    },
    logs: [],
    effects: [],
    rejections: [],
    ...overrides
  };
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

describe('FrontStagePage PageCanvas runtime source UI', () => {
  beforeEach(() => {
    resetAuthStore();
    vi.clearAllMocks();
    authenticate();
    pageContentSaveHook.useFrontstagePageContentSave.mockReturnValue({
      save: vi.fn(),
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
    mockRuntimeSessions();
    blockCodeApi.fetchFrontstageBlockCode.mockResolvedValue({
      pageId: 'page-1',
      codeRef: 'hero-code',
      code: 'export default { render() {} }'
    });
  });

  test('queries block code for the active page render plan and shows ready status in canvas slots', async () => {
    render(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          pageId="page-1"
          initialPageTree={[{ id: 'page-1', title: 'Landing', kind: 'page' }]}
          pageContent={createPageContent()}
        />
      </AppProviders>
    );

    await waitFor(() => {
      expect(blockCodeApi.fetchFrontstageBlockCode).toHaveBeenCalledWith(
        'workspace-1',
        'page-1',
        'hero-code'
      );
    });

    expect(
      within(screen.getByTestId('page-canvas-render-slots')).getByTestId(
        'block-slot-hero'
      )
    ).toBeInTheDocument();
    // No runtime session yet, so shows loading placeholder
    expect(screen.getByTestId('block-slot-hero')).toHaveTextContent(
      '区块加载中...'
    );
  });

  test('connects mocked runtime session snapshots into the PageCanvas preview without creating real workers', async () => {
    const runtimeSessionEntries = [
      {
        status: 'ready',
        blockId: 'hero',
        sourceBlockId: 'hero',
        codeRef: 'hero-code',
        sourceCodeRef: 'hero-code',
        sourceIndex: 0,
        slotIndex: 0,
        runPlanStatus: 'run_plan_ready',
        snapshot: createRuntimeSnapshot({
          status: 'ready',
          schema: {
            primitive: 'Title',
            props: { children: 'FrontStage Runtime Snapshot' }
          }
        })
      }
    ] satisfies FrontstagePageCanvasRuntimeSessionEntry[];
    blockCatalogHook.useFrontstageBlockCatalog.mockReturnValue({
      items: [createCatalogEntry()],
      diagnostics: [],
      loading: false,
      error: null
    });
    mockRuntimeSessions({
      entries: runtimeSessionEntries,
      snapshotsBySlot: { 0: runtimeSessionEntries[0].snapshot }
    });

    render(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          pageId="page-1"
          initialPageTree={[{ id: 'page-1', title: 'Landing', kind: 'page' }]}
          pageContent={createCatalogMatchedPageContent()}
        />
      </AppProviders>
    );

    // No more "运行计划已就绪" text — canvas now shows actual block content instead
    expect(
      await screen.findByRole('heading', {
        name: 'FrontStage Runtime Snapshot'
      })
    ).toBeInTheDocument();

    await waitFor(() => {
      expect(
        runtimeSessionsHook.useFrontstagePageCanvasRuntimeSessions
      ).toHaveBeenCalledWith(
        expect.objectContaining({
          runtimeRunPlanState: expect.objectContaining({
            workspaceId: 'workspace-1',
            pageId: 'page-1',
            items: [
              expect.objectContaining({
                status: 'run_plan_ready',
                blockId: 'hero',
                codeRef: 'hero-code'
              })
            ]
          })
        })
      );
    });
  });
});
