import { useEffect, useState } from 'react';
import { createNativePreparationSource } from './fixtures/native-preparation-source';
import { act, render, screen, waitFor, within } from '@testing-library/react';
import type { ConsoleFrontstageBlockNode } from '@1flowbase/api-client';
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
const nativePreparationsHook = vi.hoisted(() => ({
  useFrontstagePageCanvasNativePreparations: vi.fn()
}));
const isolatedPreparationsHook = vi.hoisted(() => ({
  useFrontstagePageCanvasIsolatedPreparations: vi.fn()
}));

vi.mock(
  '../../hooks/use-frontstage-page-content-save',
  () => pageContentSaveHook
);
vi.mock('../../hooks/use-frontstage-block-catalog', () => blockCatalogHook);
vi.mock(
  '../../hooks/use-frontstage-page-canvas-native-preparations',
  () => nativePreparationsHook
);
vi.mock(
  '../../hooks/use-frontstage-page-canvas-isolated-preparations',
  () => isolatedPreparationsHook
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
      payload: {}
    }
  };
}

function createRootNode(
  runtimeKind: 'native_react' | 'isolated_iframe' = 'native_react',
  catalogMatched = false
): ConsoleFrontstageBlockNode {
  return {
    block_id: 'hero',
    workspace_id: 'workspace-1',
    page_id: 'page-1',
    tab_id: 'tab-1',
    parent_block_id: null,
    rank: '001000',
    presentation: 'page',
    title: 'Hero',
    description: null,
    schema_version: 1,
    code_ref: 'hero-code',
    input_mapping: {},
    output_mapping: {},
    runtime_descriptor: {
      id: 'hero',
      rendererVersion: 'v1',
      codeRef: 'hero-code',
      ...(catalogMatched
        ? {
            catalog: {
              providerCode: 'official',
              installationId: 'installation-1'
            },
            contribution: {
              pluginId: 'official.blocks',
              pluginVersion: '1.0.0',
              code: 'hero'
            }
          }
        : { contributionCode: 'official.hero' }),
      runtime: {
        kind: runtimeKind,
        entry:
          runtimeKind === 'native_react'
            ? 'blocks/hero.js'
            : '@official/isolated-hero'
      },
      layout: { order: 0, region: 'main' }
    },
    created_at: '2026-08-16T00:00:00Z',
    updated_at: '2026-08-16T00:00:00Z'
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
      preparations: createNativePreparationSource(preparations),
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
    mockNativePreparations();
    isolatedPreparationsHook.useFrontstagePageCanvasIsolatedPreparations.mockReturnValue(
      { preparations: [], errorsByBlockId: {} }
    );
  });

  test('passes the active page read plan to Native preparation and shows local loading', async () => {
    render(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          pageId="page-1"
          initialPageTree={[{ id: 'page-1', title: 'Landing', kind: 'page' }]}
          pageContent={createPageContent()}
          blockRoots={[createRootNode()]}
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
    expect(blockSlot).toHaveAttribute(
      'data-flowbase-frontstage-intrinsic-height',
      '320'
    );
  });

  test('preserves pre-drawn 320px grid slot height while loading shell is mounted even if ResizeObserver fires small skeleton height', async () => {
    const OriginalResizeObserver = globalThis.ResizeObserver;
    const intrinsicObservers: Array<{
      cb: ResizeObserverCallback;
      el: Element;
    }> = [];
    globalThis.ResizeObserver = class WrappingResizeObserver {
      private inner: ResizeObserver;
      private cb: ResizeObserverCallback;
      constructor(cb: ResizeObserverCallback) {
        this.cb = cb;
        this.inner = new OriginalResizeObserver(cb);
      }
      observe(el: Element, options?: ResizeObserverOptions) {
        if (
          el instanceof HTMLElement &&
          el.hasAttribute('data-flowbase-frontstage-intrinsic-content')
        ) {
          intrinsicObservers.push({ cb: this.cb, el });
          return;
        }
        this.inner.observe(el, options);
      }
      unobserve(el: Element) {
        this.inner.unobserve(el);
      }
      disconnect() {
        for (let i = intrinsicObservers.length - 1; i >= 0; i -= 1) {
          if (intrinsicObservers[i]?.cb === this.cb) {
            intrinsicObservers.splice(i, 1);
          }
        }
        this.inner.disconnect();
      }
    } as unknown as typeof ResizeObserver;

    try {
      mockNativePreparations([
        {
          status: 'compile',
          blockId: 'hero',
          slotIndex: 0,
          priority: 1,
          generation: 1
        }
      ]);

      await act(async () => {
        render(
          <AppProviders>
            <FrontStagePage
              workspaceId="workspace-1"
              pageId="page-1"
              initialPageTree={[{ id: 'page-1', title: 'Landing', kind: 'page' }]}
              pageContent={createPageContent()}
              blockRoots={[
                createRootNode(),
                { ...createRootNode(), block_id: 'secondary', rank: '002000' }
              ]}
            />
          </AppProviders>
        );
        await vi.dynamicImportSettled();
      });

      const blockSlot = await screen.findByTestId('block-slot-hero');
      await act(async () => {
        for (const { cb, el } of intrinsicObservers) {
          cb(
            [
              {
                target: el,
                contentRect: { height: 64, width: 800 }
              } as unknown as ResizeObserverEntry
            ],
            {} as ResizeObserver
          );
        }
        await new Promise((resolve) => setTimeout(resolve, 350));
      });
      expect(blockSlot).toHaveAttribute(
        'data-flowbase-frontstage-intrinsic-height',
        '320'
      );
    } finally {
      globalThis.ResizeObserver = OriginalResizeObserver;
    }
  });

  test('monotonic height ratchet: ready block with pending initial ctx.api request never shrinks below 320px, then settles once API finishes', async () => {
    const OriginalResizeObserver = globalThis.ResizeObserver;
    const intrinsicObservers: Array<{
      cb: ResizeObserverCallback;
      el: Element;
    }> = [];
    globalThis.ResizeObserver = class RatchetResizeObserver {
      private inner: ResizeObserver;
      private cb: ResizeObserverCallback;
      constructor(cb: ResizeObserverCallback) {
        this.cb = cb;
        this.inner = new OriginalResizeObserver(cb);
      }
      observe(el: Element, options?: ResizeObserverOptions) {
        if (
          el instanceof HTMLElement &&
          el.hasAttribute('data-flowbase-frontstage-intrinsic-content')
        ) {
          intrinsicObservers.push({ cb: this.cb, el });
          return;
        }
        this.inner.observe(el, options);
      }
      unobserve(el: Element) {
        this.inner.unobserve(el);
      }
      disconnect() {
        for (let i = intrinsicObservers.length - 1; i >= 0; i -= 1) {
          if (intrinsicObservers[i]?.cb === this.cb) {
            intrinsicObservers.splice(i, 1);
          }
        }
        this.inner.disconnect();
      }
    } as unknown as typeof ResizeObserver;

    let resolveBlockApi!: (val: unknown) => void;
    const apiPromise = new Promise((resolve) => {
      resolveBlockApi = resolve;
    });

    try {
      vi.spyOn(window, 'requestAnimationFrame').mockImplementation((cb) =>
        window.setTimeout(() => cb(performance.now()), 16)
      );
      const capabilityModule = await import(
        '../../lib/js-block-capability-handlers'
      );
      vi.spyOn(
        capabilityModule,
        'createFrontstageJsBlockCapabilityHandlers'
      ).mockReturnValue({
        interface: async () => apiPromise,
        disposeRequest: vi.fn(),
        prepareDraftRun: async () => undefined,
        revokeDraftRun: vi.fn()
      });

      // A remount can retire an instance while its initial request is pending.
      const requests: Promise<{ active: boolean; error: unknown } | null>[] = [];
      const ApiSpinThenCardBlock = ({
        ctx
      }: {
        ctx: { api: { get: (path: string) => Promise<unknown> } };
      }) => {
        const [done, setDone] = useState(false);
        useEffect(() => {
          let active = true;
          requests.push(
            ctx.api.get('/api/console/demo').then(
              () => {
                if (active) setDone(true);
                return null;
              },
              (error: unknown) => ({ active, error })
            )
          );
          return () => {
            active = false;
          };
        }, [ctx.api]);
        return <div data-testid="ratchet-block">{done ? 'settled-card' : 'tiny-spin'}</div>;
      };

      mockNativePreparations([
        {
          status: 'ready',
          blockId: 'hero',
          slotIndex: 0,
          priority: 1,
          generation: 1,
          mountIntent: {
            blockId: 'hero',
            slotIndex: 0,
            identityInput: {
              sourceSha256: 'b'.repeat(64),
              compilerAbi: 'compiler-a',
              runtimeAbi: 'runtime-a'
            }
          },
          prepared: {
            artifact: {} as never,
            component: ApiSpinThenCardBlock as never,
            identityInput: {
              sourceSha256: 'b'.repeat(64),
              compilerAbi: 'compiler-a',
              runtimeAbi: 'runtime-a'
            },
            artifactCacheTier: 'l1',
            moduleAssets: [],
            moduleSources: []
          }
        }
      ]);

      let view!: ReturnType<typeof render>;
      await act(async () => {
        view = render(
          <AppProviders>
            <FrontStagePage
              workspaceId="workspace-1"
              pageId="page-1"
              tabId="tab-1"
              initialPageTree={[{ id: 'page-1', title: 'Landing', kind: 'page' }]}
              pageContent={createPageContent()}
              blockRoots={[
                createRootNode(),
                { ...createRootNode(), block_id: 'secondary', rank: '002000' }
              ]}
            />
          </AppProviders>
        );
        await vi.dynamicImportSettled();
      });

      const blockSlot = await screen.findByTestId('block-slot-hero');
      const nativeRoot = await screen.findByTestId(
        'frontstage-native-block-root-hero'
      );
      await waitFor(() => {
        expect(nativeRoot.shadowRoot).not.toBeNull();
      });
      await waitFor(
        () => {
          expect(
            within(nativeRoot.shadowRoot as unknown as HTMLElement).getByTestId(
              'ratchet-block'
            )
          ).toHaveTextContent('tiny-spin');
        },
        { timeout: 8000 }
      );
      await waitFor(() => {
        expect(intrinsicObservers.length).toBeGreaterThan(0);
      });

      // Fire 48px measurement while initial ctx.api.get is still pending: must NOT shrink below 320px!
      await act(async () => {
        for (const { cb, el } of intrinsicObservers) {
          cb(
            [
              {
                target: el,
                contentRect: { height: 48, width: 800 }
              } as unknown as ResizeObserverEntry
            ],
            {} as ResizeObserver
          );
        }
        await new Promise((resolve) => setTimeout(resolve, 450));
      });
      expect(blockSlot).toHaveAttribute(
        'data-flowbase-frontstage-intrinsic-height',
        '320'
      );

      // Settle initial API call, then fire 230px measurement: now allowed to settle cleanly!
      await act(async () => {
        resolveBlockApi({ ok: true });
        await apiPromise;
        // Let the runtime's zero-delay settle timer publish the new layout phase.
        await new Promise((resolve) => requestAnimationFrame(resolve));
      });
      await waitFor(() => {
        expect(nativeRoot.shadowRoot?.textContent).toContain('settled-card');
      });
      await act(async () => {
        for (const { cb, el } of intrinsicObservers) {
          cb(
            [
              {
                target: el,
                contentRect: { height: 230, width: 800 }
              } as unknown as ResizeObserverEntry
            ],
            {} as ResizeObserver
          );
        }
        await new Promise((resolve) => requestAnimationFrame(resolve));
      });
      await waitFor(
        () => {
          const h = Number(
            blockSlot.getAttribute('data-flowbase-frontstage-intrinsic-height')
          );
          expect(h >= 225 && h < 260).toBe(true);
        },
        { timeout: 2000 }
      );
      view.unmount();
      for (const rejection of await Promise.all(requests)) {
        if (rejection !== null) {
          expect(rejection.active).toBe(false);
          expect(rejection.error).toEqual(
            new Error('Native Block capability belongs to a stale instance.')
          );
        }
      }
    } finally {
      globalThis.ResizeObserver = OriginalResizeObserver;
      vi.restoreAllMocks();
    }
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
            compilerAbi: 'compiler-a',
            runtimeAbi: 'runtime-a'
          }
        },
        prepared: {
          artifact: {} as never,
          component: () => <h1>FrontStage Runtime Snapshot</h1>,
          artifactCacheTier: 'l2',
          moduleAssets: [],
          moduleSources: [],
          identityInput: {
            sourceSha256: 'a'.repeat(64),
            compilerAbi: 'compiler-a',
            runtimeAbi: 'runtime-a'
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
          pageContent={createPageContent()}
          blockRoots={[createRootNode('native_react', true)]}
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
      ).toHaveBeenCalledWith(expect.any(Object));
    });
  });

  test('passes graph-backed isolated render requests into production preparation', async () => {
    const content = createPageContent();
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
          blockRoots={[createRootNode('isolated_iframe', true)]}
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
