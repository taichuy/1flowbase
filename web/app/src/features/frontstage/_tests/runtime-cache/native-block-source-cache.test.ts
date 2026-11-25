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

  test.each([1, 2])(
    'upgrades a populated v%i database in place and initializes the count once',
    async (version) => {
      const indexedDbFactory = new IDBFactory();
      const databaseName = 'upgrade';
      const identity = { actorId: 'a', workspaceId: 'w', requestId: 'old' };
      const source = createValidSource(`v${version}`);
      const legacy = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDbFactory.open(databaseName, version);
        request.onupgradeneeded = () => {
          const store = request.result.createObjectStore('sources', {
            keyPath: 'key'
          });
          store.createIndex('by_actor', version === 1 ? 'actorId' : 'actorKey');
          if (version === 2) store.createIndex('by_updated_at', 'updatedAtMs');
          store.put({
            ...identity,
            key: 'a/w/old',
            schemaVersion: 1,
            byteSize: 500,
            ...(version === 1
              ? { updatedAt: 1 }
              : { actorKey: 'a', updatedAtMs: 1 }),
            source
          });
        };
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error);
      });
      legacy.close();

      const options = {
        indexedDbFactory,
        databaseName,
        maxEntries: 1,
        now: () => 2
      };
      const cache = new FrontstageNativeBlockSourceCache(options);
      expect(await cache.get(identity)).toEqual(source);
      const upgraded = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDbFactory.open(databaseName);
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error);
      });
      expect(upgraded.version).toBe(3);
      expect(
        upgraded.objectStoreNames.contains(
          'frontstage_native_block_sources_meta'
        )
      ).toBe(true);
      const store = upgraded.transaction('sources').objectStore('sources');
      expect(store.index('by_actor').keyPath).toBe('actorKey');
      expect(store.index('by_updated_at').keyPath).toBe('updatedAtMs');
      upgraded.close();

      const count = vi.spyOn(IDBObjectStore.prototype, 'count');
      const latest = { ...identity, requestId: 'new' };
      await cache.put(latest, source);
      const reloaded = new FrontstageNativeBlockSourceCache(options);
      expect(await reloaded.get(identity)).toBeUndefined();
      expect(await reloaded.get(latest)).toEqual(source);
      await reloaded.put(latest, source);
      expect(count).toHaveBeenCalledTimes(1);
    }
  );

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
    expect((indexCursor.mock.contexts[0] as IDBIndex | undefined)?.name).toBe(
      'by_updated_at'
    );
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

  test('counts once and maintains the transactional count across concurrent puts, updates, deletes and clear', async () => {
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

  test('interleaved writes through two connections persist only the newest entries', async () => {
    let now = 0;
    const options = {
      indexedDbFactory: new IDBFactory(),
      maxEntries: 2,
      now: () => ++now
    };
    const firstHandle = new FrontstageNativeBlockSourceCache(options);
    const secondHandle = new FrontstageNativeBlockSourceCache(options);
    const source = createValidSource('interleaved');
    const oldest = { actorId: 'a', workspaceId: 'w', requestId: '1' };
    const middle = { ...oldest, requestId: '2' };
    const newest = { ...oldest, requestId: '3' };

    const count = vi.spyOn(IDBObjectStore.prototype, 'count');
    await firstHandle.put(oldest, source);
    await secondHandle.put(middle, source);
    await firstHandle.put(newest, source);

    const reloaded = new FrontstageNativeBlockSourceCache(options);
    expect(await reloaded.get(oldest)).toBeUndefined();
    expect(await reloaded.get(middle)).toEqual(source);
    expect(await reloaded.get(newest)).toEqual(source);
    expect(count).toHaveBeenCalledTimes(1);
  });

  test.each(['full clear', 'actor deletion'] as const)(
    '%s through a second connection does not trigger false eviction',
    async (operation) => {
      let now = 0;
      const options = {
        indexedDbFactory: new IDBFactory(),
        maxEntries: 2,
        now: () => ++now
      };
      const firstHandle = new FrontstageNativeBlockSourceCache(options);
      const secondHandle = new FrontstageNativeBlockSourceCache(options);
      const source = createValidSource('cleared');
      const first = { actorId: 'a', workspaceId: 'w', requestId: '1' };
      const second = { ...first, requestId: '2' };
      const latest = { ...first, requestId: '3' };

      await firstHandle.put(first, source);
      await firstHandle.put(second, source);
      const cursor = vi.spyOn(IDBIndex.prototype, 'openCursor');
      const count = vi.spyOn(IDBObjectStore.prototype, 'count');
      if (operation === 'full clear') await secondHandle.clear();
      else expect(await secondHandle.deleteActor('a')).toEqual({ deleted: 2 });
      await firstHandle.put(latest, source);

      const reloaded = new FrontstageNativeBlockSourceCache(options);
      expect(await reloaded.get(latest)).toEqual(source);
      expect(await reloaded.get(first)).toBeUndefined();
      expect(await reloaded.get(second)).toBeUndefined();
      expect(cursor).not.toHaveBeenCalled();
      expect(count).not.toHaveBeenCalled();
    }
  );

  test.each(['deleteActor', 'delete'] as const)(
    '%s through another connection removes only the deleted entries from the count',
    async (operation) => {
      let now = 0;
      const options = {
        indexedDbFactory: new IDBFactory(),
        maxEntries: 2,
        now: () => ++now
      };
      const writer = new FrontstageNativeBlockSourceCache(options);
      const remover = new FrontstageNativeBlockSourceCache(options);
      const source = createValidSource('partial deletion');
      const survivor = { actorId: 'a', workspaceId: 'w', requestId: '1' };
      const victim = { ...survivor, actorId: 'b' };
      const first = { ...survivor, requestId: '2' };
      const second = { ...survivor, requestId: '3' };
      await writer.put(survivor, source);
      await writer.put(victim, source);
      if (operation === 'deleteActor') {
        expect(await remover.deleteActor('b')).toEqual({ deleted: 1 });
      } else {
        await remover.delete(victim);
      }
      const cursor = vi.spyOn(IDBIndex.prototype, 'openCursor');
      await writer.put(first, source);
      expect(cursor).not.toHaveBeenCalled();
      await writer.put(second, source);
      expect(cursor).toHaveBeenCalledExactlyOnceWith(null, 'next');

      const reloaded = new FrontstageNativeBlockSourceCache(options);
      expect(await reloaded.get(victim)).toBeUndefined();
      expect(await reloaded.get(survivor)).toBeUndefined();
      expect(await reloaded.get(first)).toEqual(source);
      expect(await reloaded.get(second)).toEqual(source);
    }
  );

  test('aborting after the metadata write rolls back both the count and eviction', async () => {
    let now = 0;
    const options = {
      indexedDbFactory: new IDBFactory(),
      maxEntries: 2,
      now: () => ++now
    };
    const writer = new FrontstageNativeBlockSourceCache(options);
    const source = createValidSource('rollback');
    const first = { actorId: 'a', workspaceId: 'w', requestId: '1' };
    const second = { ...first, requestId: '2' };
    const uncommitted = { ...first, requestId: '3' };
    const next = { ...first, requestId: '4' };
    await writer.put(first, source);
    await writer.put(second, source);

    const put = IDBObjectStore.prototype.put;
    let metadataWritten = false;
    const abortWrite = vi
      .spyOn(IDBObjectStore.prototype, 'put')
      .mockImplementation(function (this: IDBObjectStore, ...args) {
        const request = put.apply(this, args);
        if (this.name === 'frontstage_native_block_sources_meta') {
          request.addEventListener('success', () => {
            metadataWritten = true;
            this.transaction.abort();
          });
        }
        return request;
      });
    const abortedWriter = new FrontstageNativeBlockSourceCache({
      ...options,
      maxEntries: 1
    });
    await abortedWriter.put(uncommitted, source);
    abortWrite.mockRestore();
    expect(metadataWritten).toBe(true);

    const reloaded = new FrontstageNativeBlockSourceCache(options);
    expect(await reloaded.get(first)).toEqual(source);
    expect(await reloaded.get(second)).toEqual(source);
    expect(await reloaded.get(uncommitted)).toBeUndefined();
    await reloaded.put(next, source);
    const afterCommit = new FrontstageNativeBlockSourceCache(options);
    expect(await afterCommit.get(first)).toBeUndefined();
    expect(await afterCommit.get(second)).toEqual(source);
    expect(await afterCommit.get(next)).toEqual(source);
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
