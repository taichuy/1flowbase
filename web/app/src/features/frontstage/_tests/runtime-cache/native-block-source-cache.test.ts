import {
  IDBCursor,
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

  test('upgrades v1 in place and indexes the existing records before eviction', async () => {
    const indexedDbFactory = new IDBFactory();
    const databaseName = 'upgrade';
    const identity = { actorId: 'a', workspaceId: 'w', requestId: 'old' };
    const source = createValidSource('v1');
    const v1 = await new Promise<IDBDatabase>((resolve, reject) => {
      const request = indexedDbFactory.open(databaseName, 1);
      request.onupgradeneeded = () => {
        const store = request.result.createObjectStore('sources', {
          keyPath: 'key'
        });
        store.createIndex('by_actor', 'actorId');
        store.put({
          ...identity,
          key: 'a/w/old',
          schemaVersion: 1,
          byteSize: 500,
          updatedAt: 1,
          source
        });
      };
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
    v1.close();

    const options = {
      indexedDbFactory,
      databaseName,
      maxEntries: 1,
      now: () => 2
    };
    const cache = new FrontstageNativeBlockSourceCache(options);
    expect(await cache.get(identity)).toEqual(source);
    const v2 = await new Promise<IDBDatabase>((resolve, reject) => {
      const request = indexedDbFactory.open(databaseName);
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
    expect(v2.version).toBe(2);
    const store = v2.transaction('sources').objectStore('sources');
    expect(store.index('by_actor').keyPath).toBe('actorKey');
    expect(store.index('by_updated_at').keyPath).toBe('updatedAtMs');
    v2.close();

    const latest = { ...identity, requestId: 'new' };
    await cache.put(latest, source);
    const reloaded = new FrontstageNativeBlockSourceCache(options);
    expect(await reloaded.get(identity)).toBeUndefined();
    expect(await reloaded.get(latest)).toEqual(source);
  });

  test('uses only the oldest index entries on overflow, with primary-key order for ties', async () => {
    let now = 1;
    const options = { indexedDbFactory: new IDBFactory(), now: () => now };
    const source = createValidSource('cap');
    const record = (actorId: string) => ({
      actorId,
      workspaceId: 'w',
      requestId: '1'
    });
    const seed = new FrontstageNativeBlockSourceCache(options);
    for (const actor of ['z', 'y', 'x']) await seed.put(record(actor), source);
    now = 2;
    await seed.put(record('a'), source);

    const getAll = vi.spyOn(IDBObjectStore.prototype, 'getAll');
    const storeCursor = vi.spyOn(IDBObjectStore.prototype, 'openCursor');
    const indexCursor = vi.spyOn(IDBIndex.prototype, 'openCursor');
    const deleteEntry = vi.spyOn(IDBCursor.prototype, 'delete');
    const advance = vi.spyOn(IDBCursor.prototype, 'continue');
    const capped = new FrontstageNativeBlockSourceCache({
      ...options,
      maxEntries: 2
    });
    now = 3;
    await capped.put(record('b'), source);
    expect(indexCursor).toHaveBeenCalledExactlyOnceWith(null, 'next');
    expect(indexCursor.mock.contexts[0]?.name).toBe('by_updated_at');
    expect(deleteEntry).toHaveBeenCalledTimes(3);
    expect(advance).toHaveBeenCalledTimes(2);
    expect(storeCursor).not.toHaveBeenCalled();
    expect(getAll).not.toHaveBeenCalled();
    const reloaded = new FrontstageNativeBlockSourceCache(options);
    for (const actor of ['x', 'y', 'z']) {
      expect(await reloaded.get(record(actor))).toBeUndefined();
    }
    for (const actor of ['a', 'b'])
      expect(await reloaded.get(record(actor))).toEqual(source);
  });

  test('counts once and maintains the cached count across concurrent puts, updates, deletes and clear', async () => {
    const options = { indexedDbFactory: new IDBFactory(), maxEntries: 2 };
    const cache = new FrontstageNativeBlockSourceCache(options);
    const source = createValidSource('steady-state');
    const first = { actorId: 'a', workspaceId: 'w', requestId: '1' };
    const second = { ...first, requestId: '2' };
    const third = { ...first, workspaceId: 'old' };
    const count = vi.spyOn(IDBObjectStore.prototype, 'count');
    const getKey = vi.spyOn(IDBObjectStore.prototype, 'getKey');
    const cursor = vi.spyOn(IDBIndex.prototype, 'openCursor');
    await Promise.all([cache.put(first, source), cache.put(second, source)]);
    await cache.put(first, source);
    expect(getKey).toHaveBeenCalledTimes(3);
    expect(count).toHaveBeenCalledTimes(1);
    expect(cursor).not.toHaveBeenCalled();

    await cache.delete(first);
    await cache.delete(first);
    await cache.put(third, source);
    expect(cursor).not.toHaveBeenCalled();
    expect(await cache.pruneWorkspace(second)).toEqual({ deleted: 1 });
    cursor.mockClear();
    await cache.put(first, source);
    expect(cursor).not.toHaveBeenCalled();
    expect(await cache.deleteActor('a')).toEqual({ deleted: 2 });
    await Promise.all([cache.put(first, source), cache.put(second, source)]);
    expect(cursor).not.toHaveBeenCalled();
    const epoch = cache.captureWriteEpoch('a');
    await cache.clear();
    expect(await cache.get(first)).toBeUndefined();
    expect(await cache.put(first, source, epoch)).toBe(false);
    await cache.put(third, source);
    expect(cursor).not.toHaveBeenCalled();
    expect(count).toHaveBeenCalledTimes(1);
    const reloaded = new FrontstageNativeBlockSourceCache(options);
    expect(await reloaded.get(second)).toBeUndefined();
    expect(await reloaded.get(third)).toEqual(source);
  });

  test('resynchronizes the cached count if the eviction index runs out of records', async () => {
    const indexedDbFactory = new IDBFactory();
    const databaseName = 'stale-index';
    const options = { indexedDbFactory, databaseName, maxEntries: 2 };
    const cache = new FrontstageNativeBlockSourceCache(options);
    const source = createValidSource('stale-index');
    const first = { actorId: 'a', workspaceId: 'w', requestId: '1' };
    await cache.put(first, source);
    await cache.put({ ...first, requestId: '2' }, source);
    await new FrontstageNativeBlockSourceCache(options).clear();

    const count = vi.spyOn(IDBObjectStore.prototype, 'count');
    const openCursor = IDBIndex.prototype.openCursor;
    const cursor = vi
      .spyOn(IDBIndex.prototype, 'openCursor')
      .mockImplementationOnce(function (this: IDBIndex) {
        return openCursor.call(this, IDBKeyRange.only(-1), 'next');
      });
    await cache.put({ ...first, requestId: '3' }, source);
    expect(count).toHaveBeenCalledTimes(1);
    cursor.mockClear();
    await cache.put(first, source);
    expect(cursor).not.toHaveBeenCalled();
    expect(count).toHaveBeenCalledTimes(1);
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
