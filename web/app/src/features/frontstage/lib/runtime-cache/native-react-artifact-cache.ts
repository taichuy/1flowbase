import {
  canonicalizeNativeReactComponentArtifact,
  createNativeReactComponentArtifactIdentity,
  nativeReactComponentArtifactMatchesIdentity,
  sha256Text,
  type NativeReactComponentArtifact,
  type NativeReactComponentArtifactIdentity
} from '@1flowbase/page-runtime';

import { LRUCache } from 'lru-cache';
import type { NativeReactArtifactStore } from './native-react-artifact-store';
import type { NativeReactBrowserCompileResult } from '../../../../shared/code-block/native-react-compiler-browser';

export const FRONTSTAGE_NATIVE_REACT_ARTIFACT_CACHE_SCHEMA_VERSION = 3 as const;
export const DEFAULT_FRONTSTAGE_NATIVE_REACT_ARTIFACT_CACHE_BYTE_BUDGET =
  16 * 1024 * 1024;

export interface FrontstageNativeReactArtifactCacheIdentity extends NativeReactComponentArtifactIdentity {
  actorId: string;
  workspaceId: string;
  module_policy_sha256: string;
}

export interface FrontstageNativeReactArtifactCacheRecord extends FrontstageNativeReactArtifactCacheIdentity {
  key: string;
  schemaVersion: typeof FRONTSTAGE_NATIVE_REACT_ARTIFACT_CACHE_SCHEMA_VERSION;
  byteSize: number;
  lastAccessedAt: number;
  artifact: NativeReactComponentArtifact;
}

export type FrontstageNativeReactArtifactCacheStore = NativeReactArtifactStore;

export type FrontstageNativeReactArtifactCacheReadResult =
  | {
      status: 'hit';
      artifact: NativeReactComponentArtifact;
      tier?: 'l1' | 'l2';
    }
  | { status: 'miss'; reason: 'not_found' | 'corrupt' | 'identity_mismatch' }
  | { status: 'unavailable'; reason: 'indexeddb_unavailable' | 'read_failed' };

