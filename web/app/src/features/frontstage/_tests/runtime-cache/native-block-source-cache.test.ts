import {
  IDBDatabase,
  IDBFactory,
  IDBIndex,
  IDBKeyRange,
  IDBObjectStore
} from 'fake-indexeddb';
import { sha256Text } from '@1flowbase/page-runtime';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import { FrontstageNativeBlockSourceCache } from '../../lib/runtime-cache/native-block-source-cache';

function createValidSource(label: string) {
  const code = `export default function Block() { return ${JSON.stringify(label)}; }`;
  return {
    block_id: 'block-1',
    page_id: 'page-1',
    source_code: code,
    source_sha256: sha256Text(code)
  };
}

beforeEach(() => vi.stubGlobal('IDBKeyRange', IDBKeyRange));
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe('FrontstageNativeBlockSourceCache', () => {
  test('persists across cache instances in IndexedDB and isolates by actor and workspace', async () => {
    const indexedDbFactory = new IDBFactory();
    const firstInstance = new FrontstageNativeBlockSourceCache({
      databaseName: 'test-block-sources-1',
      indexedDbFactory
    });
    const sourceA = createValidSource('overview');
    const identityA = {
      actorId: 'actor-a',
      workspaceId: 'workspace-1',
      requestId: 'request-1'
    };

    expect(await firstInstance.put(identityA, sourceA)).toBe(true);

    // Simulate browser page.reload() with a fresh in-memory L1 cache backed by the same IndexedDB
    const reloadedInstance = new FrontstageNativeBlockSourceCache({
      databaseName: 'test-block-sources-1',
      indexedDbFactory
    });
    expect(await reloadedInstance.get(identityA)).toEqual(sourceA);
    expect(
      await reloadedInstance.get({
        ...identityA,
        actorId: 'actor-b'
      })
    ).toBeUndefined();
    expect(
      await reloadedInstance.get({
        ...identityA,
        workspaceId: 'workspace-2'
      })
    ).toBeUndefined();
  });

  test('rejects unverified or mismatched SHA-256 digests', async () => {
    const cache = new FrontstageNativeBlockSourceCache({
      databaseName: 'test-block-sources-2',
      indexedDbFactory: new IDBFactory()
    });
    const identity = {
      actorId: 'actor-a',
      workspaceId: 'workspace-1',
      requestId: 'request-1'
    };

    expect(
      await cache.put(identity, {
        block_id: 'block-1',
        page_id: 'page-1',
        source_code: 'export default function Block() { return null; }',
        source_sha256: '0'.repeat(64)
      })
    ).toBe(false);
    expect(
      await cache.put(identity, {
        block_id: 'block-1',
        page_id: 'page-1',
        source_code: 'export default function Block() { return null; }',
        source_sha256: null
      })
    ).toBe(false);
    expect(await cache.get(identity)).toBeUndefined();
  });

  test('deleteActor purges persisted entries and blocks stale pre-logout epoch writes', async () => {
    const indexedDbFactory = new IDBFactory();
    const cache = new FrontstageNativeBlockSourceCache({
      databaseName: 'test-block-sources-3',
      indexedDbFactory
    });
    const identity = {
      actorId: 'actor-a',
      workspaceId: 'workspace-1',
      requestId: 'request-1'
    };
    const staleEpoch = cache.captureWriteEpoch('actor-a');
    await cache.put(identity, createValidSource('v1'));

    await cache.deleteActor('actor-a');
    expect(await cache.get(identity)).toBeUndefined();

    // Stale write captured before logout must not re-persist
    expect(
      await cache.put(
        identity,
        createValidSource('stale-after-logout'),
        staleEpoch
      )
    ).toBe(false);
    expect(await cache.get(identity)).toBeUndefined();
  });

  test('pruneWorkspace deletes only the actor’s other workspaces and commits before reload', async () => {
    const options = {
      databaseName: 'multi-actor',
      indexedDbFactory: new IDBFactory()
    };
    const cache = new FrontstageNativeBlockSourceCache(options);
    const keep = { actorId: 'a', workspaceId: 'keep', requestId: '1' };
    const remove = { ...keep, workspaceId: 'old' };
    const otherActor = { ...remove, actorId: 'b' };
    const source = createValidSource('multi-actor');
    for (const identity of [
      keep,
      remove,
      { ...remove, requestId: '2' },
      otherActor
    ]) {
      expect(await cache.put(identity, source)).toBe(true);
    }
    const reloaded = new FrontstageNativeBlockSourceCache(options);
    expect(await reloaded.pruneWorkspace(keep)).toEqual({ deleted: 2 });
    expect(await reloaded.get(keep)).toEqual(source);
    expect(await reloaded.get(otherActor)).toEqual(source);
    expect(await reloaded.get(remove)).toBeUndefined();
    expect(await reloaded.get({ ...remove, requestId: '2' })).toBeUndefined();
  });

  test('actor invalidations never call getAll and logout uses a keys-only cursor', async () => {
    const cache = new FrontstageNativeBlockSourceCache({
      indexedDbFactory: new IDBFactory()
    });
    const identity = { actorId: 'a', workspaceId: 'old', requestId: '1' };
    const source = createValidSource('indexed');
    await cache.put(identity, source);
    await cache.put({ ...identity, workspaceId: 'keep' }, source);
    const getAll = vi.spyOn(IDBObjectStore.prototype, 'getAll');
    const keys = vi.spyOn(IDBIndex.prototype, 'openKeyCursor');
    const values = vi.spyOn(IDBIndex.prototype, 'openCursor');
    expect(
      await cache.pruneWorkspace({ actorId: 'a', workspaceId: 'keep' })
    ).toEqual({ deleted: 1 });
    expect(values).toHaveBeenCalledExactlyOnceWith(IDBKeyRange.only('a'));
    values.mockClear();
    expect(await cache.deleteActor('a')).toEqual({ deleted: 1 });
    expect(keys).toHaveBeenCalledExactlyOnceWith(IDBKeyRange.only('a'));
    expect(values).not.toHaveBeenCalled();
    expect(getAll).not.toHaveBeenCalled();
  });

  test('put checks the cap without reading records and eviction and invalidation never call getAll', async () => {
    let now = 1;
    const options = {
      indexedDbFactory: new IDBFactory(),
      maxEntries: 4,
      now: () => now
    };
    const cache = new FrontstageNativeBlockSourceCache(options);
    const getAll = vi.spyOn(IDBObjectStore.prototype, 'getAll');
    const count = vi.spyOn(IDBObjectStore.prototype, 'count');
    const cursor = vi.spyOn(IDBObjectStore.prototype, 'openCursor');
    const source = createValidSource('cap');
    const keep = { actorId: 'a', workspaceId: 'keep', requestId: '1' };
    const prune = { ...keep, workspaceId: 'old' };
    const evicted = { ...prune, actorId: 'y' };
    const logout = { ...prune, actorId: 'z' };
    const other = { ...keep, requestId: '1', actorId: 'b' };

    // Insert the tied records in reverse key order. The newer record has the
    // lowest key of the original records, so key order alone cannot evict it.
    expect(await cache.put(logout, source)).toBe(true);
    expect(await cache.put(evicted, source)).toBe(true);
    now = 2;
    expect(await cache.put(prune, source)).toBe(true);
    expect(await cache.put(other, source)).toBe(true);
    expect(count).toHaveBeenCalledTimes(4);
    expect(cursor).not.toHaveBeenCalled();
    expect(getAll).not.toHaveBeenCalled();

    // Even if the clock goes backwards, keep the just-written record and
    // evict exactly one older resident, breaking timestamp ties by primary key.
    now = 0;
    expect(await cache.put(keep, source)).toBe(true);
    expect(count).toHaveBeenCalledTimes(5);
    expect(cursor).toHaveBeenCalledTimes(1);
    const reloaded = new FrontstageNativeBlockSourceCache(options);
    expect(await reloaded.get(evicted)).toBeUndefined();
    for (const survivor of [keep, prune, other, logout]) {
      expect(await reloaded.get(survivor)).toEqual(source);
    }

    expect(await reloaded.deleteActor(logout.actorId)).toEqual({ deleted: 1 });
    expect(await reloaded.pruneWorkspace(keep)).toEqual({ deleted: 1 });
    const afterInvalidation = new FrontstageNativeBlockSourceCache(options);
    expect(await afterInvalidation.get(keep)).toEqual(source);
    expect(await afterInvalidation.get(other)).toEqual(source);
    for (const removed of [evicted, logout, prune]) {
      expect(await afterInvalidation.get(removed)).toBeUndefined();
    }
    expect(getAll).not.toHaveBeenCalled();
  });

  test('an aborted IDB transaction returns the L1 count and still invalidates the write epoch synchronously', async () => {
    const options = {
      databaseName: 'aborted',
      indexedDbFactory: new IDBFactory()
    };
    const cache = new FrontstageNativeBlockSourceCache(options);
    const identity = { actorId: 'a', workspaceId: 'old', requestId: '1' };
    const source = createValidSource('aborted');
    await cache.put(identity, source);
    const epoch = cache.captureWriteEpoch('a');
    const transaction = IDBDatabase.prototype.transaction;
    vi.spyOn(IDBDatabase.prototype, 'transaction').mockImplementationOnce(
      function (this: IDBDatabase, ...args) {
        const tx = transaction.apply(this, args);
        queueMicrotask(() => tx.abort());
        return tx;
      }
    );
    const pending = cache.deleteActor('a');
    expect(cache.captureWriteEpoch('a')).toEqual({
      ...epoch,
      actor: epoch.actor + 1
    });
    expect(await pending).toEqual({ deleted: 1 });
    expect(await cache.put(identity, source, epoch)).toBe(false);
    expect(
      await new FrontstageNativeBlockSourceCache(options).get(identity)
    ).toEqual(source);
  });
});
