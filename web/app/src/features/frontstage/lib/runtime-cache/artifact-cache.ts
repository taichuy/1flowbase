import {
  canonicalizeCompiledBlockArtifact,
  type CompiledBlockArtifact
} from '@1flowbase/page-runtime';

export const FRONTSTAGE_ARTIFACT_CACHE_SCHEMA_VERSION = 1 as const;
export const DEFAULT_FRONTSTAGE_ARTIFACT_CACHE_BYTE_BUDGET = 16 * 1024 * 1024;

export interface FrontstageArtifactCacheIdentity {
  actorId: string;
  workspaceId: string;
  runtimeFingerprint: string;
  sourceSha256: string;
}

export interface FrontstageArtifactCacheRecord
  extends FrontstageArtifactCacheIdentity {
  key: string;
  schemaVersion: typeof FRONTSTAGE_ARTIFACT_CACHE_SCHEMA_VERSION;
  byteSize: number;
  lastAccessedAt: number;
  artifact: CompiledBlockArtifact;
}

export interface FrontstageArtifactCacheStore {
  get(key: string): Promise<unknown | undefined>;
  list(): Promise<unknown[]>;
  put(record: FrontstageArtifactCacheRecord): Promise<void>;
  delete(key: string): Promise<void>;
}

export type FrontstageArtifactCacheReadResult =
  | { status: 'hit'; artifact: CompiledBlockArtifact }
  | { status: 'miss'; reason: 'not_found' | 'corrupt' | 'identity_mismatch' }
  | { status: 'unavailable'; reason: 'indexeddb_unavailable' | 'read_failed' };

export type FrontstageArtifactCacheWriteResult =
  | { status: 'stored'; byteSize: number }
  | { status: 'skipped'; reason: 'invalid_artifact' | 'identity_mismatch' | 'oversized' }
  | { status: 'unavailable'; reason: 'indexeddb_unavailable' | 'write_failed' | 'quota_exceeded' };

export type FrontstageArtifactCacheMaintenanceResult =
  | { status: 'completed'; deleted: number }
  | { status: 'unavailable'; reason: 'indexeddb_unavailable' | 'maintenance_failed' };

export interface FrontstageCompiledArtifactCacheOptions {
  store: FrontstageArtifactCacheStore;
  byteBudget?: number;
  now?: () => number;
}

export class FrontstageCompiledArtifactCache {
  private readonly byteBudget: number;
  private readonly now: () => number;

  constructor(private readonly options: FrontstageCompiledArtifactCacheOptions) {
    this.byteBudget = Math.max(
      1,
      Math.floor(
        options.byteBudget ?? DEFAULT_FRONTSTAGE_ARTIFACT_CACHE_BYTE_BUDGET
      )
    );
    this.now = options.now ?? Date.now;
  }

  async get(
    identity: FrontstageArtifactCacheIdentity
  ): Promise<FrontstageArtifactCacheReadResult> {
    const key = createFrontstageArtifactCacheKey(identity);
    let value: unknown;
    try {
      value = await this.options.store.get(key);
    } catch (error) {
      return unavailableRead(error);
    }
    if (value === undefined) return { status: 'miss', reason: 'not_found' };

    const record = canonicalizeFrontstageArtifactCacheRecord(value);
    if (!record) {
      await this.deleteWithoutThrow(key);
      return { status: 'miss', reason: 'corrupt' };
    }
    if (!recordMatchesIdentity(record, identity)) {
      await this.deleteWithoutThrow(key);
      return { status: 'miss', reason: 'identity_mismatch' };
    }

    const accessed = createCanonicalRecord(identity, record.artifact, this.now());
    if (accessed.byteSize > this.byteBudget) {
      await this.deleteWithoutThrow(key);
      return { status: 'miss', reason: 'corrupt' };
    }
    try {
      const records = canonicalRecords(await this.options.store.list()).filter(
        (item) => item.key !== accessed.key
      );
      await this.evictToFit(records, accessed.byteSize);
      await this.options.store.put(accessed);
    } catch {
      // Access-time persistence is best effort after a successful canonical read.
    }
    return { status: 'hit', artifact: record.artifact };
  }

