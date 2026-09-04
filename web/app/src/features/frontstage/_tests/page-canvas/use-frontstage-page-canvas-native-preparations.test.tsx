import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const nativeRuntime = vi.hoisted(() => ({
  evaluate: vi.fn(async (artifact: unknown) => ({
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
    const compile = vi.fn(async () => ({
      ok: true as const,
      artifact,
      diagnostics: [] as []
    }));
    const artifactCache = {
      get: vi.fn(async () => ({ status: 'hit' as const, artifact })),
      put: vi.fn(async () => ({ status: 'stored' as const, byteSize: 1 }))
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
      expect(result.current.preparations[0]).toMatchObject({ status: 'ready' })
    );
    expect(fetchSource).toHaveBeenCalledOnce();
    expect(compile).not.toHaveBeenCalled();
    expect(artifactCache.get).toHaveBeenCalledOnce();

    result.current.refreshBlock('block-1');

    await waitFor(() => expect(fetchSource).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(compile).toHaveBeenCalledOnce());
    expect(artifactCache.get).toHaveBeenCalledOnce();
    await waitFor(() =>
      expect(result.current.preparations[0]).toMatchObject({
        status: 'ready',
        generation: 1
      })
    );
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
      get: vi.fn(async () => ({ status: 'hit' as const, artifact })),
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

    await waitFor(() => expect(result.current.preparations).toHaveLength(2));
    await waitFor(() =>
      expect(
        result.current.preparations.every(
          (preparation) => preparation.status === 'ready'
        )
      ).toBe(true)
    );
    expect(nativeRuntime.evaluate).toHaveBeenCalledTimes(1);
    expect(moduleRegistryFactory).toHaveBeenCalledTimes(1);
    expect(resolveModuleAssets).toHaveBeenCalledOnce();
    expect(resolveModuleAssets).toHaveBeenCalledWith(['antd-style']);
    expect(result.current.preparations).toEqual(
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
