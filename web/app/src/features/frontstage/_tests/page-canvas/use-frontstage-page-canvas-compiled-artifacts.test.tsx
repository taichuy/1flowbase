import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';
import type { CompiledBlockArtifact } from '@1flowbase/page-runtime';

import { useFrontstagePageCanvasCompiledArtifacts } from '../../hooks/use-frontstage-page-canvas-compiled-artifacts';
import type { FrontstagePageCanvasRuntimeSourceState } from '../../lib/page-canvas/runtime-source';

const sourceSha256 = 'a'.repeat(64);
const runtimeFingerprint = 'runtime-a';

function artifact(): CompiledBlockArtifact {
  return {
    format: '1flowbase/js-block-compiled-artifact',
    version: 1,
    runtimeFingerprint,
    sourceSha256,
    program: {
      injectedModules: [],
      importBindings: [],
      executableBody: 'return { main: async () => ({ view: {}, outputs: {} }) };',
      executablePreambleLines: 0,
      moduleMapIdentifier: '__modules',
      defaultExportIdentifier: '__default'
    },
    manifest: { allowedImports: [] }
  };
}

function sourceState(): FrontstagePageCanvasRuntimeSourceState {
  return {
    workspaceId: 'workspace-a',
    pageId: 'page-a',
    sources: [
      {
        status: 'ready',
        blockId: 'block-a',
        sourceBlockId: 'block-a',
        codeRef: 'code-a',
        sourceCodeRef: 'code-a',
        order: 0,
        sourceIndex: 0,
        slotIndex: 0,
        renderMode: 'restricted_js_block',
        canEnterRestrictedJsRuntime: true,
        runtimeKind: 'worker',
        runtimeEntry: 'worker.js',
        contributionCode: 'block.a',
        code: 'export default { main };',
        source_sha256: sourceSha256,
        block: {} as never,
        request: {} as never
      }
    ]
  };
}

describe('Frontstage compiled artifact lookup', () => {
  test('AC-023 keeps a ready source pending until L2 lookup settles, then exposes a hit', async () => {
    let resolveLookup!: (value: { status: 'hit'; artifact: CompiledBlockArtifact }) => void;
    const get = vi.fn(
      () =>
        new Promise<{ status: 'hit'; artifact: CompiledBlockArtifact }>(
          (resolve) => {
            resolveLookup = resolve;
          }
        )
    );
    const { result } = renderHook(() =>
      useFrontstagePageCanvasCompiledArtifacts({
        actorId: 'actor-a',
        workspaceId: 'workspace-a',
        sourceState: sourceState(),
        artifactCache: { get },
        runtimeFingerprint
      })
    );
    expect(result.current.sourceState?.sources[0]).toMatchObject({
      status: 'ready',
      artifactLookupStatus: 'pending'
    });
    act(() => resolveLookup({ status: 'hit', artifact: artifact() }));
    await waitFor(() =>
      expect(result.current.sourceState?.sources[0]).toMatchObject({
        artifactLookupStatus: 'hit',
        compiledArtifact: artifact()
      })
    );
    expect(get).toHaveBeenCalledWith({
      actorId: 'actor-a',
      workspaceId: 'workspace-a',
      runtimeFingerprint,
      sourceSha256
    });
  });

  test.each(['miss', 'unavailable'] as const)(
    'AC-023 releases %s lookups to the source cold path',
    async (status) => {
      const get = vi.fn(async () =>
        status === 'miss'
          ? ({ status, reason: 'not_found' } as const)
          : ({ status, reason: 'read_failed' } as const)
      );
      const { result } = renderHook(() =>
        useFrontstagePageCanvasCompiledArtifacts({
          actorId: 'actor-a',
          workspaceId: 'workspace-a',
          sourceState: sourceState(),
          artifactCache: { get },
          runtimeFingerprint
        })
      );
      await waitFor(() =>
        expect(result.current.sourceState?.sources[0]).toMatchObject({
          artifactLookupStatus: status,
          compiledArtifact: undefined
        })
      );
    }
  );

  test('AC-023 reads only demand-eligible blocks', () => {
    const get = vi.fn();
    const { result } = renderHook(() =>
      useFrontstagePageCanvasCompiledArtifacts({
        actorId: 'actor-a',
        workspaceId: 'workspace-a',
        sourceState: sourceState(),
        demandsByBlockId: { 'block-a': 3 },
        artifactCache: { get },
        runtimeFingerprint
      })
    );
    expect(get).not.toHaveBeenCalled();
    expect(result.current.sourceState?.sources[0]).not.toHaveProperty(
      'artifactLookupStatus'
    );
  });
});