  async put(
    identity: FrontstageArtifactCacheIdentity,
    value: unknown
  ): Promise<FrontstageArtifactCacheWriteResult> {
    const artifact = canonicalizeCompiledBlockArtifact(value);
    if (!artifact) return { status: 'skipped', reason: 'invalid_artifact' };
    if (
      artifact.runtimeFingerprint !== identity.runtimeFingerprint ||
      artifact.sourceSha256 !== identity.sourceSha256
    ) {
      return { status: 'skipped', reason: 'identity_mismatch' };
    }

    const record = createCanonicalRecord(identity, artifact, this.now());
    if (record.byteSize > this.byteBudget) {
      return { status: 'skipped', reason: 'oversized' };
    }

    let records: FrontstageArtifactCacheRecord[] = [];
    try {
      records = canonicalRecords(await this.options.store.list()).filter(
        (item) => item.key !== record.key
      );
      records = await this.evictToFit(records, record.byteSize);
      await this.options.store.put(record);
      return { status: 'stored', byteSize: record.byteSize };
    } catch (error) {
      if (!isQuotaExceeded(error)) return unavailableWrite(error);
    }

    const retryVictim = sortByLru(records)[0];
    if (retryVictim) await this.deleteWithoutThrow(retryVictim.key);
    try {
      await this.options.store.put(record);
      return { status: 'stored', byteSize: record.byteSize };
    } catch (error) {
      return isIndexedDbUnavailable(error)
        ? { status: 'unavailable', reason: 'indexeddb_unavailable' }
        : { status: 'unavailable', reason: 'quota_exceeded' };
    }
  }

  async deleteActor(
    actorId: string
  ): Promise<FrontstageArtifactCacheMaintenanceResult> {
    try {
      const records = await this.options.store.list();
      const actorPrefix = `${encodeURIComponent(actorId)}/`;
      const keys = records.flatMap((value) => {
        const identity = readRecordIdentity(value);
        if (identity?.actorId === actorId) return [identity.key];
        const rawKey = readRawRecordKey(value);
        return rawKey?.startsWith(actorPrefix) ? [rawKey] : [];
      });
      await Promise.all(keys.map((key) => this.options.store.delete(key)));
      return { status: 'completed', deleted: keys.length };
    } catch (error) {
      return unavailableMaintenance(error);
    }
  }

  async pruneWorkspace({
    actorId,
    workspaceId,
    runtimeFingerprint
  }: Omit<FrontstageArtifactCacheIdentity, 'sourceSha256'>): Promise<FrontstageArtifactCacheMaintenanceResult> {
    try {
      const values = await this.options.store.list();
      const deleted = new Set<string>();
      const current: FrontstageArtifactCacheRecord[] = [];
      const namespacePrefix = [actorId, workspaceId]
        .map(encodeURIComponent)
        .join('/') + '/';
      for (const value of values) {
        const identity = readRecordIdentity(value);
        if (!identity) {
          const rawKey = readRawRecordKey(value);
          if (rawKey?.startsWith(namespacePrefix)) deleted.add(rawKey);
          continue;
        }
        if (identity.actorId !== actorId || identity.workspaceId !== workspaceId) continue;
        const record = canonicalizeFrontstageArtifactCacheRecord(value);
        if (
          !record ||
          record.runtimeFingerprint !== runtimeFingerprint ||
          !recordMatchesIdentity(record, identity)
        ) {
          deleted.add(identity.key);
          continue;
        }
        current.push(record);
      }
      for (const key of deleted) await this.options.store.delete(key);
      let total = current.reduce((sum, record) => sum + record.byteSize, 0);
      for (const record of sortByLru(current)) {
        if (total <= this.byteBudget) break;
        await this.options.store.delete(record.key);
        deleted.add(record.key);
        total -= record.byteSize;
      }
      return { status: 'completed', deleted: deleted.size };
    } catch (error) {
      return unavailableMaintenance(error);
    }
  }

  private async evictToFit(
    records: FrontstageArtifactCacheRecord[],
    incomingBytes: number
  ): Promise<FrontstageArtifactCacheRecord[]> {
    let total = records.reduce((sum, record) => sum + record.byteSize, 0);
    const deleted = new Set<string>();
    for (const record of sortByLru(records)) {
      if (total + incomingBytes <= this.byteBudget) break;
      await this.options.store.delete(record.key);
      deleted.add(record.key);
      total -= record.byteSize;
    }
    return records.filter((record) => !deleted.has(record.key));
  }

  private async deleteWithoutThrow(key: string): Promise<void> {
    try {
      await this.options.store.delete(key);
    } catch {
      // Corrupt cache cleanup must never escape into the page runtime.
    }
  }
}

export function createFrontstageArtifactCacheKey(
  identity: FrontstageArtifactCacheIdentity
): string {
  return [
    identity.actorId,
    identity.workspaceId,
    identity.runtimeFingerprint,
    identity.sourceSha256
  ]
    .map(encodeURIComponent)
    .join('/');
}

