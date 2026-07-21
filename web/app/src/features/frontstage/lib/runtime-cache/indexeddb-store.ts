import type {
  FrontstageArtifactCacheRecord,
  FrontstageArtifactCacheStore
} from './artifact-cache';

export const FRONTSTAGE_ARTIFACT_CACHE_DATABASE =
  '1flowbase-frontstage-compiled-artifacts';
export const FRONTSTAGE_ARTIFACT_CACHE_OBJECT_STORE = 'artifacts';

export interface IndexedDbArtifactCacheStoreOptions {
  indexedDB?: IDBFactory | null;
  databaseName?: string;
  onRequestSuccess?: (context: {
    mode: IDBTransactionMode;
    transaction: IDBTransaction;
  }) => void;
}

export class IndexedDbUnavailableError extends Error {
  constructor(message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = 'IndexedDbUnavailableError';
  }
}

export function createIndexedDbArtifactCacheStore(
  options: IndexedDbArtifactCacheStoreOptions = {}
): FrontstageArtifactCacheStore {
  const factory = Object.hasOwn(options, 'indexedDB')
    ? (options.indexedDB ?? null)
    : readGlobalIndexedDb();
  const databaseName =
    options.databaseName ?? FRONTSTAGE_ARTIFACT_CACHE_DATABASE;
  let databasePromise: Promise<IDBDatabase> | null = null;

  const open = () => {
    if (!factory) {
      return Promise.reject(
        new IndexedDbUnavailableError('IndexedDB is unavailable.')
      );
    }
    if (!databasePromise) {
      const pending = openDatabase(factory, databaseName, () => {
        databasePromise = null;
      });
      databasePromise = pending;
      void pending.catch(() => {
        if (databasePromise === pending) databasePromise = null;
      });
    }
    return databasePromise;
  };

  return {
    async get(key) {
      return runRequest(
        open,
        'readonly',
        (store) => store.get(key),
        options.onRequestSuccess
      );
    },
    async list() {
      const value = await runRequest(
        open,
        'readonly',
        (store) => store.getAll(),
        options.onRequestSuccess
      );
      return Array.isArray(value) ? value : [];
    },
    async put(record) {
      await runRequest(
        open,
        'readwrite',
        (store) => store.put(record),
        options.onRequestSuccess
      );
    },
    async delete(key) {
      await runRequest(
        open,
        'readwrite',
        (store) => store.delete(key),
        options.onRequestSuccess
      );
    }
  };
}

function readGlobalIndexedDb(): IDBFactory | null {
  return typeof globalThis.indexedDB === 'undefined'
    ? null
    : globalThis.indexedDB;
}

function openDatabase(
  factory: IDBFactory,
  databaseName: string,
  onVersionChange: () => void
): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    let request: IDBOpenDBRequest;
    try {
      request = factory.open(databaseName, 1);
    } catch (error) {
      reject(new IndexedDbUnavailableError('IndexedDB open failed.', { cause: error }));
      return;
    }
    request.onupgradeneeded = () => {
      const database = request.result;
      if (!database.objectStoreNames.contains(FRONTSTAGE_ARTIFACT_CACHE_OBJECT_STORE)) {
        database.createObjectStore(FRONTSTAGE_ARTIFACT_CACHE_OBJECT_STORE, {
          keyPath: 'key'
        });
      }
    };
    request.onsuccess = () => {
      request.result.onversionchange = () => {
        onVersionChange();
        request.result.close();
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
}

async function runRequest<T>(
  open: () => Promise<IDBDatabase>,
  mode: IDBTransactionMode,
  operation: (store: IDBObjectStore) => IDBRequest<T>,
  onRequestSuccess?: IndexedDbArtifactCacheStoreOptions['onRequestSuccess']
): Promise<T> {
  let database: IDBDatabase;
  try {
    database = await open();
  } catch (error) {
    throw error instanceof IndexedDbUnavailableError
      ? error
      : new IndexedDbUnavailableError('IndexedDB is unavailable.', {
          cause: error
        });
  }
  return new Promise<T>((resolve, reject) => {
    let settled = false;
    let requestSucceeded = false;
    let requestResult: T;
    const settle = (result: { ok: true } | { ok: false; error: unknown }) => {
      if (settled) return;
      settled = true;
      if (result.ok) resolve(requestResult);
      else reject(result.error);
    };
    try {
      const transaction = database.transaction(
        FRONTSTAGE_ARTIFACT_CACHE_OBJECT_STORE,
        mode
      );
      const request = operation(
        transaction.objectStore(FRONTSTAGE_ARTIFACT_CACHE_OBJECT_STORE)
      );
      request.onsuccess = () => {
        requestSucceeded = true;
        requestResult = request.result;
        try {
          onRequestSuccess?.({ mode, transaction });
        } catch (error) {
          try {
            transaction.abort();
          } catch {
            settle({ ok: false, error });
          }
        }
      };
      request.onerror = () =>
        settle({
          ok: false,
          error: request.error ?? new Error('IndexedDB request failed.')
        });
      transaction.oncomplete = () =>
        requestSucceeded
          ? settle({ ok: true })
          : settle({
              ok: false,
              error: new Error('IndexedDB transaction completed without a request result.')
            });
      transaction.onabort = () =>
        settle({
          ok: false,
          error:
            transaction.error ?? new Error('IndexedDB transaction aborted.')
        });
      transaction.onerror = () =>
        settle({
          ok: false,
          error:
            transaction.error ?? new Error('IndexedDB transaction failed.')
        });
    } catch (error) {
      settle({ ok: false, error });
    }
  });
}
