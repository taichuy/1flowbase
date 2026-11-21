import type { ConsoleFrontstageBlockNodeCode } from '@1flowbase/api-client';
import { sha256Text } from '@1flowbase/page-runtime';
import { LRUCache } from 'lru-cache';

export const FRONTSTAGE_NATIVE_BLOCK_SOURCE_CACHE_SCHEMA_VERSION = 1 as const;
const OBJECT_STORE_NAME = 'sources';
const ACTOR_INDEX_NAME = 'by_actor';
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

interface StoredNativeBlockSourceRecord
  extends FrontstageNativeBlockSourceCacheIdentity {
  key: string;
  schemaVersion: typeof FRONTSTAGE_NATIVE_BLOCK_SOURCE_CACHE_SCHEMA_VERSION;
  byteSize: number;
  updatedAt: number;
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
      updatedAt: this.now(),
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

  async deleteActor(actorId: string): Promise<{ deleted: number }> {
    if (!actorId) return { deleted: 0 };
    this.bumpActorEpoch(actorId);
    let deleted = 0;
    for (const [key, record] of this.l1.entries()) {
      if (record.actorId === actorId) {
        this.l1.delete(key);
        deleted += 1;
      }
    }
    const db = await this.openDatabase();
    if (!db) return { deleted };
    try {
      const persistedDeleted = await deleteRecordsWhere(
        db,
        actorId,
        () => true
      );
      return { deleted: Math.max(deleted, persistedDeleted) };
    } catch {
      return { deleted };
    }
  }

  async pruneWorkspace({
    actorId,
    workspaceId
  }: {
    actorId: string;
    workspaceId: string;
  }): Promise<{ deleted: number }> {
    if (!actorId || !workspaceId) return { deleted: 0 };
    this.bumpActorEpoch(actorId);
    let deleted = 0;
    for (const [key, record] of this.l1.entries()) {
      if (record.actorId === actorId && record.workspaceId !== workspaceId) {
        this.l1.delete(key);
        deleted += 1;
      }
    }
    const db = await this.openDatabase();
    if (!db) return { deleted };
    try {
      const persistedDeleted = await deleteRecordsWhere(
        db,
        actorId,
        (record) => record.workspaceId !== workspaceId
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
        const request = factory.open(
          this.databaseName,
          FRONTSTAGE_NATIVE_BLOCK_SOURCE_CACHE_SCHEMA_VERSION
        );
        request.onupgradeneeded = () => {
          const db = request.result;
          if (db.objectStoreNames.contains(OBJECT_STORE_NAME)) {
            db.deleteObjectStore(OBJECT_STORE_NAME);
          }
          const store = db.createObjectStore(OBJECT_STORE_NAME, {
            keyPath: 'key'
          });
          store.createIndex(ACTOR_INDEX_NAME, 'actorId', { unique: false });
        };
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

function deleteRecordFromDb(db: IDBDatabase, key: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(OBJECT_STORE_NAME, 'readwrite');
    tx.objectStore(OBJECT_STORE_NAME).delete(key);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
    tx.onabort = () => reject(tx.error);
  });
}

function writeRecordWithCap(
  db: IDBDatabase,
  record: StoredNativeBlockSourceRecord,
  maxEntries: number
): Promise<void> {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(OBJECT_STORE_NAME, 'readwrite');
    const store = tx.objectStore(OBJECT_STORE_NAME);
    store.put(record);
    const allReq = store.getAll();
    allReq.onsuccess = () => {
      const items = (allReq.result ?? []) as StoredNativeBlockSourceRecord[];
      if (items.length <= maxEntries) return;
      items.sort((left, right) => left.updatedAt - right.updatedAt);
      const excess = items.length - maxEntries;
      for (let index = 0; index < excess; index += 1) {
        const victim = items[index];
        if (victim && victim.key !== record.key) {
          store.delete(victim.key);
        }
      }
    };
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
    tx.onabort = () => reject(tx.error);
  });
}

function deleteRecordsWhere(
  db: IDBDatabase,
  actorId: string,
  predicate: (record: StoredNativeBlockSourceRecord) => boolean
): Promise<number> {
  return new Promise((resolve, reject) => {
    let deleted = 0;
    const tx = db.transaction(OBJECT_STORE_NAME, 'readwrite');
    const store = tx.objectStore(OBJECT_STORE_NAME);
    const allReq = store.getAll();
    allReq.onsuccess = () => {
      const records = (allReq.result ?? []) as StoredNativeBlockSourceRecord[];
      for (const record of records) {
        if (record.actorId === actorId && predicate(record)) {
          store.delete(record.key);
          deleted += 1;
        }
      }
    };
    tx.oncomplete = () => resolve(deleted);
    tx.onerror = () => reject(tx.error);
    tx.onabort = () => reject(tx.error);
  });
}
