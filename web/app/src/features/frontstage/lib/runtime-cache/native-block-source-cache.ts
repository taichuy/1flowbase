import type { ConsoleFrontstageBlockNodeCode } from '@1flowbase/api-client';
import { sha256Text } from '@1flowbase/page-runtime/browser';
import { LRUCache } from 'lru-cache';

export const FRONTSTAGE_NATIVE_BLOCK_SOURCE_CACHE_SCHEMA_VERSION = 1 as const;
const DB_VERSION = 3;
const OBJECT_STORE_NAME = 'sources';
const META_STORE_NAME = 'frontstage_native_block_sources_meta';
const ENTRY_COUNT_KEY = 'entry_count';
const ACTOR_INDEX_NAME = 'by_actor';
const UPDATED_AT_INDEX_NAME = 'by_updated_at';
const DEFAULT_L1_BYTE_BUDGET = 2 * 1024 * 1024;
const DEFAULT_L2_MAX_ENTRIES = 256;
const DEFAULT_MAX_ENTRY_BYTES = 1024 * 1024;

export interface FrontstageNativeBlockSourceCacheIdentity {
  actorId: string;
  workspaceId: string;
  requestId: string;
}

export interface FrontstageNativeBlockSourceWriteEpoch {
  global: number;
  actor: number;
}

interface StoredNativeBlockSourceRecord extends FrontstageNativeBlockSourceCacheIdentity {
  key: string;
  schemaVersion: typeof FRONTSTAGE_NATIVE_BLOCK_SOURCE_CACHE_SCHEMA_VERSION;
  byteSize: number;
  actorKey: string;
  updatedAtMs: number;
  source: ConsoleFrontstageBlockNodeCode;
}

export interface FrontstageNativeBlockSourceCacheOptions {
  databaseName?: string;
  indexedDbFactory?: IDBFactory | null;
  l1ByteBudget?: number;
  maxEntries?: number;
  maxEntryBytes?: number;
  now?: () => number;
}

export function createFrontstageNativeBlockSourceCacheKey(
  identity: FrontstageNativeBlockSourceCacheIdentity
): string {
  return [identity.actorId, identity.workspaceId, identity.requestId]
    .map(encodeURIComponent)
    .join('/');
}

function isValidConditionalSource(
  source: unknown
): source is ConsoleFrontstageBlockNodeCode & { source_sha256: string } {
  if (!source || typeof source !== 'object') return false;
  const candidate = source as Partial<ConsoleFrontstageBlockNodeCode>;
  if (
    typeof candidate.block_id !== 'string' ||
    !candidate.block_id ||
    typeof candidate.page_id !== 'string' ||
    !candidate.page_id ||
    typeof candidate.source_code !== 'string' ||
    typeof candidate.source_sha256 !== 'string' ||
    candidate.source_sha256.length === 0
  ) {
    return false;
  }
  return (
    sha256Text(candidate.source_code) === candidate.source_sha256.toLowerCase()
  );
}

function cloneFrozenSource(
  source: ConsoleFrontstageBlockNodeCode
): ConsoleFrontstageBlockNodeCode {
  return Object.freeze({
    block_id: source.block_id,
    page_id: source.page_id,
    source_code: source.source_code,
    source_sha256: source.source_sha256
      ? source.source_sha256.toLowerCase()
      : null
  });
}

function estimateSourceByteSize(
  key: string,
  source: ConsoleFrontstageBlockNodeCode
): number {
  return (
    (key.length +
      source.block_id.length +
      source.page_id.length +
      source.source_code.length +
      (source.source_sha256?.length ?? 0)) *
    2
  );
}

export class FrontstageNativeBlockSourceCache {
  private readonly databaseName: string;
  private readonly indexedDbFactory: IDBFactory | null | undefined;
  private readonly maxEntries: number;
  private readonly maxEntryBytes: number;
  private readonly now: () => number;
  private readonly l1: LRUCache<string, StoredNativeBlockSourceRecord>;
  private dbPromise: Promise<IDBDatabase | null> | null = null;
  private globalEpoch = 0;
  private readonly actorEpochs = new Map<string, number>();

  constructor(options: FrontstageNativeBlockSourceCacheOptions = {}) {
    this.databaseName =
      options.databaseName ?? '1flowbase-frontstage-native-block-sources';
    this.indexedDbFactory = options.indexedDbFactory;
    this.maxEntries = options.maxEntries ?? DEFAULT_L2_MAX_ENTRIES;
    this.maxEntryBytes = options.maxEntryBytes ?? DEFAULT_MAX_ENTRY_BYTES;
    this.now = options.now ?? (() => Date.now());
    this.l1 = new LRUCache<string, StoredNativeBlockSourceRecord>({
      maxSize: options.l1ByteBudget ?? DEFAULT_L1_BYTE_BUDGET,
      sizeCalculation: (record) => Math.max(1, record.byteSize)
    });
  }

