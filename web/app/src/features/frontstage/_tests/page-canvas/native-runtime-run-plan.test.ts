import { describe, expect, test } from 'vitest';

import { createFrontstagePageCanvasNativePreparationPlanState } from '../../lib/page-canvas/runtime-run-plan';
import type { FrontstagePageCanvasRuntimeSourceState } from '../../lib/page-canvas/runtime-source';

describe('Frontstage Native React preparation plan', () => {
  test('D3-AC-002 carries Native dependency-lock inputs without Restricted one-shot phases', () => {
    const sourceState = {
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      sources: [
        {
          status: 'ready',
          blockId: 'block-1',
          slotIndex: 0,
          codeRef: 'code-1',
          source_sha256: 'a'.repeat(64),
          code: 'export default function Block() { return null; }'
        },
        {
          status: 'loading',
          blockId: 'block-2',
          slotIndex: 1,
          codeRef: 'code-2'
        }
      ]
    } as FrontstagePageCanvasRuntimeSourceState;

    const state = createFrontstagePageCanvasNativePreparationPlanState({
      sourceState,
      dependencyLocksByBlockId: { 'block-1': [] }
    });

    expect(state.items).toEqual([
      expect.objectContaining({
        status: 'native_plan_ready',
        blockId: 'block-1',
        sourceSha256: 'a'.repeat(64),
        dependencyLock: [],
        dependencyLockIdentity: expect.any(String)
      }),
      expect.objectContaining({
        status: 'source_not_ready',
        blockId: 'block-2',
        sourceStatus: 'loading'
      })
    ]);
    expect(JSON.stringify(state)).not.toMatch(
      /RestrictedBlockRunPlan|waiting_effect|schema_validate|action/
    );
  });

  test('D3-AC-002 keeps dependency identity block-scoped for the same source and different catalog versions', () => {
    const source = (blockId: string, slotIndex: number) => ({
      status: 'ready' as const,
      blockId,
      slotIndex,
      codeRef: 'shared-code',
      source_sha256: 'a'.repeat(64),
      code: 'export default function Block() { return null; }'
    });
    const moduleLock = (version: string, digest: string) => [
      {
        module_source: '@example/components',
        module_version: version,
        browser_asset: { sha256: digest, url: `/assets/${digest}.js` },
        exports: ['Widget']
      }
    ];
    const state = createFrontstagePageCanvasNativePreparationPlanState({
      sourceState: {
        workspaceId: 'workspace-1',
        pageId: 'page-1',
        sources: [source('block-v1', 0), source('block-v2', 1)]
      } as FrontstagePageCanvasRuntimeSourceState,
      dependencyLocksByBlockId: {
        'block-v1': moduleLock('1.0.0', '1'.repeat(64)),
        'block-v2': moduleLock('2.0.0', '2'.repeat(64))
      }
    });
    const identities = state.items.map((item) =>
      item.status === 'native_plan_ready' ? item.dependencyLockIdentity : null
    );
    expect(identities[0]).not.toBe(identities[1]);
  });
});
