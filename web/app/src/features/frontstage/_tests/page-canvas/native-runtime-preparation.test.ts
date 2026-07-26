import { describe, expect, test, vi } from 'vitest';

import {
  FrontstageNativePreparationScheduler,
  FrontstagePageNativeModuleRegistryCache,
  type FrontstageNativePreparedRuntime,
  type FrontstageNativePreparationTask
} from '../../lib/page-canvas/native-runtime-preparation';
import {
  createNativeReactModuleRegistry,
  sha256Text,
  type NativeReactCatalogDependencyLock
} from '@1flowbase/page-runtime';
import {
  createFrontstageRuntimeDemandCandidates,
  resolveFrontstageRuntimePreparationKind
} from '../../lib/page-canvas/runtime-demand';
import { frontstageRuntimeSourceMatchesDigest } from '../../lib/page-canvas/runtime-source';

describe('Frontstage Native React preparation demand', () => {
  test('D3-AC-001 keeps 0/1 mount intent, 2 preload, 3 dormant and stable priority/slot order', () => {
    const values = [
      { blockId: 'dormant', slotIndex: 0 },
      { blockId: 'near-b', slotIndex: 3 },
      { blockId: 'visible', slotIndex: 2 },
      { blockId: 'selected', slotIndex: 9 },
      { blockId: 'near-a', slotIndex: 1 }
    ];
    const demands = {
      dormant: 3,
      'near-b': 2,
      visible: 1,
      selected: 0,
      'near-a': 2
    } as const;

    expect(
      createFrontstageRuntimeDemandCandidates(values, demands).map(
        ({ blockId, preparationKind }) => [blockId, preparationKind]
      )
    ).toEqual([
      ['selected', 'prepare_and_mount_intent'],
      ['visible', 'prepare_and_mount_intent'],
      ['near-a', 'preload'],
      ['near-b', 'preload']
    ]);
    expect(resolveFrontstageRuntimePreparationKind(3)).toBe('dormant');
  });

  test('D3-AC-002 rejects a backend digest that does not match source bytes', () => {
    expect(
      frontstageRuntimeSourceMatchesDigest(
        'export default function Block() { return null; }',
        '0'.repeat(64)
      )
    ).toBe(false);
  });
});

describe('FrontstagePageNativeModuleRegistryCache', () => {
  test('D3-AC-004 fetches a shared module once for multiple blocks with the same page lock', async () => {
    const cache = new FrontstagePageNativeModuleRegistryCache();
    const source = 'export const Widget = 1;';
    const fetchAsset = vi.fn(async () => new Response(source, { status: 200 }));
    const createRegistry = vi.fn((lock: NativeReactCatalogDependencyLock) =>
      createNativeReactModuleRegistry({
        dependencyLock: lock,
        hostModules: {},
        fetchAsset
      })
    );
    const dependencyLock = [
      {
        module_source: '@example/components',
        module_version: '1.0.0',
        browser_asset: { sha256: sha256Text(source), url: '/module.js' },
        exports: ['Widget']
      }
    ];

    const firstRegistry = cache.get(dependencyLock, createRegistry);
    const secondRegistry = cache.get([...dependencyLock], createRegistry);
    expect(firstRegistry).toBe(secondRegistry);
    await Promise.all([
      firstRegistry.load('@example/components'),
      secondRegistry.load('@example/components')
    ]);
    expect(createRegistry).toHaveBeenCalledOnce();
    expect(fetchAsset).toHaveBeenCalledOnce();
  });
});

