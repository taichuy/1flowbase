import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const nativeRuntime = vi.hoisted(() => ({
  evaluate: vi.fn(async (artifact: unknown, _registry?: unknown) => ({
    ok: true as const,
    artifact,
    component: () => null,
    diagnostics: [] as []
  }))
}));

vi.mock('@1flowbase/page-runtime', async (importOriginal) => {
  const actual =
    await importOriginal<typeof import('@1flowbase/page-runtime')>();
  return {
    ...actual,
    evaluateNativeReactComponentArtifactWithRegistry: nativeRuntime.evaluate
  };
});

import type {
  NativeReactComponentArtifact,
  NativeReactModuleRegistry,
  NativeReactResolvedModuleAsset
} from '@1flowbase/page-runtime';
import type { ConsoleFrontstageBlockNodeCode } from '@1flowbase/api-client';

import { useFrontstagePageCanvasNativePreparations } from '../../hooks/use-frontstage-page-canvas-native-preparations';
import type { FrontstagePageCanvasBlockCodeReadPlan } from '../../lib/page-canvas/runtime-source';

const SOURCE = 'export default function Block() { return null; }';

describe('useFrontstagePageCanvasNativePreparations', () => {
  beforeEach(() => nativeRuntime.evaluate.mockClear());

  test('I2005-AC-001 prepares a block without re-rendering its page owner', async () => {
    const ownerRender = vi.fn();
    const plan = readPlan();
    const artifact = createArtifact();
    const fetchSource = vi.fn(async () => ({
      block_id: 'block-1',
      page_id: 'page-1',
      source_code: SOURCE,
      source_sha256: null
    }));
    const artifactCache = {
      get: vi.fn(async (_identity: { module_policy_sha256: string }) => ({
        status: 'hit' as const,
        artifact,
        tier: 'l1' as const
      })),
      put: vi.fn(async () => ({ status: 'stored' as const, byteSize: 1 }))
    };
    const moduleRegistryFactory = (): NativeReactModuleRegistry => ({
      definitions: [],
      load: vi.fn(async () => ({})),
      resolveModuleMap: vi.fn(async () => ({})),
      resolveModuleAssets: vi.fn(async () => [])
    });
    const { result } = renderHook(() => {
      ownerRender();
      return useFrontstagePageCanvasNativePreparations({
        actorId: 'actor-1',
        actorWorkspaceId: 'workspace-1',
        readPlan: plan,
        fetchSource,
        artifactCache,
        moduleRegistryFactory
      });
    });
    await waitFor(() =>
      expect(
        result.current.preparations.getBlockSnapshot('block-1')
      ).toMatchObject({
        status: 'ready',
        prepared: { artifactCacheTier: 'l1' }
      })
    );
    expect(ownerRender).toHaveBeenCalledOnce();
  });

  test('AC-002 and AC-003 re-fetches and compiles only the refreshed block without reading its artifact cache', async () => {
    const artifact = createArtifact();
    const fetchSource = vi.fn(
      async (): Promise<ConsoleFrontstageBlockNodeCode> => ({
        block_id: 'block-1',
        page_id: 'page-1',
        source_code: SOURCE,
        source_sha256: null
      })
    );
    const compile = vi.fn(async (_input: unknown) => ({
      ok: true as const,
      artifact,
      diagnostics: [] as []
    }));
    const artifactCache = {
      get: vi.fn(async (_identity: { module_policy_sha256: string }) => ({
        status: 'hit' as const,
        artifact
      })),
      put: vi.fn(async () => new Promise<never>(() => {}))
    };
    const moduleRegistryFactory = (): NativeReactModuleRegistry => ({
      definitions: [],
      load: vi.fn(async () => ({})),
      resolveModuleMap: vi.fn(async () => ({})),
      resolveModuleAssets: vi.fn(async () => [])
    });
    const { result } = renderHook(() =>
      useFrontstagePageCanvasNativePreparations({
        actorId: 'actor-1',
        actorWorkspaceId: 'workspace-1',
        readPlan: readPlan(),
        fetchSource,
        compile,
        artifactCache,
        moduleRegistryFactory
      })
    );

    await waitFor(() =>
      expect(
        result.current.preparations.getBlockSnapshot('block-1')
      ).toMatchObject({ status: 'ready' })
    );
    expect(fetchSource).toHaveBeenCalledOnce();
    expect(compile).not.toHaveBeenCalled();
    expect(artifactCache.get).toHaveBeenCalledOnce();

    const before = result.current.preparations.getBlockSnapshot('block-1');
    result.current.refreshBlock('block-1');

    await waitFor(() => expect(fetchSource).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(compile).toHaveBeenCalledOnce());
    expect(artifactCache.get).toHaveBeenCalledOnce();
    await waitFor(() =>
      expect(
        result.current.preparations.getBlockSnapshot('block-1')
      ).toMatchObject({
        status: 'ready',
        generation: 1
      })
    );
    expect(nativeRuntime.evaluate).toHaveBeenCalledTimes(2);
    expect(nativeRuntime.evaluate.mock.calls[1][1]).not.toBe(
      nativeRuntime.evaluate.mock.calls[0][1]
    );
    const refreshed = result.current.preparations.getBlockSnapshot('block-1');
    if (before?.status !== 'ready' || refreshed?.status !== 'ready')
      throw new Error('Expected ready components');
    expect(refreshed.prepared.component).not.toBe(before.prepared.component);
    expect(compile).toHaveBeenCalledWith(
      expect.objectContaining({ signal: expect.any(AbortSignal) })
    );
    expect(compile.mock.calls[0][0]).not.toHaveProperty('flightKey');
  });

  test('a module policy change invalidates a ready preparation and its artifact identity', async () => {
    const artifact = createArtifact();
    const plan = readPlan();
    const fetchSource = vi.fn(async () => ({
      block_id: 'block-1',
      page_id: 'page-1',
      source_code: SOURCE,
      source_sha256: null
    }));
    const artifactCache = {
      get: vi.fn(async (_identity: { module_policy_sha256: string }) => ({
        status: 'hit' as const,
        artifact
      })),
      put: vi.fn(async () => ({ status: 'stored' as const, byteSize: 1 }))
    };
    const factory =
      (moduleSource: string) => (): NativeReactModuleRegistry => ({
        definitions: [{ module_source: moduleSource, exports: ['default'] }],
        load: vi.fn(async () => ({})),
        resolveModuleMap: vi.fn(async () => ({})),
        resolveModuleAssets: vi.fn(async () => [])
      });
    const { result, rerender } = renderHook(
      ({ registryFactory }) =>
        useFrontstagePageCanvasNativePreparations({
          actorId: 'actor-1',
          actorWorkspaceId: 'workspace-1',
          readPlan: plan,
          fetchSource,
          artifactCache,
          moduleRegistryFactory: registryFactory
        }),
      { initialProps: { registryFactory: factory('first-policy') } }
    );
    await waitFor(() =>
      expect(
        result.current.preparations.getBlockSnapshot('block-1')
      ).toMatchObject({ status: 'ready' })
    );
    rerender({ registryFactory: factory('second-policy') });
    await waitFor(() => expect(artifactCache.get).toHaveBeenCalledTimes(2));
    await waitFor(() =>
      expect(
        result.current.preparations.getBlockSnapshot('block-1')
      ).toMatchObject({ status: 'ready', generation: 1 })
    );
    expect(artifactCache.get.mock.calls[0][0].module_policy_sha256).not.toBe(
      artifactCache.get.mock.calls[1][0].module_policy_sha256
    );
    expect(nativeRuntime.evaluate).toHaveBeenCalledTimes(2);
  });

  test('I1989-AC-static-style keeps the component and assets in one shared artifact flight', async () => {
    const artifact = createArtifact(['antd-style']);
    const asset: NativeReactResolvedModuleAsset = {
      module_source: 'antd-style',
      role: 'shadow_style',
      media_type: 'text/css',
      sha256: 'a'.repeat(64),
      url: 'frontend-module-style:static',
      bytes: new TextEncoder().encode('.css-static{color:#123456}').buffer
    };
    const resolveModuleAssets = vi.fn(async () => [asset]);
    const moduleRegistryFactory = vi.fn(
      (): NativeReactModuleRegistry => ({
        definitions: [],
        load: vi.fn(async () => ({})),
        resolveModuleMap: vi.fn(async () => ({})),
        resolveModuleAssets
      })
    );
    const artifactCache = {
      get: vi.fn(async (_identity: { module_policy_sha256: string }) => ({
        status: 'hit' as const,
        artifact
      })),
      put: vi.fn(async () => ({ status: 'stored' as const, byteSize: 1 }))
    };

    const { result } = renderHook(() =>
      useFrontstagePageCanvasNativePreparations({
        actorId: 'actor-1',
        actorWorkspaceId: 'workspace-1',
        readPlan: readPlan(2),
        maxConcurrent: 2,
        fetchSource: vi.fn(async (request) => ({
          block_id: request.blockId,
          page_id: request.pageId,
          source_code: SOURCE,
          source_sha256: null
        })),
        artifactCache,
        moduleRegistryFactory
      })
    );

    await waitFor(() =>
      expect(
        result.current.preparations.getBlockSnapshot('block-2')
      ).not.toBeNull()
    );
    await waitFor(() =>
      expect(
        ['block-1', 'block-2']
          .map((id) => result.current.preparations.getBlockSnapshot(id))
          .every((preparation) => preparation?.status === 'ready')
      ).toBe(true)
    );
    expect(nativeRuntime.evaluate).toHaveBeenCalledTimes(1);
    expect(moduleRegistryFactory).toHaveBeenCalledTimes(2);
    expect(resolveModuleAssets).toHaveBeenCalledOnce();
    expect(resolveModuleAssets).toHaveBeenCalledWith(['antd-style']);
    expect(
      ['block-1', 'block-2'].map((id) =>
        result.current.preparations.getBlockSnapshot(id)
      )
    ).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          prepared: expect.objectContaining({ moduleAssets: [asset] })
        }),
        expect.objectContaining({
          prepared: expect.objectContaining({ moduleAssets: [asset] })
        })
      ])
    );
  });
});

function readPlan(count = 1): FrontstagePageCanvasBlockCodeReadPlan {
  return {
    workspaceId: 'workspace-1',
    pageId: 'page-1',
    requests: Array.from({ length: count }, (_, index) => {
      const sequence = index + 1;
      return {
        requestId: `request-${sequence}`,
        workspaceId: 'workspace-1',
        pageId: 'page-1',
        blockId: `block-${sequence}`,
        sourceBlockId: `block-${sequence}`,
        codeRef: 'code-1',
        sourceCodeRef: 'code-1',
        runtimeEntry: 'default',
        runtimeKind: 'native_react',
        order: 0,
        sourceIndex: 0,
        slotIndex: 0,
        installationId: null,
        providerCode: null,
        pluginId: null,
        pluginVersion: null,
        contributionCode: 'block'
      };
    })
  };
}

function createArtifact(
  moduleSources: string[] = []
): NativeReactComponentArtifact {
  return {
    identity: {
      source_sha256: 'source-sha',
      compiler_abi: 'compiler',
      runtime_abi: 'runtime'
    },
    program: {
      injectedModules: moduleSources.map((source) => ({ source }))
    }
  } as unknown as NativeReactComponentArtifact;
}
