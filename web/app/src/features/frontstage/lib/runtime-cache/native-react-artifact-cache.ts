import {
  canonicalizeNativeReactComponentArtifact,
  createNativeReactComponentArtifactIdentity,
  nativeReactComponentArtifactMatchesIdentity,
  sha256Text,
  type NativeReactCatalogDependencyLock,
  type NativeReactComponentArtifact,
  type NativeReactComponentArtifactIdentity
} from '@1flowbase/page-runtime';

import type { FrontstageIndexedDbRecordStore } from './indexeddb-store';
import type { NativeReactBrowserCompileResult } from '../../../../shared/code-block/native-react-compiler-browser';

export const FRONTSTAGE_NATIVE_REACT_ARTIFACT_CACHE_SCHEMA_VERSION = 1 as const;
export const DEFAULT_FRONTSTAGE_NATIVE_REACT_ARTIFACT_CACHE_BYTE_BUDGET =
  16 * 1024 * 1024;

export interface FrontstageNativeReactArtifactCacheIdentity extends NativeReactComponentArtifactIdentity {
  actorId: string;
  workspaceId: string;
}

export interface FrontstageNativeReactArtifactCacheRecord extends FrontstageNativeReactArtifactCacheIdentity {
  key: string;
  schemaVersion: typeof FRONTSTAGE_NATIVE_REACT_ARTIFACT_CACHE_SCHEMA_VERSION;
  byteSize: number;
  lastAccessedAt: number;
  artifact: NativeReactComponentArtifact;
}

export type FrontstageNativeReactArtifactCacheStore =
  FrontstageIndexedDbRecordStore<FrontstageNativeReactArtifactCacheRecord>;

export type FrontstageNativeReactArtifactCacheReadResult =
  | { status: 'hit'; artifact: NativeReactComponentArtifact }
  | { status: 'miss'; reason: 'not_found' | 'corrupt' | 'identity_mismatch' }
  | { status: 'unavailable'; reason: 'indexeddb_unavailable' | 'read_failed' };

export type FrontstageNativeReactArtifactCacheWriteResult =
  | { status: 'stored'; byteSize: number }
  | {
      status: 'skipped';
      reason: 'invalid_artifact' | 'identity_mismatch' | 'oversized';
    }
  | {
      status: 'unavailable';
      reason: 'indexeddb_unavailable' | 'write_failed' | 'quota_exceeded';
    };

export type FrontstageNativeReactArtifactCacheMaintenanceResult =
  | { status: 'completed'; deleted: number }
  | {
      status: 'unavailable';
      reason: 'indexeddb_unavailable' | 'maintenance_failed';
    };

export interface FrontstageNativeReactArtifactCacheOptions {
  store: FrontstageNativeReactArtifactCacheStore;
  byteBudget?: number;
  now?: () => number;
}

export type FrontstageNativeReactArtifactResolution =
  | { status: 'hit'; artifact: NativeReactComponentArtifact }
  | {
      status: 'compiled';
      artifact: NativeReactComponentArtifact;
      cacheWrite: FrontstageNativeReactArtifactCacheWriteResult;
    }
  | { status: 'compile_failed'; result: NativeReactBrowserCompileResult };

export class FrontstageNativeReactArtifactCache {
  private readonly byteBudget: number;
  private readonly now: () => number;

  constructor(
    private readonly options: FrontstageNativeReactArtifactCacheOptions
  ) {
    this.byteBudget = Math.max(
      1,
      Math.floor(
        options.byteBudget ??
          DEFAULT_FRONTSTAGE_NATIVE_REACT_ARTIFACT_CACHE_BYTE_BUDGET
      )
    );
    this.now = options.now ?? Date.now;
  }

