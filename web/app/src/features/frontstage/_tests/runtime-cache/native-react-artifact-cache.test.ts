import { IDBFactory } from 'fake-indexeddb';
import { describe, expect, test, vi } from 'vitest';
import {
  compileNativeReactComponent,
  createNativeReactRuntimeFingerprint,
  type NativeReactComponentArtifact
} from '@1flowbase/page-runtime';

import {
  FrontstageNativeReactArtifactCache,
  createFrontstageNativeReactArtifactCacheIdentity,
  createFrontstageNativeReactArtifactCacheKey,
  resolveFrontstageNativeReactArtifact,
  type FrontstageNativeReactArtifactCacheIdentity,
  type FrontstageNativeReactArtifactCacheRecord,
  type FrontstageNativeReactArtifactCacheStore
} from '../../lib/runtime-cache/native-react-artifact-cache';
import { createIndexedDbRecordStore } from '../../lib/runtime-cache/indexeddb-store';

const runtimeFingerprint = createNativeReactRuntimeFingerprint('/worker-a.js');

describe('Native React Artifact V2 IndexedDB cache', () => {
  test('D2-AC-005 returns a canonical L2 hit without invoking the compiler and isolates actor/workspace', async () => {
    const { cache } = subject('native-l2-hit');
    const currentIdentity = identity('source-a');
    const currentArtifact = artifact('source-a');
    await cache.put(currentIdentity, currentArtifact);
    const compile = vi.fn(async () => ({
      ok: true as const,
      artifact: currentArtifact,
      diagnostics: [] as []
    }));

    await expect(
      resolveFrontstageNativeReactArtifact({
        cache,
        identity: currentIdentity,
        compile
      })
    ).resolves.toEqual({ status: 'hit', artifact: currentArtifact });
    expect(compile).not.toHaveBeenCalled();
    await expect(
      cache.get({ ...currentIdentity, actorId: 'actor-b' })
    ).resolves.toEqual({ status: 'miss', reason: 'not_found' });
    await expect(
      cache.get({ ...currentIdentity, workspaceId: 'workspace-b' })
    ).resolves.toEqual({ status: 'miss', reason: 'not_found' });
  });

  test('D2-AC-006 stores only canonical artifact bytes and management metadata', async () => {
    const { cache, store } = subject('native-sensitive-canary');
    const currentArtifact = {
      ...artifact('source-a'),
      ctx: { inputs: { secret: 'ctx-input' } },
      token: 'secret-token',
      apiResponse: { secret: 'api-response' },
      reactState: { secret: 'react-state' },
      logs: ['secret-log'],
      effects: ['secret-effect']
    };
    await cache.put(identity('source-a'), currentArtifact);

    const records = await store.list();
    expect(records).toHaveLength(1);
    expect(JSON.stringify(records[0])).not.toMatch(
      /ctx-input|secret-token|api-response|react-state|secret-log|secret-effect/
    );
    expect(Object.keys(records[0] as object).sort()).toEqual([
      'actorId',
      'artifact',
      'byteSize',
      'compiler_abi',
      'dependency_lock_sha256',
      'key',
      'lastAccessedAt',
      'runtime_abi',
      'runtime_fingerprint',
      'schemaVersion',
      'source_sha256',
      'workspaceId'
    ]);
  });

  test('D2-AC-006 removes corrupt, identity-mismatched and old-fingerprint records for cold recovery', async () => {
    const { cache, store } = subject('native-recovery');
    const sourceBIdentity = identity('source-b');
    await cache.put(sourceBIdentity, artifact('source-b'));
    const record = (
      await store.list()
    )[0] as FrontstageNativeReactArtifactCacheRecord;
    await store.delete(record.key);
    await store.put({
      ...record,
      key: createFrontstageNativeReactArtifactCacheKey(identity('source-a'))
    });
    await expect(cache.get(identity('source-a'))).resolves.toEqual({
      status: 'miss',
      reason: 'identity_mismatch'
    });

    await store.put({
      ...record,
      key: createFrontstageNativeReactArtifactCacheKey(sourceBIdentity),
      artifact: {
        ...record.artifact,
        program: { ...record.artifact.program, executableBody: 'corrupt' }
      }
    });
    await expect(cache.get(sourceBIdentity)).resolves.toEqual({
      status: 'miss',
      reason: 'corrupt'
    });

    const oldRuntimeFingerprint =
      createNativeReactRuntimeFingerprint('/worker-old.js');
    await cache.put(
      identity('source-old', oldRuntimeFingerprint),
      artifact('source-old', oldRuntimeFingerprint)
    );
    await expect(
      cache.pruneWorkspace({
        actorId: 'actor-a',
        workspaceId: 'workspace-a',
        runtimeFingerprint
      })
    ).resolves.toMatchObject({ status: 'completed', deleted: 1 });
  });

  test('D2-AC-006 enforces byte LRU and retries once after quota eviction', async () => {
    const seeded = subject('native-lru-seed');
    await seeded.cache.put(identity('source-a'), artifact('source-a'));
    const first = (
      await seeded.store.list()
    )[0] as FrontstageNativeReactArtifactCacheRecord;
    const lru = new FrontstageNativeReactArtifactCache({
      store: seeded.store,
      byteBudget: first.byteSize * 2,
      now: () => 10
    });
    await lru.put(identity('source-b'), artifact('source-b'));
    await lru.put(identity('source-c'), artifact('source-c'));
    await expect(lru.get(identity('source-a'))).resolves.toEqual({
      status: 'miss',
      reason: 'not_found'
    });

    const quotaSubject = subject('native-quota');
    await quotaSubject.cache.put(identity('source-a'), artifact('source-a'));
    const quota = quotaStore(quotaSubject.store, 1);
    await expect(
      new FrontstageNativeReactArtifactCache({ store: quota }).put(
        identity('source-b'),
        artifact('source-b')
      )
    ).resolves.toMatchObject({ status: 'stored' });
    expect(quota.putAttempts()).toBe(2);
  });

  test('D2-AC-006 returns unavailable without blocking when IndexedDB is absent or read/write fails', async () => {
    const unavailable = new FrontstageNativeReactArtifactCache({
      store:
        createIndexedDbRecordStore<FrontstageNativeReactArtifactCacheRecord>({
          indexedDB: null
        })
    });
    await expect(unavailable.get(identity('source-a'))).resolves.toEqual({
      status: 'unavailable',
      reason: 'indexeddb_unavailable'
    });
    await expect(
      unavailable.put(identity('source-a'), artifact('source-a'))
    ).resolves.toEqual({
      status: 'unavailable',
      reason: 'indexeddb_unavailable'
    });
    const compiledArtifact = artifact('source-a');
    await expect(
      resolveFrontstageNativeReactArtifact({
        cache: unavailable,
        identity: identity('source-a'),
        compile: async () => ({
          ok: true,
          artifact: compiledArtifact,
          diagnostics: []
        })
      })
    ).resolves.toEqual({
      status: 'compiled',
      artifact: compiledArtifact,
      cacheWrite: {
        status: 'unavailable',
        reason: 'indexeddb_unavailable'
      }
    });

    const failingStore: FrontstageNativeReactArtifactCacheStore = {
      get: async () => {
        throw new Error('read failed');
      },
      list: async () => {
        throw new Error('list failed');
      },
      put: async () => {
        throw new Error('write failed');
      },
      delete: async () => undefined
    };
    const failing = new FrontstageNativeReactArtifactCache({
      store: failingStore
    });
    await expect(failing.get(identity('source-a'))).resolves.toEqual({
      status: 'unavailable',
      reason: 'read_failed'
    });
    await expect(
      failing.put(identity('source-a'), artifact('source-a'))
    ).resolves.toEqual({ status: 'unavailable', reason: 'write_failed' });
  });
});