  captureWriteEpoch(actorId: string): FrontstageNativeBlockSourceWriteEpoch {
    return {
      global: this.globalEpoch,
      actor: this.actorEpochs.get(actorId) ?? 0
    };
  }

  private isCurrentEpoch(
    actorId: string,
    epoch?: FrontstageNativeBlockSourceWriteEpoch
  ): boolean {
    if (!epoch) return true;
    return (
      epoch.global === this.globalEpoch &&
      epoch.actor === (this.actorEpochs.get(actorId) ?? 0)
    );
  }

  private bumpActorEpoch(actorId: string): void {
    this.actorEpochs.set(actorId, (this.actorEpochs.get(actorId) ?? 0) + 1);
  }

  async get(
    identity: FrontstageNativeBlockSourceCacheIdentity
  ): Promise<ConsoleFrontstageBlockNodeCode | undefined> {
    if (!identity.actorId || !identity.workspaceId || !identity.requestId) {
      return undefined;
    }
    const key = createFrontstageNativeBlockSourceCacheKey(identity);
    const l1Hit = this.l1.get(key);
    if (l1Hit) {
      if (
        l1Hit.actorId === identity.actorId &&
        l1Hit.workspaceId === identity.workspaceId &&
        l1Hit.requestId === identity.requestId &&
        isValidConditionalSource(l1Hit.source)
      ) {
        return l1Hit.source;
      }
      this.l1.delete(key);
    }

    const db = await this.openDatabase();
    if (!db) return undefined;
    const epoch = this.captureWriteEpoch(identity.actorId);

    try {
      const record = await readRecordFromDb(db, key);
      if (!record) return undefined;
      if (
        record.schemaVersion !==
          FRONTSTAGE_NATIVE_BLOCK_SOURCE_CACHE_SCHEMA_VERSION ||
        record.key !== key ||
        record.actorId !== identity.actorId ||
        record.workspaceId !== identity.workspaceId ||
        record.requestId !== identity.requestId ||
        !isValidConditionalSource(record.source)
      ) {
        await deleteRecordFromDb(db, key).catch(() => undefined);
        return undefined;
      }
      const frozenSource = cloneFrozenSource(record.source);
      if (this.isCurrentEpoch(identity.actorId, epoch)) {
        this.l1.set(key, { ...record, source: frozenSource });
      }
      return frozenSource;
    } catch {
      return undefined;
    }
  }

  async put(
    identity: FrontstageNativeBlockSourceCacheIdentity,
    source: ConsoleFrontstageBlockNodeCode,
    epoch?: FrontstageNativeBlockSourceWriteEpoch
  ): Promise<boolean> {
    if (
      !identity.actorId ||
      !identity.workspaceId ||
      !identity.requestId ||
      !this.isCurrentEpoch(identity.actorId, epoch) ||
      !isValidConditionalSource(source)
    ) {
      return false;
    }
    const key = createFrontstageNativeBlockSourceCacheKey(identity);
    const frozenSource = cloneFrozenSource(source);
    const byteSize = estimateSourceByteSize(key, frozenSource);
    if (byteSize > this.maxEntryBytes) return false;

    const record: StoredNativeBlockSourceRecord = {
      key,
      schemaVersion: FRONTSTAGE_NATIVE_BLOCK_SOURCE_CACHE_SCHEMA_VERSION,
      actorId: identity.actorId,
      workspaceId: identity.workspaceId,
      requestId: identity.requestId,
      byteSize,
      actorKey: identity.actorId,
      updatedAtMs: this.now(),
      source: frozenSource
    };
    this.l1.set(key, record);

    const db = await this.openDatabase();
    if (!db || !this.isCurrentEpoch(identity.actorId, epoch)) {
      return true;
    }

    try {
      await writeRecordWithCap(db, record, this.maxEntries);
    } catch {
      // Persistence is an advisory If-None-Match hint; L1 remains valid.
    }
    return true;
  }

  async delete(
    identity: FrontstageNativeBlockSourceCacheIdentity
  ): Promise<void> {
    const key = createFrontstageNativeBlockSourceCacheKey(identity);
    this.l1.delete(key);
    const db = await this.openDatabase();
    if (!db) return;
    await deleteRecordFromDb(db, key).catch(() => undefined);
  }

