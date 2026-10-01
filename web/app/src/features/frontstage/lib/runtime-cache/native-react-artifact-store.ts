import {
  NATIVE_REACT_COMPILER_ABI,
  NATIVE_REACT_RUNTIME_ABI
} from '@1flowbase/page-runtime';

import { IndexedDbUnavailableError } from './indexeddb-store';
import type { FrontstageNativeReactArtifactCacheRecord } from './native-react-artifact-cache';

type ArtifactRecord = FrontstageNativeReactArtifactCacheRecord;
type Metadata = Omit<
  ArtifactRecord,
  'artifact' | 'schemaVersion' | 'source_sha256'
>;
interface BudgetState {
  key: string;
  totalBytes: number;
  clock: number;
}

export interface NativeReactArtifactStore {
  get(key: string): Promise<unknown>;
  readEpoch(actorId: string): Promise<number>;
  commit(
    record: ArtifactRecord,
    byteBudget: number,
    epoch: number
  ): Promise<boolean>;
  touch(keys: readonly string[], now: number): Promise<void>;
  delete(key: string): Promise<void>;
  deleteActor(actorId: string): Promise<number>;
  pruneWorkspace(
    actorId: string,
    workspaceId: string,
    byteBudget: number
  ): Promise<number>;
}

const RECORDS = 'records';
const METADATA = 'metadata';
const STATE = 'state';

/** L2 payloads are point-read; eviction uses the small timestamp index only.
 * Every writer shares the state store transaction, including writers in other tabs.
 */
export interface NativeReactArtifactStoreOptions {
  indexedDB?: IDBFactory | null;
  databaseName?: string;
  /** Transaction fault injection; completion is still the persistence barrier. */
  onCommitQueued?(transaction: IDBTransaction): void;
}

