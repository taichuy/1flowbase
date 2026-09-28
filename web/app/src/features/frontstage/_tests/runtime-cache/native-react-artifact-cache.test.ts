import { IDBFactory } from 'fake-indexeddb';
import { describe, expect, test, vi } from 'vitest';
import { compileNativeReactComponent } from '@1flowbase/page-runtime';

import {
  FrontstageNativeReactArtifactCache,
  createFrontstageNativeReactArtifactCacheIdentity,
  createFrontstageNativeReactArtifactCacheKey,
  resolveFrontstageNativeReactArtifact,
  type FrontstageNativeReactArtifactCacheRecord
} from '../../lib/runtime-cache/native-react-artifact-cache';
import { createNativeReactArtifactStore } from '../../lib/runtime-cache/native-react-artifact-store';

function fixture(
  label = 'a',
  actorId = 'actor-a',
  workspaceId = 'workspace-a'
) {
  const source = `export default function Block() { return ${JSON.stringify(label)}; }`;
  const compiled = compileNativeReactComponent(source, []);
  if (!compiled.ok) throw new Error('Fixture failed to compile');
  return {
    identity: createFrontstageNativeReactArtifactCacheIdentity({
      actorId,
      workspaceId,
      source
    }),
    artifact: compiled.artifact
  };
}
function subject(
  indexedDB = new IDBFactory(),
  databaseName = 'artifacts',
  byteBudget?: number
) {
  const store = createNativeReactArtifactStore({ indexedDB, databaseName });
  return {
    store,
    cache: new FrontstageNativeReactArtifactCache({
      store,
      byteBudget,
      now: () => 10
    }),
    indexedDB,
    databaseName
  };
}
async function rawRead(
  indexedDB: IDBFactory,
  databaseName: string,
  store: string,
  key?: string
) {
  const db = await new Promise<IDBDatabase>((resolve) => {
    const request = indexedDB.open(databaseName, 2);
    request.onsuccess = () => resolve(request.result);
  });
  try {
    return await new Promise<unknown>((resolve, reject) => {
      const tx = db.transaction(store, 'readonly');
      const request = key
        ? tx.objectStore(store).get(key)
        : tx.objectStore(store).getAll();
      tx.oncomplete = () => resolve(request.result);
      tx.onabort = () => reject(tx.error);
    });
  } finally {
    db.close();
  }
}
async function rawPut(
  indexedDB: IDBFactory,
  databaseName: string,
  store: string,
  record: unknown
) {
  const request = indexedDB.open(databaseName, 2);
  const db = await new Promise<IDBDatabase>((resolve) => {
    request.onsuccess = () => resolve(request.result);
  });
  try {
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(store, 'readwrite');
      tx.objectStore(store).put(record);
      tx.oncomplete = () => resolve();
      tx.onabort = () => reject(tx.error);
    });
  } finally {
    db.close();
  }
}