export type FrontstageNativeReactArtifactCacheWriteResult =
  | { status: 'stored'; byteSize: number }
  | {
      status: 'skipped';
      reason:
        | 'invalid_artifact'
        | 'identity_mismatch'
        | 'oversized'
        | 'superseded';
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

export interface FrontstageArtifactWriteEpoch {
  local: number;
  persisted: number;
}
interface PendingWrite {
  record: FrontstageNativeReactArtifactCacheRecord;
  epoch: Promise<FrontstageArtifactWriteEpoch>;
  resolve(result: FrontstageNativeReactArtifactCacheWriteResult): void;
}

export class FrontstageNativeReactArtifactCache {
  private readonly byteBudget: number;
  private readonly now: () => number;
  private readonly memory: LRUCache<
    string,
    FrontstageNativeReactArtifactCacheRecord
  >;
  private readonly pending = new Map<string, PendingWrite>();
  private readonly touches = new Set<string>();
  private pendingBytes = 0;
  private epoch = 0;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private draining: Promise<void> | undefined;

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
    this.memory = new LRUCache({
      maxSize: this.byteBudget,
      sizeCalculation: (record) => record.byteSize
    });
  }

  /** Capture before source fetch/compile, so logout/pruning cannot admit a late result. */
  async captureWriteEpoch(
    actorId: string
  ): Promise<FrontstageArtifactWriteEpoch> {
    const local = this.epoch;
    const persisted = await this.options.store
      .readEpoch(actorId)
      .catch(() => -1);
    return { local, persisted };
  }

  async get(
    identity: FrontstageNativeReactArtifactCacheIdentity
  ): Promise<FrontstageNativeReactArtifactCacheReadResult> {
    const key = createFrontstageNativeReactArtifactCacheKey(identity);
    const localEpoch = this.epoch;
    const hot = this.memory.get(key);
    if (hot) {
      this.touch(key);
      return { status: 'hit', artifact: hot.artifact, tier: 'l1' };
    }
    let value: unknown;
    try {
      value = await this.options.store.get(key);
    } catch (error) {
      return unavailableRead(error);
    }
    if (value === undefined || localEpoch !== this.epoch)
      return { status: 'miss', reason: 'not_found' };
    const record = canonicalizeFrontstageNativeReactArtifactCacheRecord(value);
    if (!record || record.byteSize > this.byteBudget) {
      void this.options.store.delete(key).catch(() => undefined);
      return { status: 'miss', reason: 'corrupt' };
    }
    if (!recordMatchesIdentity(record, identity)) {
      void this.options.store.delete(key).catch(() => undefined);
      return { status: 'miss', reason: 'identity_mismatch' };
    }
    // Admission validates and copies the artifact. Freeze the owned copy so hits need no rehash.
    deepFreeze(record);
    this.memory.set(key, record);
    this.touch(key);
    return { status: 'hit', artifact: record.artifact, tier: 'l2' };
  }

  async put(
    identity: FrontstageNativeReactArtifactCacheIdentity,
    value: unknown,
    epoch = this.captureWriteEpoch(identity.actorId)
  ): Promise<FrontstageNativeReactArtifactCacheWriteResult> {
    // Attach rejection immediately even when the queued write runs on a later browser turn.
    const guardedEpoch = epoch.catch(() => ({ local: -1, persisted: -1 }));
    const artifact = canonicalizeNativeReactComponentArtifact(value);
    if (!artifact) return { status: 'skipped', reason: 'invalid_artifact' };
    if (!recordArtifactMatchesIdentity(artifact, identity))
      return { status: 'skipped', reason: 'identity_mismatch' };
    const record = createCanonicalRecord(identity, artifact, this.now());
    if (record.byteSize > this.byteBudget)
      return { status: 'skipped', reason: 'oversized' };
    return new Promise((resolve) => {
      const previous = this.pending.get(record.key);
      if (previous) this.dropPending(previous);
      while (this.pendingBytes + record.byteSize > this.byteBudget) {
        const oldest = this.pending.values().next().value;
        if (!oldest) break;
        this.dropPending(oldest);
      }
      this.pending.set(record.key, { record, epoch: guardedEpoch, resolve });
      this.pendingBytes += record.byteSize;
      this.schedule();
    });
  }

  async deleteActor(
    actorId: string
  ): Promise<FrontstageNativeReactArtifactCacheMaintenanceResult> {
    this.invalidate();
    try {
      return {
        status: 'completed',
        deleted: await this.options.store.deleteActor(actorId)
      };
    } catch (error) {
      return unavailableMaintenance(error);
    }
  }

  async pruneWorkspace({
    actorId,
    workspaceId
  }: {
    actorId: string;
    workspaceId: string;
  }): Promise<FrontstageNativeReactArtifactCacheMaintenanceResult> {
    this.invalidate();
    try {
      return {
        status: 'completed',
        deleted: await this.options.store.pruneWorkspace(
          actorId,
          workspaceId,
          this.byteBudget
        )
      };
    } catch (error) {
      return unavailableMaintenance(error);
    }
  }

  /** Explicit test/shutdown barrier; render consumers never await maintenance. */
  async flush(): Promise<void> {
    if (this.timer !== undefined) {
      clearTimeout(this.timer);
      this.timer = undefined;
    }
    if (this.draining) await this.draining;
    if (this.pending.size || this.touches.size) {
      this.draining = this.drain();
      await this.draining;
      this.draining = undefined;
      await this.flush();
    }
  }

  private invalidate() {
    this.epoch += 1;
    this.memory.clear();
    this.touches.clear();
    for (const write of this.pending.values()) this.dropPending(write);
  }

  private dropPending(write: PendingWrite) {
    this.pending.delete(write.record.key);
    this.pendingBytes -= write.record.byteSize;
    write.resolve({ status: 'skipped', reason: 'superseded' });
  }

  private touch(key: string) {
    // Bound and coalesce metadata work independently of the number of hits.
    if (this.touches.size >= 128 && !this.touches.has(key))
      this.touches.delete(this.touches.values().next().value!);
    this.touches.delete(key);
    this.touches.add(key);
    this.schedule();
  }

  private schedule() {
    if (this.timer !== undefined || this.draining) return;
    this.timer = setTimeout(() => {
      this.timer = undefined;
      this.draining = this.drain().finally(() => {
        this.draining = undefined;
        if (this.pending.size || this.touches.size) this.schedule();
      });
    }, 0);
  }

  private async drain(): Promise<void> {
    const keys = [...this.touches].slice(0, 32);
    keys.forEach((key) => this.touches.delete(key));
    if (keys.length)
      await this.options.store.touch(keys, this.now()).catch(() => undefined);
    const write = this.pending.values().next().value;
    if (!write) return;
    this.pending.delete(write.record.key);
    this.pendingBytes -= write.record.byteSize;
    try {
      const epoch = await write.epoch;
      if (epoch.local !== this.epoch) {
        write.resolve({ status: 'skipped', reason: 'superseded' });
        return;
      }
      if (epoch.persisted < 0) {
        deepFreeze(write.record);
        this.memory.set(write.record.key, write.record);
        write.resolve({
          status: 'unavailable',
          reason: 'indexeddb_unavailable'
        });
        return;
      }
      const stored = await this.options.store.commit(
        write.record,
        this.byteBudget,
        epoch.persisted
      );
      if (stored && epoch.local === this.epoch) {
        deepFreeze(write.record);
        this.memory.set(write.record.key, write.record);
      }
      write.resolve(
        stored
          ? { status: 'stored', byteSize: write.record.byteSize }
          : { status: 'skipped', reason: 'superseded' }
      );
    } catch (error) {
      write.resolve(
        isQuotaExceeded(error)
          ? { status: 'unavailable', reason: 'quota_exceeded' }
          : unavailableWrite(error)
      );
    }
  }
}