export function canonicalizeFrontstageArtifactCacheRecord(
  value: unknown
): FrontstageArtifactCacheRecord | null {
  const identity = readRecordIdentity(value);
  if (!identity || !isRecord(value)) return null;
  const artifact = canonicalizeCompiledBlockArtifact(value.artifact);
  if (
    !artifact ||
    value.schemaVersion !== FRONTSTAGE_ARTIFACT_CACHE_SCHEMA_VERSION ||
    !isNonNegativeNumber(value.lastAccessedAt) ||
    !isPositiveInteger(value.byteSize)
  ) return null;
  const canonical = createCanonicalRecord(identity, artifact, value.lastAccessedAt);
  return canonical.byteSize === value.byteSize ? canonical : null;
}

function createCanonicalRecord(
  identity: FrontstageArtifactCacheIdentity,
  artifact: CompiledBlockArtifact,
  lastAccessedAt: number
): FrontstageArtifactCacheRecord {
  const base = {
    key: createFrontstageArtifactCacheKey(identity),
    schemaVersion: FRONTSTAGE_ARTIFACT_CACHE_SCHEMA_VERSION,
    actorId: identity.actorId,
    workspaceId: identity.workspaceId,
    runtimeFingerprint: identity.runtimeFingerprint,
    sourceSha256: identity.sourceSha256,
    lastAccessedAt,
    artifact
  };
  let byteSize = 1;
  for (;;) {
    const next = utf8ByteSize(JSON.stringify({ ...base, byteSize }));
    if (next === byteSize) return { ...base, byteSize };
    byteSize = next;
  }
}

function readRecordIdentity(value: unknown): (FrontstageArtifactCacheIdentity & { key: string }) | null {
  if (!isRecord(value)) return null;
  if (
    !isNonEmptyString(value.key) ||
    !isNonEmptyString(value.actorId) ||
    !isNonEmptyString(value.workspaceId) ||
    !isNonEmptyString(value.runtimeFingerprint) ||
    !isNonEmptyString(value.sourceSha256)
  ) return null;
  return {
    key: value.key,
    actorId: value.actorId,
    workspaceId: value.workspaceId,
    runtimeFingerprint: value.runtimeFingerprint,
    sourceSha256: value.sourceSha256
  };
}

function readRawRecordKey(value: unknown): string | null {
  return isRecord(value) && isNonEmptyString(value.key) ? value.key : null;
}

function recordMatchesIdentity(
  record: FrontstageArtifactCacheRecord,
  identity: FrontstageArtifactCacheIdentity
): boolean {
  return (
    record.key === createFrontstageArtifactCacheKey(identity) &&
    record.actorId === identity.actorId &&
    record.workspaceId === identity.workspaceId &&
    record.runtimeFingerprint === identity.runtimeFingerprint &&
    record.sourceSha256 === identity.sourceSha256 &&
    record.artifact.runtimeFingerprint === identity.runtimeFingerprint &&
    record.artifact.sourceSha256 === identity.sourceSha256
  );
}

function canonicalRecords(values: unknown[]): FrontstageArtifactCacheRecord[] {
  return values
    .map(canonicalizeFrontstageArtifactCacheRecord)
    .filter((record): record is FrontstageArtifactCacheRecord => record !== null);
}

function sortByLru(records: FrontstageArtifactCacheRecord[]): FrontstageArtifactCacheRecord[] {
  return [...records].sort(
    (left, right) =>
      left.lastAccessedAt - right.lastAccessedAt || compareStableKey(left.key, right.key)
  );
}

function compareStableKey(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

function utf8ByteSize(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function unavailableRead(error: unknown): FrontstageArtifactCacheReadResult {
  return {
    status: 'unavailable',
    reason: isIndexedDbUnavailable(error) ? 'indexeddb_unavailable' : 'read_failed'
  };
}
function unavailableWrite(error: unknown): FrontstageArtifactCacheWriteResult {
  return {
    status: 'unavailable',
    reason: isIndexedDbUnavailable(error) ? 'indexeddb_unavailable' : 'write_failed'
  };
}
function unavailableMaintenance(error: unknown): FrontstageArtifactCacheMaintenanceResult {
  return {
    status: 'unavailable',
    reason: isIndexedDbUnavailable(error) ? 'indexeddb_unavailable' : 'maintenance_failed'
  };
}
function isQuotaExceeded(error: unknown): boolean {
  return isRecord(error) && error.name === 'QuotaExceededError';
}
function isIndexedDbUnavailable(error: unknown): boolean {
  return isRecord(error) && error.name === 'IndexedDbUnavailableError';
}
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0;
}
function isNonNegativeNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0;
}
function isPositiveInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) > 0;
}