  async clear(): Promise<void> {
    this.globalEpoch += 1;
    this.actorEpochs.clear();
    this.l1.clear();
    const db = await this.openDatabase();
    if (!db) return;
    await clearRecordsFromDb(db).catch(() => undefined);
  }

  async deleteActor(actorId: string): Promise<{ deleted: number }> {
    return actorId ? this.invalidateActor(actorId) : { deleted: 0 };
  }

  async pruneWorkspace({
    actorId,
    workspaceId
  }: {
    actorId: string;
    workspaceId: string;
  }): Promise<{ deleted: number }> {
    return actorId && workspaceId
      ? this.invalidateActor(actorId, workspaceId)
      : { deleted: 0 };
  }

  private async invalidateActor(
    actorId: string,
    keepWorkspaceId?: string
  ): Promise<{ deleted: number }> {
    this.bumpActorEpoch(actorId);
    let deleted = 0;
    for (const [key, record] of this.l1.entries()) {
      if (
        record.actorId === actorId &&
        record.workspaceId !== keepWorkspaceId
      ) {
        this.l1.delete(key);
        deleted += 1;
      }
    }
    const db = await this.openDatabase();
    if (!db) return { deleted };
    try {
      const persistedDeleted = await deleteActorRecords(
        db,
        actorId,
        keepWorkspaceId
      );
      return { deleted: Math.max(deleted, persistedDeleted) };
    } catch {
      return { deleted };
    }
  }

  private openDatabase(): Promise<IDBDatabase | null> {
    if (this.dbPromise) return this.dbPromise;
    const factory =
      this.indexedDbFactory !== undefined
        ? this.indexedDbFactory
        : typeof indexedDB !== 'undefined'
          ? indexedDB
          : null;
    if (!factory) {
      this.dbPromise = Promise.resolve(null);
      return this.dbPromise;
    }
    this.dbPromise = new Promise<IDBDatabase | null>((resolve) => {
      try {
        const request = factory.open(this.databaseName, DB_VERSION);
        request.onupgradeneeded = ({ oldVersion }) =>
          upgradeDatabase(request, oldVersion);
        request.onsuccess = () => {
          const db = request.result;
          db.onversionchange = () => {
            db.close();
            this.dbPromise = null;
          };
          resolve(db);
        };
        request.onerror = () => resolve(null);
        request.onblocked = () => resolve(null);
      } catch {
        resolve(null);
      }
    });
    return this.dbPromise;
  }
}

function upgradeDatabase(request: IDBOpenDBRequest, oldVersion: number): void {
  const db = request.result;
  if (!db.objectStoreNames.contains(META_STORE_NAME)) {
    db.createObjectStore(META_STORE_NAME, { keyPath: 'key' });
  }
  const store = db.objectStoreNames.contains(OBJECT_STORE_NAME)
    ? request.transaction!.objectStore(OBJECT_STORE_NAME)
    : db.createObjectStore(OBJECT_STORE_NAME, { keyPath: 'key' });
  if (
    store.indexNames.contains(ACTOR_INDEX_NAME) &&
    store.index(ACTOR_INDEX_NAME).keyPath !== 'actorKey'
  ) {
    store.deleteIndex(ACTOR_INDEX_NAME);
  }
  if (!store.indexNames.contains(ACTOR_INDEX_NAME)) {
    store.createIndex(ACTOR_INDEX_NAME, 'actorKey');
  }
  if (!store.indexNames.contains(UPDATED_AT_INDEX_NAME)) {
    store.createIndex(UPDATED_AT_INDEX_NAME, 'updatedAtMs');
  }
  if (oldVersion === 1) {
    const scan = store.openCursor();
    scan.onsuccess = () => {
      const cursor = scan.result;
      if (!cursor) return;
      const { updatedAt, ...record } = cursor.value as Omit<
        StoredNativeBlockSourceRecord,
        'actorKey' | 'updatedAtMs'
      > & {
        updatedAt: number;
      };
      cursor.update({
        ...record,
        actorKey: record.actorId,
        updatedAtMs: updatedAt
      });
      cursor.continue();
    };
  }
}

function readRecordFromDb(
  db: IDBDatabase,
  key: string
): Promise<StoredNativeBlockSourceRecord | undefined> {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(OBJECT_STORE_NAME, 'readonly');
    const req = tx.objectStore(OBJECT_STORE_NAME).get(key);
    req.onsuccess = () =>
      resolve(req.result as StoredNativeBlockSourceRecord | undefined);
    req.onerror = () => reject(req.error);
  });
}

