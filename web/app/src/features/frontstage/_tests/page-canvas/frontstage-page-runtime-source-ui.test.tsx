import { render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { AppProviders } from '../../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import type { FrontstagePageContent } from '../../api/page-content';
import type { FrontstageNativePreparationSnapshot } from '../../lib/page-canvas/native-runtime-preparation';
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
const nativePreparationsHook = vi.hoisted(() => ({
  useFrontstagePageCanvasNativePreparations: vi.fn()
}));
const isolatedPreparationsHook = vi.hoisted(() => ({
  useFrontstagePageCanvasIsolatedPreparations: vi.fn()
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
  '../../hooks/use-frontstage-page-canvas-native-preparations',
  () => nativePreparationsHook
);
vi.mock(
  '../../hooks/use-frontstage-page-canvas-isolated-preparations',
  () => isolatedPreparationsHook
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
            renderer_version: 'v1',
            codeRef: 'hero-code',
            contributionCode: 'official.hero',
            runtime: { kind: 'native_react', entry: 'blocks/hero.js' },
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
            renderer_version: 'v1',
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
            runtime: { kind: 'native_react', entry: 'blocks/hero.js' },
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
    runtimeKind: 'native_react',
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

function mockNativePreparations(
  preparations: FrontstageNativePreparationSnapshot[] = []
) {
  nativePreparationsHook.useFrontstagePageCanvasNativePreparations.mockReturnValue(
    {
      preparations,
      retryBlock: vi.fn()
    }
  );
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
    mockNativePreparations();
    isolatedPreparationsHook.useFrontstagePageCanvasIsolatedPreparations.mockReturnValue(
      { preparations: [], errorsByBlockId: {} }
    );
    blockCodeApi.fetchFrontstageBlockCode.mockResolvedValue({
      pageId: 'page-1',
      codeRef: 'hero-code',
      code: 'export default { render() {} }',
      source_sha256: 'hero-source-sha256'
    });
  });

  test('passes the active page read plan to Native preparation and shows local loading', async () => {
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
      expect(
        nativePreparationsHook.useFrontstagePageCanvasNativePreparations
      ).toHaveBeenCalledWith(
        expect.objectContaining({
          readPlan: expect.objectContaining({
            workspaceId: 'workspace-1',
            pageId: 'page-1',
            requests: [expect.objectContaining({ codeRef: 'hero-code' })]
          })
        })
      );
    });

    expect(
      within(screen.getByTestId('page-canvas-render-slots')).getByTestId(
        'block-slot-hero'
      )
    ).toBeInTheDocument();
    const blockSlot = screen.getByTestId('block-slot-hero');
    expect(
      within(blockSlot).getByTestId('block-ui-loading-shell')
    ).toHaveAttribute('aria-busy', 'true');
    expect(blockSlot).not.toHaveTextContent('区块加载中...');
  });

  test('connects a ready Native preparation to the real PageCanvas Host surface', async () => {
    mockNativePreparations([
      {
        status: 'ready',
        blockId: 'hero',
        slotIndex: 0,
        priority: 1,
        generation: 0,
        mountIntent: {
          blockId: 'hero',
          slotIndex: 0,
          identityInput: {
            sourceSha256: 'a'.repeat(64),
            runtimeFingerprint: 'runtime-a',
            dependencyLockIdentity: 'lock-a'
          }
        },
        prepared: {
          artifact: {} as never,
          component: () => <h1>FrontStage Runtime Snapshot</h1>,
          artifactCacheTier: 'l2',
          moduleAssets: [],
          identityInput: {
            sourceSha256: 'a'.repeat(64),
            runtimeFingerprint: 'runtime-a',
            dependencyLockIdentity: 'lock-a'
          }
        }
      }
    ]);
    blockCatalogHook.useFrontstageBlockCatalog.mockReturnValue({
      items: [createCatalogEntry()],
      diagnostics: [],
      loading: false,
      error: null
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

    const nativeRoot = await screen.findByTestId(
      'frontstage-native-block-root-hero'
    );
    await waitFor(() => expect(nativeRoot.shadowRoot).not.toBeNull());
    expect(
      await within(nativeRoot.shadowRoot as unknown as HTMLElement).findByRole(
        'heading',
        {
          name: 'FrontStage Runtime Snapshot'
        }
      )
    ).toBeInTheDocument();

    await waitFor(() => {
      expect(
        nativePreparationsHook.useFrontstagePageCanvasNativePreparations
      ).toHaveBeenCalledWith(
        expect.objectContaining({
          externalNpm: undefined
        })
      );
    });
  });

  test('passes graph-backed isolated render requests into production preparation', async () => {
    const content = createCatalogMatchedPageContent();
    content.document.payload = {
      blocks: [
        {
          id: 'hero',
          renderer_version: 'v1',
          catalog: {
            providerCode: 'official',
            installationId: 'installation-1'
          },
          contribution: {
            pluginId: 'official.blocks',
            pluginVersion: '1.0.0',
            code: 'hero'
          },
          runtime: {
            kind: 'isolated_iframe',
            entry: '@official/isolated-hero'
          },
          layout: { order: 0, region: 'main' }
        }
      ]
    };
    const catalogEntry = createCatalogEntry();
    catalogEntry.runtimeKind = 'isolated_iframe';
    catalogEntry.raw = {
      runtime: 'isolated_iframe',
      runtime_kind: 'isolated'
    } as NormalizedFrontstageBlockCatalogEntry['raw'];
    blockCatalogHook.useFrontstageBlockCatalog.mockReturnValue({
      items: [catalogEntry],
      diagnostics: [],
      loading: false,
      error: null,
      isSuccess: true
    });

    render(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          pageId="page-1"
          initialPageTree={[{ id: 'page-1', title: 'Landing', kind: 'page' }]}
          pageContent={content}
        />
      </AppProviders>
    );

    await waitFor(() =>
      expect(
        isolatedPreparationsHook.useFrontstagePageCanvasIsolatedPreparations
      ).toHaveBeenCalledWith(
        expect.objectContaining({
          actorId: 'actor-1',
          actorWorkspaceId: 'workspace-1',
          workspaceId: 'workspace-1',
          catalogEntries: [catalogEntry],
          renderPlan: expect.objectContaining({
            items: [
              expect.objectContaining({
                blockId: 'hero',
                renderMode: 'isolated_iframe',
                canMountIsolatedIframe: true
              })
            ]
          })
        })
      )
    );
  });
});