describe('FrontstageNativePreparationScheduler', () => {
  test('D3-AC-001 bounds concurrency, pauses new work while hidden, and preserves ready preparations', async () => {
    const scheduler = new FrontstageNativePreparationScheduler(2);
    const flights = [deferred(), deferred(), deferred()];
    const starts: string[] = [];
    const tasks = ['selected', 'visible', 'near'].map((blockId, slotIndex) =>
      task(blockId, slotIndex, async () => {
        starts.push(blockId);
        return flights[slotIndex].promise;
      })
    );
    scheduler.reconcile(tasks, { selected: 0, visible: 1, near: 2 });
    expect(starts).toEqual(['selected', 'visible']);

    scheduler.setPageVisible(false);
    flights[0].resolve(prepared('selected'));
    await tick();
    expect(starts).toEqual(['selected', 'visible']);
    expect(
      scheduler.getSnapshots().find(({ blockId }) => blockId === 'selected')
    ).toMatchObject({ status: 'ready', mountIntent: { blockId: 'selected' } });

    scheduler.setPageVisible(true);
    expect(starts).toEqual(['selected', 'visible', 'near']);
    flights[1].resolve(prepared('visible'));
    flights[2].resolve(prepared('near'));
    await tick();
    expect(
      scheduler.getSnapshots().find(({ blockId }) => blockId === 'near')
    ).toMatchObject({ status: 'ready', mountIntent: null });
  });

  test('D3-AC-002 exposes compile/module failures and retries with a new generation', async () => {
    const scheduler = new FrontstageNativePreparationScheduler(1);
    let attempt = 0;
    const stages: string[] = [];
    scheduler.subscribe(() => {
      const status = scheduler.getSnapshots()[0]?.status;
      if (status) stages.push(status);
    });
    scheduler.reconcile(
      [
        task('block', 0, async (_signal, enterStage) => {
          enterStage(attempt++ === 0 ? 'compile' : 'module_resolve');
          if (attempt === 1) throw new Error('compile failed');
          return prepared('block');
        })
      ],
      { block: 0 }
    );
    await tick();
    expect(scheduler.getSnapshots()[0]).toMatchObject({
      status: 'failed',
      failedStage: 'compile',
      generation: 0
    });

    scheduler.retry('block');
    await tick();
    expect(scheduler.getSnapshots()[0]).toMatchObject({
      status: 'ready',
      generation: 1
    });
    expect(stages).toContain('module_resolve');
  });

  test('D3-AC-002 skips compile on an L2 hit and compiles exactly once on a miss', async () => {
    const scheduler = new FrontstageNativePreparationScheduler(2);
    const compile = vi.fn();
    scheduler.reconcile(
      [
        task('l2-hit', 0, async (_signal, enterStage) => {
          enterStage('artifact_lookup');
          enterStage('module_resolve');
          return { ...prepared('l2-hit'), artifactCacheTier: 'l2' };
        }),
        task('l2-miss', 1, async (_signal, enterStage) => {
          enterStage('artifact_lookup');
          enterStage('compile');
          compile();
          enterStage('module_resolve');
          return prepared('l2-miss');
        })
      ],
      { 'l2-hit': 0, 'l2-miss': 1 }
    );
    await tick();
    expect(compile).toHaveBeenCalledOnce();
    expect(scheduler.getSnapshots()).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          blockId: 'l2-hit',
          prepared: expect.objectContaining({ artifactCacheTier: 'l2' })
        }),
        expect.objectContaining({
          blockId: 'l2-miss',
          prepared: expect.objectContaining({ artifactCacheTier: 'miss' })
        })
      ])
    );
  });

  test('D3-AC-002 attributes source fetch errors to source_fetch', async () => {
    const scheduler = new FrontstageNativePreparationScheduler();
    scheduler.reconcile(
      [
        task('source-fail', 0, async () => Promise.reject(new Error('offline')))
      ],
      { 'source-fail': 1 }
    );
    await tick();
    expect(scheduler.getSnapshots()[0]).toMatchObject({
      status: 'failed',
      failedStage: 'source_fetch',
      error: { message: 'offline' }
    });
  });

  test('D3-AC-001 cancels dormant work and rejects stale generation completion', async () => {
    const scheduler = new FrontstageNativePreparationScheduler(1);
    const first = deferred<FrontstageNativePreparedRuntime>();
    const second = deferred<FrontstageNativePreparedRuntime>();
    scheduler.reconcile([task('block', 0, () => first.promise, 'v1')], {
      block: 0
    });
    scheduler.reconcile([task('block', 0, () => second.promise, 'v2')], {
      block: 0
    });
    first.resolve(prepared('stale'));
    await tick();
    expect(scheduler.getSnapshots()[0]).not.toMatchObject({ status: 'ready' });

    scheduler.reconcile([task('block', 0, () => second.promise, 'v2')], {
      block: 3
    });
    second.resolve(prepared('cancelled'));
    await tick();
    expect(scheduler.getSnapshots()[0]).toMatchObject({
      status: 'idle',
      priority: 3
    });
  });

  test('D3-AC-002 marks module_resolve failures without action/schema rerun phases', async () => {
    const scheduler = new FrontstageNativePreparationScheduler();
    scheduler.reconcile(
      [
        task('module-fail', 0, async (_signal, enterStage) => {
          enterStage('artifact_lookup');
          enterStage('module_resolve');
          throw new Error('module digest mismatch');
        })
      ],
      { 'module-fail': 1 }
    );
    await tick();
    expect(scheduler.getSnapshots()[0]).toMatchObject({
      status: 'failed',
      failedStage: 'module_resolve',
      error: { message: 'module digest mismatch' }
    });
    expect(
      scheduler
        .getSnapshots()
        .some(({ status }) =>
          ['waiting_effect', 'schema_validate'].includes(status)
        )
    ).toBe(false);
  });
});

function task(
  blockId: string,
  slotIndex: number,
  prepare: FrontstageNativePreparationTask['prepare'],
  identity = blockId
): FrontstageNativePreparationTask {
  return { blockId, slotIndex, identity, prepare: vi.fn(prepare) };
}

function prepared(blockId: string): FrontstageNativePreparedRuntime {
  return {
    artifact: {
      source_sha256: blockId
    } as unknown as FrontstageNativePreparedRuntime['artifact'],
    component: (() => null) as FrontstageNativePreparedRuntime['component'],
    artifactCacheTier: 'miss',
    identityInput: {
      sourceSha256: blockId,
      runtimeFingerprint: 'runtime',
      dependencyLockIdentity: 'lock'
    }
  };
}

function deferred<T = FrontstageNativePreparedRuntime>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => {
    resolve = next;
  });
  return { promise, resolve };
}

async function tick(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}