type SaveCount<T> = (count: number, value: T) => void;

function mutateRecords<T>(
  db: IDBDatabase,
  mutate: (store: IDBObjectStore, saveCount: SaveCount<T>) => void
): Promise<T> {
  return new Promise((resolve, reject) => {
    // Overlapping write transactions serialize across connections, so the
    // count and payloads always commit (or roll back) together.
    const tx = db.transaction(
      [OBJECT_STORE_NAME, META_STORE_NAME],
      'readwrite'
    );
    const meta = tx.objectStore(META_STORE_NAME);
    let result: T;
    tx.oncomplete = () => resolve(result);
    tx.onabort = tx.onerror = () => reject(tx.error);
    mutate(tx.objectStore(OBJECT_STORE_NAME), (count, value) => {
      meta.put({ key: ENTRY_COUNT_KEY, count });
      result = value;
    });
  });
}

function mutateCountedRecords<T>(
  db: IDBDatabase,
  mutate: (
    store: IDBObjectStore,
    count: number,
    saveCount: SaveCount<T>
  ) => void
): Promise<T> {
  return mutateRecords<T>(db, (store, saveCount) => {
    const request = store.transaction
      .objectStore(META_STORE_NAME)
      .get(ENTRY_COUNT_KEY);
    request.onsuccess = () => {
      const state = request.result as { count: number } | undefined;
      if (state) {
        mutate(store, state.count, saveCount);
        return;
      }
      // Bootstrap a new or upgraded database once. The mutation saves the
      // resulting count before the same transaction commits.
      const initialCount = store.count();
      initialCount.onsuccess = () =>
        mutate(store, initialCount.result, saveCount);
    };
  });
}

function deleteRecordFromDb(db: IDBDatabase, key: string): Promise<void> {
  return mutateCountedRecords<void>(db, (store, count, saveCount) => {
    const existing = store.getKey(key);
    existing.onsuccess = () => {
      const found = existing.result !== undefined;
      if (found) store.delete(key);
      saveCount(Math.max(0, count - (found ? 1 : 0)), undefined);
    };
  });
}

function clearRecordsFromDb(db: IDBDatabase): Promise<void> {
  return mutateRecords<void>(db, (store, saveCount) => {
    store.clear();
    saveCount(0, undefined);
  });
}

function writeRecordWithCap(
  db: IDBDatabase,
  record: StoredNativeBlockSourceRecord,
  maxEntries: number
): Promise<void> {
  return mutateCountedRecords<void>(db, (store, count, saveCount) => {
    const existing = store.getKey(record.key);
    existing.onsuccess = () => {
      const nextCount = count + (existing.result === undefined ? 1 : 0);
      store.put(record);
      const overflow = nextCount - Math.max(0, maxEntries);
      if (overflow <= 0) {
        saveCount(nextCount, undefined);
        return;
      }
      let evicted = 0;
      const eviction = store
        .index(UPDATED_AT_INDEX_NAME)
        .openCursor(null, 'next');
      eviction.onsuccess = () => {
        const cursor = eviction.result;
        if (!cursor) {
          // An inconsistent index must not publish a partial eviction.
          store.transaction.abort();
          return;
        }
        cursor.delete();
        evicted += 1;
        if (evicted < overflow) cursor.continue();
        else saveCount(nextCount - evicted, undefined);
      };
    };
  });
}

function deleteActorRecords(
  db: IDBDatabase,
  actorId: string,
  keepWorkspaceId?: string
): Promise<number> {
  return mutateCountedRecords<number>(db, (store, count, saveCount) => {
    let deleted = 0;
    const finish = () => saveCount(Math.max(0, count - deleted), deleted);
    const index = store.index(ACTOR_INDEX_NAME);
    if (keepWorkspaceId === undefined) {
      const request = index.openKeyCursor(IDBKeyRange.only(actorId));
      request.onsuccess = () => {
        const cursor = request.result;
        if (!cursor) {
          finish();
          return;
        }
        store.delete(cursor.primaryKey);
        deleted += 1;
        cursor.continue();
      };
      return;
    }

    const request = index.openCursor(IDBKeyRange.only(actorId));
    request.onsuccess = () => {
      const cursor = request.result;
      if (!cursor) {
        finish();
        return;
      }
      const record = cursor.value as StoredNativeBlockSourceRecord;
      if (record.workspaceId !== keepWorkspaceId) {
        cursor.delete();
        deleted += 1;
      }
      cursor.continue();
    };
  });
}