  async get(
    identity: FrontstageNativeReactArtifactCacheIdentity
  ): Promise<FrontstageNativeReactArtifactCacheReadResult> {
    const key = createFrontstageNativeReactArtifactCacheKey(identity);
    let value: unknown;
    try {
      value = await this.options.store.get(key);
    } catch (error) {
      return unavailableRead(error);
    }
    if (value === undefined) return { status: 'miss', reason: 'not_found' };

    const record = canonicalizeFrontstageNativeReactArtifactCacheRecord(value);
    if (!record) {
      await this.deleteWithoutThrow(key);
      return { status: 'miss', reason: 'corrupt' };
    }
    if (!recordMatchesIdentity(record, identity)) {
      await this.deleteWithoutThrow(key);
      return { status: 'miss', reason: 'identity_mismatch' };
    }

    const accessed = createCanonicalRecord(
      identity,
      record.artifact,
      this.now()
    );
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
      // A canonical L2 hit remains usable when access-time persistence fails.
    }
    return { status: 'hit', artifact: record.artifact };
  }

  async put(
    identity: FrontstageNativeReactArtifactCacheIdentity,
    value: unknown
  ): Promise<FrontstageNativeReactArtifactCacheWriteResult> {
    const artifact = canonicalizeNativeReactComponentArtifact(value);
    if (!artifact) return { status: 'skipped', reason: 'invalid_artifact' };
    if (!recordArtifactMatchesIdentity(artifact, identity)) {
      return { status: 'skipped', reason: 'identity_mismatch' };
    }

    const record = createCanonicalRecord(identity, artifact, this.now());
    if (record.byteSize > this.byteBudget) {
      return { status: 'skipped', reason: 'oversized' };
    }

    let records: FrontstageNativeReactArtifactCacheRecord[] = [];
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
  ): Promise<FrontstageNativeReactArtifactCacheMaintenanceResult> {
    try {
      const values = await this.options.store.list();
      const prefix = `${encodeURIComponent(actorId)}/`;
      const keys = values.flatMap((value) => {
        const identity = readRecordIdentity(value);
        if (identity?.actorId === actorId) return [identity.key];
        const rawKey = readRawRecordKey(value);
        return rawKey?.startsWith(prefix) ? [rawKey] : [];
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
  }: {
    actorId: string;
    workspaceId: string;
    runtimeFingerprint: string;
  }): Promise<FrontstageNativeReactArtifactCacheMaintenanceResult> {
    try {
      const values = await this.options.store.list();
      const deleted = new Set<string>();
      const current: FrontstageNativeReactArtifactCacheRecord[] = [];
      const prefix =
        [actorId, workspaceId].map(encodeURIComponent).join('/') + '/';
      for (const value of values) {
        const identity = readRecordIdentity(value);
        if (!identity) {
          const rawKey = readRawRecordKey(value);
          if (rawKey?.startsWith(prefix)) deleted.add(rawKey);
          continue;
        }
        if (
          identity.actorId !== actorId ||
          identity.workspaceId !== workspaceId
        ) {
          continue;
        }
        const record =
          canonicalizeFrontstageNativeReactArtifactCacheRecord(value);
        if (
          !record ||
          record.runtime_fingerprint !== runtimeFingerprint ||
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
    records: FrontstageNativeReactArtifactCacheRecord[],
    incomingBytes: number
  ): Promise<FrontstageNativeReactArtifactCacheRecord[]> {
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
      // Corrupt L2 cleanup must never block Native React execution.
    }
  }
}

export async function resolveFrontstageNativeReactArtifact({
  cache,
  identity,
  compile
}: {
  cache: Pick<FrontstageNativeReactArtifactCache, 'get' | 'put'>;
  identity: FrontstageNativeReactArtifactCacheIdentity;
  compile: () => Promise<NativeReactBrowserCompileResult>;
}): Promise<FrontstageNativeReactArtifactResolution> {
  const cached = await cache.get(identity);
  if (cached.status === 'hit') {
    return { status: 'hit', artifact: cached.artifact };
  }
  const compiled = await compile();
  if (!compiled.ok) return { status: 'compile_failed', result: compiled };
  const cacheWrite = await cache.put(identity, compiled.artifact);
  return { status: 'compiled', artifact: compiled.artifact, cacheWrite };
}

export function createFrontstageNativeReactArtifactCacheIdentity({
  actorId,
  workspaceId,
  source,
  dependencyLock,
  runtimeFingerprint
}: {
  actorId: string;
  workspaceId: string;
  source: string;
  dependencyLock: NativeReactCatalogDependencyLock;
  runtimeFingerprint: string;
}): FrontstageNativeReactArtifactCacheIdentity {
  return {
    actorId,
    workspaceId,
    ...createNativeReactComponentArtifactIdentity({
      sourceSha256: sha256Text(source),
      dependencyLock,
      runtimeFingerprint
    })
  };
}

export function createFrontstageNativeReactArtifactCacheKey(
  identity: FrontstageNativeReactArtifactCacheIdentity
): string {
  return [
    identity.actorId,
    identity.workspaceId,
    identity.runtime_fingerprint,
    identity.compiler_abi,
    identity.runtime_abi,
    identity.dependency_lock_sha256,
    identity.source_sha256
  ]
    .map(encodeURIComponent)
    .join('/');
}

export function canonicalizeFrontstageNativeReactArtifactCacheRecord(
  value: unknown
): FrontstageNativeReactArtifactCacheRecord | null {
  const identity = readRecordIdentity(value);
  if (!identity || !isRecord(value)) return null;
  const artifact = canonicalizeNativeReactComponentArtifact(value.artifact);
  if (
    !artifact ||
    value.schemaVersion !==
      FRONTSTAGE_NATIVE_REACT_ARTIFACT_CACHE_SCHEMA_VERSION ||
    !isNonNegativeNumber(value.lastAccessedAt) ||
    !isPositiveInteger(value.byteSize)
  ) {
    return null;
  }
  const canonical = createCanonicalRecord(
    identity,
    artifact,
    value.lastAccessedAt
  );
  return canonical.byteSize === value.byteSize ? canonical : null;
}

function createCanonicalRecord(
  identity: FrontstageNativeReactArtifactCacheIdentity,
  artifact: NativeReactComponentArtifact,
  lastAccessedAt: number
): FrontstageNativeReactArtifactCacheRecord {
  const base = {
    key: createFrontstageNativeReactArtifactCacheKey(identity),
    schemaVersion: FRONTSTAGE_NATIVE_REACT_ARTIFACT_CACHE_SCHEMA_VERSION,
    actorId: identity.actorId,
    workspaceId: identity.workspaceId,
    source_sha256: identity.source_sha256,
    compiler_abi: identity.compiler_abi,
    runtime_abi: identity.runtime_abi,
    runtime_fingerprint: identity.runtime_fingerprint,
    dependency_lock_sha256: identity.dependency_lock_sha256,
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

function readRecordIdentity(
  value: unknown
): (FrontstageNativeReactArtifactCacheIdentity & { key: string }) | null {
  if (!isRecord(value)) return null;
  if (
    !isNonEmptyString(value.key) ||
    !isNonEmptyString(value.actorId) ||
    !isNonEmptyString(value.workspaceId) ||
    !isSha256(value.source_sha256) ||
    !isNonEmptyString(value.compiler_abi) ||
    !isNonEmptyString(value.runtime_abi) ||
    !isNonEmptyString(value.runtime_fingerprint) ||
    !isSha256(value.dependency_lock_sha256)
  ) {
    return null;
  }
  return {
    key: value.key,
    actorId: value.actorId,
    workspaceId: value.workspaceId,
    source_sha256: value.source_sha256,
    compiler_abi:
      value.compiler_abi as NativeReactComponentArtifactIdentity['compiler_abi'],
    runtime_abi:
      value.runtime_abi as NativeReactComponentArtifactIdentity['runtime_abi'],
    runtime_fingerprint: value.runtime_fingerprint,
    dependency_lock_sha256: value.dependency_lock_sha256
  };
}

function recordMatchesIdentity(
  record: FrontstageNativeReactArtifactCacheRecord,
  identity: FrontstageNativeReactArtifactCacheIdentity
): boolean {
  return (
    record.key === createFrontstageNativeReactArtifactCacheKey(identity) &&
    record.actorId === identity.actorId &&
    record.workspaceId === identity.workspaceId &&
    record.source_sha256 === identity.source_sha256 &&
    record.compiler_abi === identity.compiler_abi &&
    record.runtime_abi === identity.runtime_abi &&
    record.runtime_fingerprint === identity.runtime_fingerprint &&
    record.dependency_lock_sha256 === identity.dependency_lock_sha256 &&
    recordArtifactMatchesIdentity(record.artifact, identity)
  );
}

function recordArtifactMatchesIdentity(
  artifact: NativeReactComponentArtifact,
  identity: FrontstageNativeReactArtifactCacheIdentity
): boolean {
  return nativeReactComponentArtifactMatchesIdentity(artifact, identity);
}

function canonicalRecords(
  values: unknown[]
): FrontstageNativeReactArtifactCacheRecord[] {
  return values
    .map(canonicalizeFrontstageNativeReactArtifactCacheRecord)
    .filter(
      (record): record is FrontstageNativeReactArtifactCacheRecord =>
        record !== null
    );
}

function sortByLru(
  records: FrontstageNativeReactArtifactCacheRecord[]
): FrontstageNativeReactArtifactCacheRecord[] {
  return [...records].sort(
    (left, right) =>
      left.lastAccessedAt - right.lastAccessedAt ||
      compareStableKey(left.key, right.key)
  );
}

function compareStableKey(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

function utf8ByteSize(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function unavailableRead(
  error: unknown
): FrontstageNativeReactArtifactCacheReadResult {
  return {
    status: 'unavailable',
    reason: isIndexedDbUnavailable(error)
      ? 'indexeddb_unavailable'
      : 'read_failed'
  };
}

function unavailableWrite(
  error: unknown
): FrontstageNativeReactArtifactCacheWriteResult {
  return {
    status: 'unavailable',
    reason: isIndexedDbUnavailable(error)
      ? 'indexeddb_unavailable'
      : 'write_failed'
  };
}

function unavailableMaintenance(
  error: unknown
): FrontstageNativeReactArtifactCacheMaintenanceResult {
  return {
    status: 'unavailable',
    reason: isIndexedDbUnavailable(error)
      ? 'indexeddb_unavailable'
      : 'maintenance_failed'
  };
}

function isQuotaExceeded(error: unknown): boolean {
  return isRecord(error) && error.name === 'QuotaExceededError';
}

function isIndexedDbUnavailable(error: unknown): boolean {
  return isRecord(error) && error.name === 'IndexedDbUnavailableError';
}

function readRawRecordKey(value: unknown): string | null {
  return isRecord(value) && isNonEmptyString(value.key) ? value.key : null;
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

function isSha256(value: unknown): value is string {
  return typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
}