describe('Native React artifact L1/L2 cache', () => {
  test('ordinary document reopen reuses validated artifacts and partitions actor/workspace/source/ABI/policy', async () => {
    const { cache, indexedDB } = subject();
    const item = fixture();
    await cache.put(item.identity, item.artifact);
    const reopened = subject(indexedDB).cache;
    const compile = vi.fn();
    expect(
      await resolveFrontstageNativeReactArtifact({
        cache: reopened,
        identity: item.identity,
        compile
      })
    ).toEqual({ status: 'hit', artifact: item.artifact });
    expect(compile).not.toHaveBeenCalled();
    for (const identity of [
      fixture('b').identity,
      fixture('a', 'other-actor').identity,
      fixture('a', 'actor-a', 'other-workspace').identity,
      { ...item.identity, module_policy_sha256: 'f'.repeat(64) },
      { ...item.identity, compiler_abi: 'previous' }
    ]) {
      expect(await reopened.get(identity as typeof item.identity)).toEqual({
        status: 'miss',
        reason: 'not_found'
      });
    }
    await reopened.flush();
  });

  test('hits point-read one payload; repeated L1 hits do no persistence reads or foreground maintenance', async () => {
    const seed = subject();
    const item = fixture();
    await seed.cache.put(item.identity, item.artifact);
    const get = vi.fn(seed.store.get);
    const touch = vi.fn(async () => new Promise<void>(() => {}));
    const reopened = new FrontstageNativeReactArtifactCache({
      store: { ...seed.store, get, touch }
    });
    expect(await reopened.get(item.identity)).toMatchObject({
      status: 'hit',
      tier: 'l2'
    });
    for (let i = 0; i < 50; i++)
      expect(await reopened.get(item.identity)).toMatchObject({
        status: 'hit'
      });
    expect(get).toHaveBeenCalledOnce();
    expect(touch).not.toHaveBeenCalled();
    // Even if maintenance stalls, returning the next artifact remains independent.
    await new Promise((resolve) => setTimeout(resolve, 5));
    expect(await reopened.get(item.identity)).toMatchObject({
      status: 'hit',
      tier: 'l1'
    });
    expect(touch).toHaveBeenCalledOnce();
  });

  test('admission copies and freezes artifacts; consumers cannot poison later memory hits', async () => {
    const { cache } = subject();
    const item = fixture();
    await cache.put(item.identity, {
      ...item.artifact,
      ctx: 'secret',
      apiResponse: 'secret',
      token: 'secret'
    });
    item.artifact.program.executableBody = 'corrupted input after put';
    const hit = await cache.get(item.identity);
    expect(hit.status).toBe('hit');
    if (hit.status !== 'hit') return;
    expect(() => {
      hit.artifact.program.executableBody = 'corrupted return';
    }).toThrow();
    expect(JSON.stringify(hit.artifact)).not.toContain('secret');
    expect(await cache.get(item.identity)).toEqual(hit);
    await cache.flush();
  });

  test('disk integrity/identity/structure corruption fails closed after a document reopen', async () => {
    for (const corruption of ['integrity', 'structure', 'identity']) {
      const env = subject(new IDBFactory(), corruption);
      const item = fixture();
      await env.cache.put(item.identity, item.artifact);
      const key = createFrontstageNativeReactArtifactCacheKey(item.identity);
      const record = (await env.store.get(
        key
      )) as FrontstageNativeReactArtifactCacheRecord;
      if (corruption === 'integrity')
        record.artifact.integritySha256 = '0'.repeat(64);
      if (corruption === 'structure')
        record.artifact.program.executablePreambleLines = -1;
      if (corruption === 'identity') record.actorId = 'wrong-actor';
      await rawPut(env.indexedDB, env.databaseName, 'records', record);
      expect(
        (
          await subject(env.indexedDB, env.databaseName).cache.get(
            item.identity
          )
        ).status
      ).toBe('miss');
    }
  });

  test('metadata touches never rewrite artifact payloads or their serialized byte sizes', async () => {
    const env = subject();
    const item = fixture();
    await env.cache.put(item.identity, item.artifact);
    const key = createFrontstageNativeReactArtifactCacheKey(item.identity);
    const before = await env.store.get(key);
    await env.cache.get(item.identity);
    await env.cache.flush();
    expect(await env.store.get(key)).toEqual(before);
    expect(
      await rawRead(env.indexedDB, env.databaseName, 'metadata', key)
    ).toMatchObject({ lastAccessedAt: 11 });
  });

  test('byte LRU accounts replacements and competing tab writers atomically', async () => {
    const env = subject();
    const a = fixture('a'),
      b = fixture('b'),
      c = fixture('c');
    await env.cache.put(a.identity, a.artifact);
    const record = (await env.store.get(
      createFrontstageNativeReactArtifactCacheKey(a.identity)
    )) as FrontstageNativeReactArtifactCacheRecord;
    const budget = record.byteSize * 2;
    const first = subject(env.indexedDB, env.databaseName, budget);
    const second = subject(env.indexedDB, env.databaseName, budget);
    await Promise.all([
      first.cache.put(b.identity, b.artifact),
      second.cache.put(c.identity, c.artifact)
    ]);
    let records = (await rawRead(
      env.indexedDB,
      env.databaseName,
      'records'
    )) as FrontstageNativeReactArtifactCacheRecord[];
    expect(records).toHaveLength(2);
    expect(records.reduce((sum, r) => sum + r.byteSize, 0)).toBeLessThanOrEqual(
      budget
    );
    await first.cache.get(b.identity);
    await first.cache.flush();
    await second.cache.put(a.identity, a.artifact);
    records = (await rawRead(
      env.indexedDB,
      env.databaseName,
      'records'
    )) as FrontstageNativeReactArtifactCacheRecord[];
    expect(records.map((r) => r.source_sha256)).toContain(
      b.identity.source_sha256
    );
    expect(records.map((r) => r.source_sha256)).not.toContain(
      c.identity.source_sha256
    );
    await first.cache.put(a.identity, a.artifact);
    records = (await rawRead(
      env.indexedDB,
      env.databaseName,
      'records'
    )) as FrontstageNativeReactArtifactCacheRecord[];
    const state = (await rawRead(
      env.indexedDB,
      env.databaseName,
      'state',
      'budget'
    )) as { totalBytes: number };
    expect(state.totalBytes).toBe(
      records.reduce((sum, r) => sum + r.byteSize, 0)
    );
    expect(state.totalBytes).toBeLessThanOrEqual(budget);
  });

  test('actor deletion rejects queued and cross-tab late writes captured before logout', async () => {
    const env = subject();
    const otherTab = subject(env.indexedDB);
    const item = fixture();
    const oldEpoch = otherTab.cache.captureWriteEpoch(item.identity.actorId);
    await oldEpoch;
    const queued = env.cache.put(item.identity, item.artifact);
    await env.cache.deleteActor(item.identity.actorId);
    expect(await queued).toMatchObject({ status: 'skipped' });
    expect(
      await otherTab.cache.put(item.identity, item.artifact, oldEpoch)
    ).toMatchObject({ status: 'skipped' });
    expect(
      await env.store.get(
        createFrontstageNativeReactArtifactCacheKey(item.identity)
      )
    ).toBeUndefined();
    expect(
      await otherTab.cache.put(item.identity, item.artifact)
    ).toMatchObject({ status: 'stored' });
  });

  test('an aborted admission rolls payload, metadata, eviction and byte accounting back together', async () => {
    const env = subject();
    const a = fixture('a'),
      b = fixture('b');
    await env.cache.put(a.identity, a.artifact);
    const before = await rawRead(
      env.indexedDB,
      env.databaseName,
      'state',
      'budget'
    );
    const records = await rawRead(env.indexedDB, env.databaseName, 'records');
    const record = (records as FrontstageNativeReactArtifactCacheRecord[])[0];
    const aborted = new FrontstageNativeReactArtifactCache({
      byteBudget: record.byteSize + 64,
      store: createNativeReactArtifactStore({
        indexedDB: env.indexedDB,
        databaseName: env.databaseName,
        onCommitQueued: (tx) => tx.abort()
      })
    });
    expect(await aborted.put(b.identity, b.artifact)).toMatchObject({
      status: 'unavailable',
      reason: 'write_failed'
    });
    expect(await rawRead(env.indexedDB, env.databaseName, 'records')).toEqual(
      records
    );
    expect(
      await rawRead(env.indexedDB, env.databaseName, 'state', 'budget')
    ).toEqual(before);
  });

  test('workspace pruning uses metadata and rejects stale writes even from another connection', async () => {
    const env = subject();
    const item = fixture();
    await env.cache.put(item.identity, item.artifact);
    const oldEpoch = env.cache.captureWriteEpoch(item.identity.actorId);
    await oldEpoch;
    const key = createFrontstageNativeReactArtifactCacheKey(item.identity);
    const metadata = (await rawRead(
      env.indexedDB,
      env.databaseName,
      'metadata',
      key
    )) as object;
    await rawPut(env.indexedDB, env.databaseName, 'metadata', {
      ...metadata,
      compiler_abi: 'obsolete'
    });
    const anotherTab = subject(env.indexedDB);
    expect(await anotherTab.cache.pruneWorkspace(item.identity)).toEqual({
      status: 'completed',
      deleted: 1
    });
    expect(
      await env.cache.put(item.identity, item.artifact, oldEpoch)
    ).toMatchObject({ status: 'skipped' });
    expect(await env.store.get(key)).toBeUndefined();
  });

  test('oversized/unavailable/quota writes remain optional; failed transactions preserve existing artifacts', async () => {
    const item = fixture();
    const env = subject();
    expect(
      await subject(new IDBFactory(), 'small', 1).cache.put(
        item.identity,
        item.artifact
      )
    ).toEqual({ status: 'skipped', reason: 'oversized' });
    const absent = new FrontstageNativeReactArtifactCache({
      store: createNativeReactArtifactStore({ indexedDB: null })
    });
    expect(await absent.get(item.identity)).toEqual({
      status: 'unavailable',
      reason: 'indexeddb_unavailable'
    });
    expect(await absent.put(item.identity, item.artifact)).toEqual({
      status: 'unavailable',
      reason: 'indexeddb_unavailable'
    });
    await env.cache.put(item.identity, item.artifact);
    const failed = new FrontstageNativeReactArtifactCache({
      store: {
        ...env.store,
        commit: async () => {
          throw new DOMException('quota', 'QuotaExceededError');
        }
      }
    });
    expect(
      await failed.put(fixture('b').identity, fixture('b').artifact)
    ).toEqual({ status: 'unavailable', reason: 'quota_exceeded' });
    expect(await env.cache.get(item.identity)).toMatchObject({ status: 'hit' });
    await env.cache.flush();
  });
});
