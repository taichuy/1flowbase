import { renderHook, waitFor } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

const nativeRuntime = vi.hoisted(() => ({
  evaluate: vi.fn(async (artifact: unknown) => ({
    ok: true as const,
    artifact,
    component: () => null,
    diagnostics: [] as []
  }))
}));

vi.mock('@1flowbase/page-runtime', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@1flowbase/page-runtime')>();
  return {
    ...actual,
    evaluateNativeReactComponentArtifactWithRegistry: nativeRuntime.evaluate
  };
});

import type {
  NativeReactComponentArtifact,
  NativeReactModuleRegistry
} from '@1flowbase/page-runtime';
import type { ConsoleFrontstageBlockNodeCode } from '@1flowbase/api-client';

import { useFrontstagePageCanvasNativePreparations } from '../../hooks/use-frontstage-page-canvas-native-preparations';
import type { FrontstagePageCanvasBlockCodeReadPlan } from '../../lib/page-canvas/runtime-source';

const SOURCE = 'export default function Block() { return null; }';

describe('useFrontstagePageCanvasNativePreparations', () => {
  test('AC-002 and AC-003 re-fetches and compiles only the refreshed block without reading its artifact cache', async () => {
    const artifact = createArtifact();
    const fetchSource = vi.fn(async (): Promise<ConsoleFrontstageBlockNodeCode> => ({
      block_id: 'block-1',
      page_id: 'page-1',
      source_code: SOURCE,
      source_sha256: null
    }));
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

  test('AC-004 prepares URL imports for the unrestricted iframe without the local module registry', async () => {
    const fetchSource = vi.fn(async (): Promise<ConsoleFrontstageBlockNodeCode> => ({
      block_id: 'block-1',
      page_id: 'page-1',
      source_code:
        "import Widget from 'https://esm.sh/example-widget@1'; export default () => <Widget />;",
      source_sha256: 'external-source-sha'
    }));
    const compile = vi.fn();
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
        moduleRegistryFactory
      })
    );

    await waitFor(() =>
      expect(result.current.preparations[0]).toMatchObject({
        status: 'ready',
        prepared: {
          source:
            "import Widget from 'https://esm.sh/example-widget@1'; export default () => <Widget />;"
        }
      })
    );
    expect(compile).not.toHaveBeenCalled();
  });
});

function readPlan(): FrontstagePageCanvasBlockCodeReadPlan {
  return {
    workspaceId: 'workspace-1',
    pageId: 'page-1',
    requests: [
      {
        requestId: 'request-1',
        workspaceId: 'workspace-1',
        pageId: 'page-1',
        blockId: 'block-1',
        sourceBlockId: 'block-1',
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
      }
    ]
  };
}

function createArtifact(): NativeReactComponentArtifact {
  return {
    identity: {
      source_sha256: 'source-sha',
      compiler_abi: 'compiler',
      runtime_abi: 'runtime'
    },
    program: { injectedModules: [] }
  } as unknown as NativeReactComponentArtifact;
}
