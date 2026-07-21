import { IDBFactory } from 'fake-indexeddb';
import { describe, expect, test } from 'vitest';
import type { CompiledBlockArtifact } from '@1flowbase/page-runtime';

import {
  FrontstageCompiledArtifactCache,
  createFrontstageArtifactCacheKey,
  type FrontstageArtifactCacheIdentity,
  type FrontstageArtifactCacheRecord,
  type FrontstageArtifactCacheStore
} from '../../lib/runtime-cache/artifact-cache';
import { createIndexedDbArtifactCacheStore } from '../../lib/runtime-cache/indexeddb-store';

const runtimeFingerprint = 'runtime-fingerprint';
const sourceA = 'a'.repeat(64);
const sourceB = 'b'.repeat(64);
const sourceC = 'c'.repeat(64);

function identity(
  sourceSha256 = sourceA,
  overrides: Partial<FrontstageArtifactCacheIdentity> = {}
): FrontstageArtifactCacheIdentity {
  return {
    actorId: 'actor-a',
    workspaceId: 'workspace-a',
    runtimeFingerprint,
    sourceSha256,
    ...overrides
  };
}

function artifact(
  sourceSha256 = sourceA,
  overrides: Partial<CompiledBlockArtifact> = {}
): CompiledBlockArtifact {
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
    manifest: { allowedImports: [] },
    ...overrides
  };
}

function subject(name: string, byteBudget?: number, now = () => 10) {
  const store = createIndexedDbArtifactCacheStore({
    indexedDB: new IDBFactory(),
    databaseName: name
  });
  return {
    store,
    cache: new FrontstageCompiledArtifactCache({ store, byteBudget, now })
  };
}