function identity(
  source: string,
  fingerprint = runtimeFingerprint
): FrontstageNativeReactArtifactCacheIdentity {
  return createFrontstageNativeReactArtifactCacheIdentity({
    actorId: 'actor-a',
    workspaceId: 'workspace-a',
    source: componentSource(source),
    dependencyLock: [],
    runtimeFingerprint: fingerprint
  });
}

function artifact(
  source: string,
  fingerprint = runtimeFingerprint
): NativeReactComponentArtifact {
  const result = compileNativeReactComponent(
    componentSource(source),
    [],
    fingerprint
  );
  if (!result.ok)
    throw new Error('Expected cache artifact fixture to compile.');
  return result.artifact;
}

function componentSource(label: string): string {
  return `export default function Block() { return ${JSON.stringify(label)}; }`;
}

function subject(name: string) {
  const store =
    createIndexedDbRecordStore<FrontstageNativeReactArtifactCacheRecord>({
      indexedDB: new IDBFactory(),
      databaseName: name
    });
  return {
    store,
    cache: new FrontstageNativeReactArtifactCache({ store })
  };
}

function quotaStore(
  store: FrontstageNativeReactArtifactCacheStore,
  failures: number
) {
  let attempts = 0;
  return {
    get: store.get,
    list: store.list,
    delete: store.delete,
    async put(record: FrontstageNativeReactArtifactCacheRecord) {
      attempts += 1;
      if (attempts <= failures) {
        throw new DOMException('quota fixture', 'QuotaExceededError');
      }
      await store.put(record);
    },
    putAttempts: () => attempts
  } satisfies FrontstageNativeReactArtifactCacheStore & {
    putAttempts(): number;
  };
}