function deepFreeze(value: object): void {
  for (const child of Object.values(value))
    if (typeof child === 'object' && child !== null) deepFreeze(child);
  Object.freeze(value);
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
  modulePolicySha256 = sha256Text('[]')
}: {
  actorId: string;
  workspaceId: string;
  source: string;
  modulePolicySha256?: string;
}): FrontstageNativeReactArtifactCacheIdentity {
  return {
    actorId,
    workspaceId,
    module_policy_sha256: modulePolicySha256,
    ...createNativeReactComponentArtifactIdentity({
      sourceSha256: sha256Text(source)
    })
  };
}

export function createFrontstageNativeReactArtifactCacheKey(
  identity: FrontstageNativeReactArtifactCacheIdentity
): string {
  return [
    identity.actorId,
    identity.workspaceId,
    identity.compiler_abi,
    identity.runtime_abi,
    identity.source_sha256,
    identity.module_policy_sha256
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
    module_policy_sha256: identity.module_policy_sha256,
    source_sha256: identity.source_sha256,
    compiler_abi: identity.compiler_abi,
    runtime_abi: identity.runtime_abi,
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
    !isSha256(value.module_policy_sha256) ||
    !isNonEmptyString(value.compiler_abi) ||
    !isNonEmptyString(value.runtime_abi)
  ) {
    return null;
  }
  return {
    key: value.key,
    actorId: value.actorId,
    workspaceId: value.workspaceId,
    module_policy_sha256: value.module_policy_sha256,
    source_sha256: value.source_sha256,
    compiler_abi:
      value.compiler_abi as NativeReactComponentArtifactIdentity['compiler_abi'],
    runtime_abi:
      value.runtime_abi as NativeReactComponentArtifactIdentity['runtime_abi']
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
    record.module_policy_sha256 === identity.module_policy_sha256 &&
    record.source_sha256 === identity.source_sha256 &&
    record.compiler_abi === identity.compiler_abi &&
    record.runtime_abi === identity.runtime_abi &&
    recordArtifactMatchesIdentity(record.artifact, identity)
  );
}

function recordArtifactMatchesIdentity(
  artifact: NativeReactComponentArtifact,
  identity: FrontstageNativeReactArtifactCacheIdentity
): boolean {
  return nativeReactComponentArtifactMatchesIdentity(artifact, identity);
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