describe('Frontstage compiled artifact IndexedDB cache', () => {
  test('AC-022 D5-003 round trips through real IndexedDB and isolates actor/workspace namespaces', async () => {
    const { cache } = subject('roundtrip');
    await expect(cache.put(identity(), artifact())).resolves.toMatchObject({
      status: 'stored'
    });
    await expect(cache.get(identity())).resolves.toEqual({
      status: 'hit',
      artifact: artifact()
    });
    await expect(
      cache.get(identity(sourceA, { actorId: 'actor-b' }))
    ).resolves.toEqual({ status: 'miss', reason: 'not_found' });
    await expect(
      cache.get(identity(sourceA, { workspaceId: 'workspace-b' }))
    ).resolves.toEqual({ status: 'miss', reason: 'not_found' });
  });

  test('AC-022 D5-004 persists only canonical artifact and management metadata', async () => {
    const { cache, store } = subject('canonical-record');
    const canary = {
      ...artifact(),
      token: 'secret-token',
      headers: { authorization: 'secret-header' },
      apiResponse: { secret: true },
      blockResult: { view: {}, outputs: { secret: true } },
      logs: ['secret-log'],
      effects: [{ payload: 'secret-effect' }],
      context: { currentUser: 'secret-user' },
      userState: { secret: true }
    };
    await cache.put(identity(), canary);
    const records = await store.list();
    expect(records).toHaveLength(1);
    expect(Object.keys(records[0] as object).sort()).toEqual([
      'actorId',
      'artifact',
      'byteSize',
      'key',
      'lastAccessedAt',
      'runtimeFingerprint',
      'schemaVersion',
      'sourceSha256',
      'workspaceId'
    ]);
    expect(JSON.stringify(records[0])).not.toMatch(
      /secret-token|secret-header|secret-log|secret-effect|secret-user|apiResponse|blockResult|userState/
    );
  });

  test('AC-023 D5-005 enforces deterministic byte LRU with stable-key ties and skips oversized records', async () => {
    const seeded = subject('lru-seed');
    await seeded.cache.put(identity(sourceA), artifact(sourceA));
    const first = (await seeded.store.list())[0] as FrontstageArtifactCacheRecord;
    const cache = new FrontstageCompiledArtifactCache({
      store: seeded.store,
      byteBudget: first.byteSize * 2,
      now: () => 10
    });
    await cache.put(identity(sourceB), artifact(sourceB));
    await cache.put(identity(sourceC), artifact(sourceC));
    await expect(cache.get(identity(sourceA))).resolves.toEqual({
      status: 'miss',
      reason: 'not_found'
    });
    await expect(cache.get(identity(sourceB))).resolves.toMatchObject({ status: 'hit' });
    await expect(cache.get(identity(sourceC))).resolves.toMatchObject({ status: 'hit' });

    let now = 1;
    const accessedSubject = subject('lru-access', undefined, () => now++);
    await accessedSubject.cache.put(identity(sourceA), artifact(sourceA));
    const accessedFirst = (await accessedSubject.store.list())[0] as FrontstageArtifactCacheRecord;
    const accessedCache = new FrontstageCompiledArtifactCache({
      store: accessedSubject.store,
      byteBudget: accessedFirst.byteSize * 2,
      now: () => now++
    });
    await accessedCache.put(identity(sourceB), artifact(sourceB));
    await accessedCache.get(identity(sourceA));
    await accessedCache.put(identity(sourceC), artifact(sourceC));
    await expect(accessedCache.get(identity(sourceB))).resolves.toEqual({
      status: 'miss',
      reason: 'not_found'
    });

    const oversized = subject('oversized', 32);
    await expect(
      oversized.cache.put(identity(), artifact())
    ).resolves.toEqual({ status: 'skipped', reason: 'oversized' });
    await expect(oversized.store.list()).resolves.toEqual([]);
  });

  test('D5-006 evicts after first quota failure, retries once, and degrades after the second failure', async () => {
    const seeded = subject('quota');
    await seeded.cache.put(identity(sourceA), artifact(sourceA));
    const quotaOnce = quotaStore(seeded.store, 1);
    const recovered = new FrontstageCompiledArtifactCache({ store: quotaOnce });
    await expect(
      recovered.put(identity(sourceB), artifact(sourceB))
    ).resolves.toMatchObject({ status: 'stored' });
    expect(quotaOnce.putAttempts()).toBe(2);
    await expect(seeded.cache.get(identity(sourceA))).resolves.toEqual({
      status: 'miss',
      reason: 'not_found'
    });

    const failedSubject = subject('quota-twice');
    await failedSubject.cache.put(identity(sourceA), artifact(sourceA));
    const quotaTwice = quotaStore(failedSubject.store, 2);
    await expect(
      new FrontstageCompiledArtifactCache({ store: quotaTwice }).put(
        identity(sourceB),
        artifact(sourceB)
      )
    ).resolves.toEqual({ status: 'unavailable', reason: 'quota_exceeded' });
    expect(quotaTwice.putAttempts()).toBe(2);
  });

  test('D5-006 returns unavailable without throwing when IndexedDB is absent', async () => {
    const cache = new FrontstageCompiledArtifactCache({
      store: createIndexedDbArtifactCacheStore({ indexedDB: null })
    });
    await expect(cache.get(identity())).resolves.toEqual({
      status: 'unavailable',
      reason: 'indexeddb_unavailable'
    });
    await expect(cache.put(identity(), artifact())).resolves.toEqual({
      status: 'unavailable',
      reason: 'indexeddb_unavailable'
    });
  });

  test('AC-022 removes corrupt and identity-mismatched records as misses', async () => {
    const { cache, store } = subject('corrupt');
    await cache.put(identity(sourceB), artifact(sourceB));
    const recordB = (await store.list())[0] as FrontstageArtifactCacheRecord;
    await store.delete(recordB.key);
    await store.put({
      ...recordB,
      key: createFrontstageArtifactCacheKey(identity(sourceA))
    });
    await expect(cache.get(identity(sourceA))).resolves.toEqual({
      status: 'miss',
      reason: 'identity_mismatch'
    });

    await (store.put as (value: FrontstageArtifactCacheRecord) => Promise<void>)({
      ...recordB,
      key: createFrontstageArtifactCacheKey(identity(sourceC)),
      artifact: { ...artifact(sourceC), program: { executableBody: 'broken' } } as never
    });
    await expect(cache.get(identity(sourceC))).resolves.toMatchObject({
      status: 'miss'
    });
  });

  test('AC-022 purges every workspace for the previous actor and prunes stale fingerprints on startup', async () => {
    const { cache, store } = subject('lifecycle');
    await cache.put(identity(sourceA), artifact(sourceA));
    await cache.put(
      identity(sourceB, { workspaceId: 'workspace-b' }),
      artifact(sourceB)
    );
    await cache.put(
      identity(sourceC, { actorId: 'actor-b' }),
      artifact(sourceC)
    );
    await expect(cache.deleteActor('actor-a')).resolves.toEqual({
      status: 'completed',
      deleted: 2
    });
    expect((await store.list()) as FrontstageArtifactCacheRecord[]).toEqual([
      expect.objectContaining({ actorId: 'actor-b' })
    ]);

    const oldFingerprint = 'old-runtime';
    await cache.put(
      identity(sourceA, { actorId: 'actor-b', runtimeFingerprint: oldFingerprint }),
      artifact(sourceA, { runtimeFingerprint: oldFingerprint })
    );
    const oldRecord = (await store.list()).find(
      (value) =>
        (value as FrontstageArtifactCacheRecord).runtimeFingerprint ===
        oldFingerprint
    ) as FrontstageArtifactCacheRecord;
    await store.put({
      ...oldRecord,
      key: createFrontstageArtifactCacheKey(
        identity(sourceB, { actorId: 'actor-b' })
      ),
      runtimeFingerprint,
      sourceSha256: sourceB,
      artifact: {
        ...artifact(sourceB),
        format: 'old-artifact-format'
      } as never
    });
    await expect(
      cache.pruneWorkspace({
        actorId: 'actor-b',
        workspaceId: 'workspace-a',
        runtimeFingerprint
      })
    ).resolves.toMatchObject({ status: 'completed', deleted: 2 });
    await expect(
      new FrontstageCompiledArtifactCache({ store, byteBudget: 1 }).pruneWorkspace({
        actorId: 'actor-b',
        workspaceId: 'workspace-a',
        runtimeFingerprint
      })
    ).resolves.toMatchObject({ status: 'completed', deleted: 1 });
  });
});

function quotaStore(store: FrontstageArtifactCacheStore, failures: number) {
  let attempts = 0;
  return {
    get: store.get,
    list: store.list,
    delete: store.delete,
    async put(record: FrontstageArtifactCacheRecord) {
      attempts += 1;
      if (attempts <= failures) {
        throw new DOMException('quota fixture', 'QuotaExceededError');
      }
      await store.put(record);
    },
    putAttempts: () => attempts
  } satisfies FrontstageArtifactCacheStore & { putAttempts(): number };
}