export function createNativeReactArtifactStore(
  options: NativeReactArtifactStoreOptions = {}
): NativeReactArtifactStore {
  const factory = Object.hasOwn(options, 'indexedDB')
    ? options.indexedDB
    : globalThis.indexedDB;
  let connection: Promise<IDBDatabase> | undefined;
  const open = () => {
    if (!factory)
      return Promise.reject(
        new IndexedDbUnavailableError('IndexedDB is unavailable.')
      );
    if (!connection) {
      connection = new Promise<IDBDatabase>((resolve, reject) => {
        const request = factory.open(
          options.databaseName ?? '1flowbase-frontstage-native-react-artifacts',
          2
        );
        request.onupgradeneeded = () => {
          const db = request.result;
          // V1 artifacts are disposable; rebuild rather than scan payloads to migrate metadata.
          if (db.objectStoreNames.contains(RECORDS))
            db.deleteObjectStore(RECORDS);
          db.createObjectStore(RECORDS, { keyPath: 'key' });
          const metadata = db.createObjectStore(METADATA, { keyPath: 'key' });
          metadata.createIndex('access', 'lastAccessedAt');
          metadata.createIndex('actor', 'actorId');
          metadata.createIndex('workspace', ['actorId', 'workspaceId']);
          db.createObjectStore(STATE, { keyPath: 'key' });
        };
        request.onsuccess = () => {
          request.result.onversionchange = () => {
            request.result.close();
            connection = undefined;
          };
          resolve(request.result);
        };
        request.onerror = () =>
          reject(
            new IndexedDbUnavailableError('IndexedDB open failed.', {
              cause: request.error
            })
          );
        request.onblocked = () =>
          reject(new IndexedDbUnavailableError('IndexedDB open was blocked.'));
      });
      const pending = connection;
      void pending.catch(() => {
        if (connection === pending) connection = undefined;
      });
    }
    return connection;
  };

  const transact = async <T>(
    stores: string[],
    mode: IDBTransactionMode,
    operation: (tx: IDBTransaction, done: (value: T) => void) => void
  ): Promise<T> => {
    const db = await open();
    return new Promise<T>((resolve, reject) => {
      const tx = db.transaction(stores, mode);
      let result: T;
      tx.oncomplete = () => resolve(result);
      tx.onabort = tx.onerror = () =>
        reject(tx.error ?? new Error('IndexedDB transaction aborted.'));
      try {
        operation(tx, (value) => {
          result = value;
        });
      } catch (error) {
        tx.abort();
        reject(error);
      }
    });
  };
  const write = <T>(
    operation: (
      tx: IDBTransaction,
      state: BudgetState,
      done: (value: T) => void
    ) => void
  ) =>
    transact<T>([RECORDS, METADATA, STATE], 'readwrite', (tx, done) => {
      const request = tx.objectStore(STATE).get('budget');
      request.onsuccess = () =>
        operation(
          tx,
          request.result ?? { key: 'budget', totalBytes: 0, clock: 0 },
          done
        );
    });

  function remove(tx: IDBTransaction, state: BudgetState, metadata: Metadata) {
    tx.objectStore(RECORDS).delete(metadata.key);
    tx.objectStore(METADATA).delete(metadata.key);
    state.totalBytes -= metadata.byteSize;
  }
  function evict(
    tx: IDBTransaction,
    state: BudgetState,
    budget: number,
    done: (count: number) => void,
    protectedKey?: string
  ) {
    if (state.totalBytes <= budget) {
      done(0);
      return;
    }
    let count = 0;
    const request = tx.objectStore(METADATA).index('access').openCursor();
    request.onsuccess = () => {
      const cursor = request.result;
      if (!cursor || state.totalBytes <= budget) {
        done(count);
        return;
      }
      const metadata = cursor.value as Metadata;
      if (metadata.key !== protectedKey) {
        remove(tx, state, metadata);
        count += 1;
      }
      cursor.continue();
    };
  }

  return {
    get: (key) =>
      transact([RECORDS], 'readonly', (tx, done) => {
        const request = tx.objectStore(RECORDS).get(key);
        request.onsuccess = () => done(request.result);
      }),
    readEpoch: (actorId) =>
      transact([STATE], 'readonly', (tx, done) => {
        const request = tx.objectStore(STATE).get(`actor:${actorId}`);
        request.onsuccess = () => done(request.result?.epoch ?? 0);
      }),
    commit: (record, byteBudget, epoch) =>
      write<boolean>((tx, state, done) => {
        const epochRequest = tx
          .objectStore(STATE)
          .get(`actor:${record.actorId}`);
        epochRequest.onsuccess = () => {
          if (
            (epochRequest.result?.epoch ?? 0) !== epoch ||
            record.byteSize > byteBudget
          ) {
            done(false);
            return;
          }
          const existing = tx.objectStore(METADATA).get(record.key);
          existing.onsuccess = () => {
            state.clock = Math.max(state.clock + 1, record.lastAccessedAt);
            state.totalBytes +=
              record.byteSize - (existing.result?.byteSize ?? 0);
            const metadata: Metadata = {
              key: record.key,
              actorId: record.actorId,
              workspaceId: record.workspaceId,
              module_policy_sha256: record.module_policy_sha256,
              compiler_abi: record.compiler_abi,
              runtime_abi: record.runtime_abi,
              byteSize: record.byteSize,
              lastAccessedAt: record.lastAccessedAt
            };
            tx.objectStore(RECORDS).put(record);
            tx.objectStore(METADATA).put({
              ...metadata,
              lastAccessedAt: state.clock
            });
            evict(
              tx,
              state,
              byteBudget,
              () => {
                tx.objectStore(STATE).put(state);
                try {
                  options.onCommitQueued?.(tx);
                } catch {
                  tx.abort();
                }
                done(true);
              },
              record.key
            );
          };
        };
      }),
    touch: (keys, now) =>
      transact([METADATA, STATE], 'readwrite', (tx, done) => {
        const budget = tx.objectStore(STATE).get('budget');
        budget.onsuccess = () => {
          const state: BudgetState = budget.result ?? {
            key: 'budget',
            totalBytes: 0,
            clock: 0
          };
          for (const key of keys) {
            const request = tx.objectStore(METADATA).get(key);
            request.onsuccess = () => {
              if (!request.result) return;
              state.clock = Math.max(state.clock + 1, now);
              tx.objectStore(METADATA).put({
                ...request.result,
                lastAccessedAt: state.clock
              });
              tx.objectStore(STATE).put(state);
            };
          }
          done(undefined);
        };
      }),
    delete: (key) =>
      write<void>((tx, state, done) => {
        const request = tx.objectStore(METADATA).get(key);
        request.onsuccess = () => {
          if (request.result) remove(tx, state, request.result);
          else tx.objectStore(RECORDS).delete(key);
          tx.objectStore(STATE).put(state);
          done(undefined);
        };
      }),
    deleteActor: (actorId) =>
      write<number>((tx, state, done) => {
        const epochKey = `actor:${actorId}`;
        const epochRequest = tx.objectStore(STATE).get(epochKey);
        epochRequest.onsuccess = () =>
          tx.objectStore(STATE).put({
            key: epochKey,
            epoch: (epochRequest.result?.epoch ?? 0) + 1
          });
        const request = tx
          .objectStore(METADATA)
          .index('actor')
          .openCursor(actorId);
        let count = 0;
        request.onsuccess = () => {
          const cursor = request.result;
          if (!cursor) {
            tx.objectStore(STATE).put(state);
            done(count);
            return;
          }
          remove(tx, state, cursor.value);
          count += 1;
          cursor.continue();
        };
      }),
    pruneWorkspace: (actorId, workspaceId, byteBudget) =>
      write<number>((tx, state, done) => {
        const epochKey = `actor:${actorId}`;
        const epochRequest = tx.objectStore(STATE).get(epochKey);
        epochRequest.onsuccess = () =>
          tx.objectStore(STATE).put({
            key: epochKey,
            epoch: (epochRequest.result?.epoch ?? 0) + 1
          });
        const request = tx
          .objectStore(METADATA)
          .index('workspace')
          .openCursor([actorId, workspaceId]);
        let count = 0;
        request.onsuccess = () => {
          const cursor = request.result;
          if (!cursor) {
            evict(tx, state, byteBudget, (evicted) => {
              tx.objectStore(STATE).put(state);
              done(count + evicted);
            });
            return;
          }
          const metadata: Metadata = cursor.value;
          if (
            metadata.compiler_abi !== NATIVE_REACT_COMPILER_ABI ||
            metadata.runtime_abi !== NATIVE_REACT_RUNTIME_ABI
          ) {
            remove(tx, state, metadata);
            count += 1;
          }
          cursor.continue();
        };
      })
  };
}
