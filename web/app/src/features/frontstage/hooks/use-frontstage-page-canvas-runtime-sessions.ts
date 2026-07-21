import type { JsBlockHostEffectHandlers } from '@1flowbase/page-runtime';
import type { Dispatch, SetStateAction } from 'react';
import { useEffect, useMemo, useRef, useState } from 'react';

import {
  createFrontstageRestrictedBlockRuntimeSession,
  type FrontstageRestrictedBlockRuntimeHostOptions,
  type FrontstageRestrictedBlockRuntimeSession
} from '../lib/frontstage-restricted-block-runtime-host';
import type {
  FrontstagePageCanvasRuntimeRunPlanItem,
  FrontstagePageCanvasRuntimeRunPlanReadyItem,
  FrontstagePageCanvasRuntimeRunPlanState
} from '../lib/page-canvas/runtime-run-plan';
import type {
  RestrictedBlockRuntimeHostSnapshot,
  RestrictedBlockRuntimeHostSnapshotStatus
} from '../lib/restricted-block-runtime-host';
import {
  resolveFrontstageRuntimeDemand,
  type FrontstageRuntimeDemandByBlockId
} from '../lib/page-canvas/runtime-demand';
import type { FrontstageBlockInstance } from '../lib/page-document';
import {
  createFrontstagePageSignalSession,
  FrontstageSignalRuntimeCoordinator,
  type FrontstagePageSignalSession
} from '../lib/page-canvas/signal-runtime';
import {
  recordFrontstageRuntimeObservation,
  type FrontstageRuntimeObservationStage
} from '../lib/page-canvas/runtime-observation';
import {
  frontstageCompiledArtifactCache,
  type FrontstageCompiledArtifactCache
} from '../lib/runtime-cache';
import { getFrontstageRestrictedBlockRuntimeFingerprint } from '../lib/restricted-block-worker-factory';

export type FrontstagePageCanvasRuntimeSessionFactory = (
  options: FrontstageRestrictedBlockRuntimeHostOptions
) => FrontstageRestrictedBlockRuntimeSession;

export type FrontstagePageCanvasRuntimeSessionSkippedReason = Exclude<
  FrontstagePageCanvasRuntimeRunPlanItem['status'],
  'run_plan_ready'
>;

export type FrontstagePageCanvasRuntimeSessionEntryStatus =
  | RestrictedBlockRuntimeHostSnapshotStatus
  | 'skipped'
  | 'factory_failed';

export interface FrontstagePageCanvasRuntimeSessionEntryBase {
  blockId: string;
  sourceBlockId: string | null;
  codeRef: string;
  sourceCodeRef: string | null;
  sourceIndex: number;
  slotIndex: number;
  sourceStatus: FrontstagePageCanvasRuntimeRunPlanItem['sourceStatus'];
  runPlanStatus: FrontstagePageCanvasRuntimeRunPlanItem['status'];
}

export interface FrontstagePageCanvasRuntimeSessionSnapshotEntry extends FrontstagePageCanvasRuntimeSessionEntryBase {
  status: RestrictedBlockRuntimeHostSnapshotStatus;
  snapshot: RestrictedBlockRuntimeHostSnapshot;
}

export interface FrontstagePageCanvasRuntimeSessionSkippedEntry extends FrontstagePageCanvasRuntimeSessionEntryBase {
  status: 'skipped';
  skipReason: FrontstagePageCanvasRuntimeSessionSkippedReason;
  message: string;
  path: string;
}

export interface FrontstagePageCanvasRuntimeSessionFactoryFailedEntry extends FrontstagePageCanvasRuntimeSessionEntryBase {
  status: 'factory_failed';
  message: string;
  error: Error;
}

export type FrontstagePageCanvasRuntimeSessionEntry =
  | FrontstagePageCanvasRuntimeSessionSnapshotEntry
  | FrontstagePageCanvasRuntimeSessionSkippedEntry
  | FrontstagePageCanvasRuntimeSessionFactoryFailedEntry;

export interface UseFrontstagePageCanvasRuntimeSessionsInput {
  actorId: string | null | undefined;
  actorWorkspaceId: string | null | undefined;
  runtimeRunPlanState:
    | FrontstagePageCanvasRuntimeRunPlanState
    | null
    | undefined;
  runtimeSessionFactory?: FrontstagePageCanvasRuntimeSessionFactory;
  handlers?: JsBlockHostEffectHandlers;
  demandsByBlockId?: FrontstageRuntimeDemandByBlockId;
  maxConcurrent?: number;
  blocks?: readonly FrontstageBlockInstance[];
  tabId?: string | null;
  runtimeResultCache?: FrontstageRuntimeResultCache;
  artifactCache?: Pick<FrontstageCompiledArtifactCache, 'put'>;
  runtimeFingerprint?: string;
}

export interface UseFrontstagePageCanvasRuntimeSessionsResult {
  entries: FrontstagePageCanvasRuntimeSessionEntry[];
  snapshotsBySlot: Readonly<Record<number, RestrictedBlockRuntimeHostSnapshot>>;
  running: boolean;
  hasError: boolean;
  retryBlock(blockId: string): void;
}

interface ActiveRuntimeSession {
  session: FrontstageRestrictedBlockRuntimeSession;
  unsubscribe: () => void;
  snapshot: RestrictedBlockRuntimeHostSnapshot;
  executing: boolean;
  observedStage: FrontstageRuntimeObservationStage;
  observedAtMs: number;
  cacheTier: 'l2' | 'miss';
}

interface RuntimeObservationContext {
  actorId: string;
  workspaceId: string;
  pageId: string;
  tabId: string | null;
  blockId: string;
}

const DEFAULT_RUNTIME_RESULT_CACHE_BYTE_BUDGET = 4 * 1024 * 1024;
const EMPTY_BLOCKS: readonly FrontstageBlockInstance[] = [];

export type FrontstageCachedBlockResult = Pick<
  RestrictedBlockRuntimeHostSnapshot,
  'view' | 'outputs' | 'schemaValidationOptions'
>;

interface FrontstageCachedBlockResultEntry {
  value: FrontstageCachedBlockResult;
  byteWeight: number;
}

export class FrontstageRuntimeResultCache {
  readonly byteBudget: number;
  private readonly entries = new Map<
    string,
    FrontstageCachedBlockResultEntry
  >();
  private usedBytes = 0;

  constructor(byteBudget = DEFAULT_RUNTIME_RESULT_CACHE_BYTE_BUDGET) {
    if (!Number.isSafeInteger(byteBudget) || byteBudget < 0) {
      throw new Error(
        'runtime result cache byte budget must be a non-negative integer'
      );
    }
    this.byteBudget = byteBudget;
  }

  get byteSize(): number {
    return this.usedBytes;
  }

  get size(): number {
    return this.entries.size;
  }

  get(sessionKey: string): FrontstageCachedBlockResult | undefined {
    const cached = this.entries.get(sessionKey);
    if (!cached) {
      return undefined;
    }
    this.entries.delete(sessionKey);
    this.entries.set(sessionKey, cached);
    return cached.value;
  }

  set(sessionKey: string, value: FrontstageCachedBlockResult): void {
    this.delete(sessionKey);
    const byteWeight = utf8ByteLength(stableSerialize([sessionKey, value]));
    if (byteWeight > this.byteBudget) {
      return;
    }

    while (this.usedBytes + byteWeight > this.byteBudget) {
      const leastRecentlyUsedKey = this.entries.keys().next().value as
        | string
        | undefined;
      if (leastRecentlyUsedKey === undefined) {
        break;
      }
      this.delete(leastRecentlyUsedKey);
    }

    this.entries.set(sessionKey, { value, byteWeight });
    this.usedBytes += byteWeight;
  }

  delete(sessionKey: string): void {
    const cached = this.entries.get(sessionKey);
    if (!cached) {
      return;
    }
    this.entries.delete(sessionKey);
    this.usedBytes -= cached.byteWeight;
  }

  clear(): void {
    this.entries.clear();
    this.usedBytes = 0;
  }
}

export const frontstageRuntimeResultCache =
  new FrontstageRuntimeResultCache();

export function clearFrontstageRuntimeSessionCache(): void {
  frontstageRuntimeResultCache.clear();
}

export function readFrontstageRuntimeSessionCacheSize(): number {
  return frontstageRuntimeResultCache.size;
}

type InternalRuntimeSessionEntry = FrontstagePageCanvasRuntimeSessionEntry & {
  sessionKey?: string;
};

export function useFrontstagePageCanvasRuntimeSessions({
  actorId,
  actorWorkspaceId,
  runtimeRunPlanState,
  runtimeSessionFactory = createFrontstageRestrictedBlockRuntimeSession,
  handlers,
  demandsByBlockId,
  maxConcurrent = 2,
  blocks = EMPTY_BLOCKS,
  tabId = null,
  runtimeResultCache: resultCache = frontstageRuntimeResultCache,
  artifactCache = frontstageCompiledArtifactCache,
  runtimeFingerprint = getFrontstageRestrictedBlockRuntimeFingerprint()
}: UseFrontstagePageCanvasRuntimeSessionsInput): UseFrontstagePageCanvasRuntimeSessionsResult {
  const activeRuntimeSessionsRef = useRef(
    new Map<string, ActiveRuntimeSession>()
  );
  const pageSignalSessionsRef = useRef(
    new Map<string, FrontstagePageSignalSession>()
  );
  const restoredSignalSessionKeysRef = useRef(new Set<string>());
  const restoredObservationSessionKeysRef = useRef(new Set<string>());
  const [internalEntries, setInternalEntries] = useState<
    InternalRuntimeSessionEntry[]
  >([]);
  const [runtimeRevision, setRuntimeRevision] = useState(0);
  const [signalRevision, setSignalRevision] = useState(0);
  const signalPageId = runtimeRunPlanState?.pageId ?? null;
  const signalCoordinator = useMemo(() => {
    if (!tabId || !signalPageId) return null;
    let session = pageSignalSessionsRef.current.get(signalPageId);
    if (!session) {
      session = createFrontstagePageSignalSession();
      pageSignalSessionsRef.current.set(signalPageId, session);
    }
    return new FrontstageSignalRuntimeCoordinator(blocks, tabId, session);
  }, [blocks, signalPageId, tabId]);
  const [pageVisible, setPageVisible] = useState(
    () =>
      typeof document === 'undefined' || document.visibilityState !== 'hidden'
  );

  useEffect(() => {
    if (typeof document === 'undefined') {
      return;
    }
    const handleVisibilityChange = () => {
      setPageVisible(document.visibilityState !== 'hidden');
    };
    document.addEventListener('visibilitychange', handleVisibilityChange);
    return () =>
      document.removeEventListener('visibilitychange', handleVisibilityChange);
  }, []);

  useEffect(() => {
    const activeRuntimeSessions = activeRuntimeSessionsRef.current;

    if (
      !runtimeRunPlanState ||
      !actorId ||
      actorWorkspaceId !== runtimeRunPlanState.workspaceId
    ) {
      disposeAllRuntimeSessions(activeRuntimeSessions);
      setInternalEntries((currentEntries) =>
        currentEntries.length === 0 ? currentEntries : []
      );
      return;
    }

    const nextSessionKeys = new Set<string>();
    const readyItems: Array<{
      item: FrontstagePageCanvasRuntimeRunPlanReadyItem;
      sessionKey: string;
    }> = [];

    for (const item of runtimeRunPlanState.items) {
      if (item.status === 'run_plan_ready') {
        const sessionKey = createRuntimeSessionKey(
          runtimeRunPlanState,
          item,
          actorId,
          tabId,
          signalCoordinator?.inputSignature(item.blockId)
        );
        nextSessionKeys.add(sessionKey);
        readyItems.push({ item, sessionKey });
      }
    }

    for (const [sessionKey, activeRuntimeSession] of [
      ...activeRuntimeSessions
    ]) {
      if (!nextSessionKeys.has(sessionKey)) {
        disposeRuntimeSession(
          activeRuntimeSessions,
          sessionKey,
          activeRuntimeSession
        );
      }
    }

    for (const restoredSessionKey of restoredSignalSessionKeysRef.current) {
      if (!nextSessionKeys.has(restoredSessionKey)) {
        restoredSignalSessionKeysRef.current.delete(restoredSessionKey);
      }
    }
    for (const restoredSessionKey of restoredObservationSessionKeysRef.current) {
      if (!nextSessionKeys.has(restoredSessionKey)) {
        restoredObservationSessionKeysRef.current.delete(restoredSessionKey);
      }
    }

    const restoredSnapshots = new Map<
      string,
      RestrictedBlockRuntimeHostSnapshot
    >();
    let didRestoreSignalOutputs = false;
    for (const { item, sessionKey } of readyItems) {
      if (activeRuntimeSessions.has(sessionKey)) {
        continue;
      }
      const cachedResult = resultCache.get(sessionKey);
      if (!cachedResult) {
        continue;
      }

      restoredSnapshots.set(
        sessionKey,
        createRestoredSnapshot(item, cachedResult)
      );
      if (!restoredObservationSessionKeysRef.current.has(sessionKey)) {
        restoredObservationSessionKeysRef.current.add(sessionKey);
        recordFrontstageRuntimeObservation({
          stage: 'present',
          cacheTier: 'l1',
          actorId,
          workspaceId: runtimeRunPlanState.workspaceId,
          pageId: runtimeRunPlanState.pageId,
          tabId,
          blockId: item.blockId
        });
      }
      if (
        cachedResult.outputs &&
        !restoredSignalSessionKeysRef.current.has(sessionKey)
      ) {
        restoredSignalSessionKeysRef.current.add(sessionKey);
        signalCoordinator?.beginRun(item.blockId, sessionKey);
        const committed = signalCoordinator?.commit(
          item.blockId,
          sessionKey,
          cachedResult.outputs
        );
        if (committed?.ok) {
          didRestoreSignalOutputs = true;
          setSignalRevision((revision) => revision + 1);
        }
      }
    }

    const createdEntries = new Map<string, InternalRuntimeSessionEntry>();
    if (pageVisible && !didRestoreSignalOutputs) {
      const runningCount = [...activeRuntimeSessions.values()].filter(
        (session) => session.executing
      ).length;
      const candidates = readyItems
        .filter(({ sessionKey }) => !activeRuntimeSessions.has(sessionKey))
        .filter(({ sessionKey }) => !restoredSnapshots.has(sessionKey))
        .filter(({ item }) => signalCoordinator?.canRun(item.blockId) ?? true)
        .sort((left, right) => {
          const priorityDifference =
            resolveFrontstageRuntimeDemand(
              demandsByBlockId,
              left.item.blockId,
              left.item.slotIndex
            ) -
            resolveFrontstageRuntimeDemand(
              demandsByBlockId,
              right.item.blockId,
              right.item.slotIndex
            );
          return (
            priorityDifference || left.item.slotIndex - right.item.slotIndex
          );
        })
        .slice(0, Math.max(0, maxConcurrent - runningCount));

      for (const { item, sessionKey } of candidates) {
        const createdEntry = createAndRunRuntimeSession({
          item,
          sessionKey,
          runtimeSessionFactory,
          handlers,
          activeRuntimeSessions,
          setInternalEntries,
          setRuntimeRevision,
          setSignalRevision,
          signalCoordinator,
          resultCache,
          artifactCache,
          runtimeFingerprint,
          observationContext: {
            actorId,
            workspaceId: runtimeRunPlanState.workspaceId,
            pageId: runtimeRunPlanState.pageId,
            tabId,
            blockId: item.blockId
          }
        });
        createdEntries.set(sessionKey, createdEntry);
      }
    }

    const nextEntries: InternalRuntimeSessionEntry[] =
      runtimeRunPlanState.items.map((item) => {
        if (item.status !== 'run_plan_ready') {
          return createSkippedEntry(item);
        }
        const sessionKey = createRuntimeSessionKey(
          runtimeRunPlanState,
          item,
          actorId,
          tabId,
          signalCoordinator?.inputSignature(item.blockId)
        );
        const createdEntry = createdEntries.get(sessionKey);
        if (createdEntry?.status === 'factory_failed') {
          return createdEntry;
        }
        const active = activeRuntimeSessions.get(sessionKey);
        const snapshot =
          active?.snapshot ?? restoredSnapshots.get(sessionKey);
        return snapshot
          ? { ...createSnapshotEntry(item, snapshot), sessionKey }
          : createQueuedEntry(item, sessionKey);
      });

    setInternalEntries((currentEntries) =>
      areInternalEntriesEqual(currentEntries, nextEntries)
        ? currentEntries
        : nextEntries
    );
  }, [
    actorId,
    actorWorkspaceId,
    artifactCache,
    demandsByBlockId,
    handlers,
    maxConcurrent,
    pageVisible,
    runtimeRevision,
    resultCache,
    signalCoordinator,
    signalRevision,
    tabId,
    runtimeRunPlanState,
    runtimeSessionFactory,
    runtimeFingerprint
  ]);

  useEffect(
    () => () => {
      disposeAllRuntimeSessions(activeRuntimeSessionsRef.current);
    },
    []
  );

  useEffect(
    () => () => {
      if (!signalPageId) return;
      pageSignalSessionsRef.current.delete(signalPageId);
    },
    [signalPageId]
  );

  const entries = useMemo(
    () => internalEntries.map(toPublicEntry),
    [internalEntries]
  );
  const snapshotsBySlot = useMemo(
    () => createSnapshotsBySlot(entries),
    [entries]
  );
  const running = useMemo(
    () => entries.some((entry) => entry.status === 'running'),
    [entries]
  );
  const hasError = useMemo(
    () => entries.some((entry) => isErrorEntry(entry)),
    [entries]
  );
  const retryBlock = (blockId: string) => {
    const activeRuntimeSessions = activeRuntimeSessionsRef.current;
    for (const entry of internalEntries) {
      if (entry.blockId !== blockId || !entry.sessionKey) {
        continue;
      }
      resultCache.delete(entry.sessionKey);
      restoredSignalSessionKeysRef.current.delete(entry.sessionKey);
      restoredObservationSessionKeysRef.current.delete(entry.sessionKey);
      const activeRuntimeSession = activeRuntimeSessions.get(entry.sessionKey);
      if (activeRuntimeSession) {
        disposeRuntimeSession(
          activeRuntimeSessions,
          entry.sessionKey,
          activeRuntimeSession
        );
      }
    }
    setRuntimeRevision((revision) => revision + 1);
  };

  return {
    entries,
    snapshotsBySlot,
    running,
    hasError,
    retryBlock
  };
}

function createAndRunRuntimeSession({
  item,
  sessionKey,
  runtimeSessionFactory,
  handlers,
  activeRuntimeSessions,
  setInternalEntries,
  setRuntimeRevision,
  setSignalRevision,
  signalCoordinator,
  resultCache,
  artifactCache,
  runtimeFingerprint,
  observationContext
}: {
  item: FrontstagePageCanvasRuntimeRunPlanReadyItem;
  sessionKey: string;
  runtimeSessionFactory: FrontstagePageCanvasRuntimeSessionFactory;
  handlers: JsBlockHostEffectHandlers | undefined;
  activeRuntimeSessions: Map<string, ActiveRuntimeSession>;
  setInternalEntries: Dispatch<SetStateAction<InternalRuntimeSessionEntry[]>>;
  setRuntimeRevision: Dispatch<SetStateAction<number>>;
  setSignalRevision: Dispatch<SetStateAction<number>>;
  signalCoordinator: FrontstageSignalRuntimeCoordinator | null;
  resultCache: FrontstageRuntimeResultCache;
  artifactCache: Pick<FrontstageCompiledArtifactCache, 'put'>;
  runtimeFingerprint: string;
  observationContext: RuntimeObservationContext;
}): InternalRuntimeSessionEntry {
  let session: FrontstageRestrictedBlockRuntimeSession | null = null;
  let unsubscribe: (() => void) | null = null;

  try {
    const runtimeStartedAt = Date.now();
    const cacheTier =
      item.runPlan.request.program.kind === 'compiled_artifact'
        ? 'l2'
        : 'miss';
    recordFrontstageRuntimeObservation({
      ...observationContext,
      stage: 'worker_boot',
      cacheTier,
      timestampMs: runtimeStartedAt
    });
    const runPlan = signalCoordinator
      ? {
          ...item.runPlan,
          request: {
            ...item.runPlan.request,
            inputs: signalCoordinator.inputsFor(item.blockId)
          }
        }
      : item.runPlan;
    signalCoordinator?.beginRun(item.blockId, sessionKey);
    const runtimeOptions: FrontstageRestrictedBlockRuntimeHostOptions = {
      runPlan,
      runtimeFingerprint
    };

    if (handlers) {
      runtimeOptions.handlers = handlers;
    }

    session = runtimeSessionFactory(runtimeOptions);
    unsubscribe = session.subscribe((snapshot) => {
      const activeRuntimeSession = activeRuntimeSessions.get(sessionKey);
      if (!activeRuntimeSession || activeRuntimeSession.session !== session) {
        return;
      }

      activeRuntimeSession.snapshot = snapshot;
      activeRuntimeSession.executing = snapshot.status === 'running';
      observeRuntimeSnapshot(
        activeRuntimeSession,
        snapshot,
        observationContext
      );
      if (snapshot.status === 'ready') {
        resultCache.set(sessionKey, toCachedBlockResult(snapshot));
        if (snapshot.compiledArtifact) {
          void artifactCache
            .put(
              {
                actorId: observationContext.actorId,
                workspaceId: observationContext.workspaceId,
                runtimeFingerprint,
                sourceSha256: item.source_sha256
              },
              snapshot.compiledArtifact
            )
            .catch(() => undefined);
        }
        if (snapshot.outputs) {
          const committed = signalCoordinator?.commit(
            item.blockId,
            sessionKey,
            snapshot.outputs
          );
          if (committed?.ok) setSignalRevision((revision) => revision + 1);
        }
      }
      setInternalEntries((currentEntries) =>
        updateInternalEntries(currentEntries, sessionKey, {
          ...createSnapshotEntry(item, snapshot),
          sessionKey
        })
      );
      if (snapshot.status !== 'running') {
        setRuntimeRevision((revision) => revision + 1);
      }
    });

    const snapshot = session.run();
    const activeRuntimeSession: ActiveRuntimeSession = {
      session,
      unsubscribe,
      snapshot,
      executing: true,
      observedStage: 'worker_boot',
      observedAtMs: runtimeStartedAt,
      cacheTier
    };
    activeRuntimeSessions.set(sessionKey, activeRuntimeSession);
    observeRuntimeSnapshot(activeRuntimeSession, snapshot, observationContext);

    return {
      ...createSnapshotEntry(item, snapshot),
      sessionKey
    };
  } catch (error) {
    if (unsubscribe) {
      unsubscribe();
    }
    if (session) {
      session.dispose();
    }
    activeRuntimeSessions.delete(sessionKey);

    return createFactoryFailedEntry(item, toError(error));
  }
}

function createQueuedEntry(
  item: FrontstagePageCanvasRuntimeRunPlanReadyItem,
  sessionKey: string
): InternalRuntimeSessionEntry {
  return {
    ...createSnapshotEntry(item, {
      status: 'running',
      phase: 'queued',
      requestId: item.runPlan.request.requestId,
      blockId: item.blockId,
      schemaValidationOptions: item.runPlan.schemaValidationOptions,
      logs: [],
      effects: [],
      rejections: []
    }),
    sessionKey
  };
}

function updateInternalEntries(
  entries: InternalRuntimeSessionEntry[],
  sessionKey: string,
  nextEntry: InternalRuntimeSessionEntry
): InternalRuntimeSessionEntry[] {
  let didUpdate = false;
  const nextEntries = entries.map((entry) => {
    if (entry.sessionKey !== sessionKey) {
      return entry;
    }

    didUpdate = true;
    return areInternalEntriesEqual([entry], [nextEntry]) ? entry : nextEntry;
  });

  return didUpdate ? nextEntries : entries;
}

function areInternalEntriesEqual(
  currentEntries: readonly InternalRuntimeSessionEntry[],
  nextEntries: readonly InternalRuntimeSessionEntry[]
): boolean {
  if (currentEntries.length !== nextEntries.length) {
    return false;
  }

  return currentEntries.every((entry, index) =>
    isInternalEntryEqual(entry, nextEntries[index])
  );
}

function isInternalEntryEqual(
  currentEntry: InternalRuntimeSessionEntry,
  nextEntry: InternalRuntimeSessionEntry
): boolean {
  if (
    currentEntry.sessionKey !== nextEntry.sessionKey ||
    currentEntry.status !== nextEntry.status ||
    currentEntry.runPlanStatus !== nextEntry.runPlanStatus ||
    currentEntry.blockId !== nextEntry.blockId ||
    currentEntry.sourceBlockId !== nextEntry.sourceBlockId ||
    currentEntry.codeRef !== nextEntry.codeRef ||
    currentEntry.sourceCodeRef !== nextEntry.sourceCodeRef ||
    currentEntry.sourceIndex !== nextEntry.sourceIndex ||
    currentEntry.slotIndex !== nextEntry.slotIndex
  ) {
    return false;
  }

  if ('snapshot' in currentEntry || 'snapshot' in nextEntry) {
    return (
      'snapshot' in currentEntry &&
      'snapshot' in nextEntry &&
      (currentEntry.snapshot === nextEntry.snapshot ||
        stableSerialize(currentEntry.snapshot) ===
          stableSerialize(nextEntry.snapshot))
    );
  }

  if (currentEntry.status === 'skipped' || nextEntry.status === 'skipped') {
    return (
      currentEntry.status === 'skipped' &&
      nextEntry.status === 'skipped' &&
      currentEntry.skipReason === nextEntry.skipReason &&
      currentEntry.message === nextEntry.message &&
      currentEntry.path === nextEntry.path
    );
  }

  return (
    currentEntry.status === 'factory_failed' &&
    nextEntry.status === 'factory_failed' &&
    currentEntry.message === nextEntry.message &&
    currentEntry.error === nextEntry.error
  );
}

function createSnapshotEntry(
  item: FrontstagePageCanvasRuntimeRunPlanReadyItem,
  snapshot: RestrictedBlockRuntimeHostSnapshot
): FrontstagePageCanvasRuntimeSessionSnapshotEntry {
  return {
    ...createBaseEntry(item),
    status: snapshot.status,
    snapshot
  };
}

function createSkippedEntry(
  item: Exclude<
    FrontstagePageCanvasRuntimeRunPlanItem,
    FrontstagePageCanvasRuntimeRunPlanReadyItem
  >
): FrontstagePageCanvasRuntimeSessionSkippedEntry {
  const issue = item.status === 'rejected' ? item.rejection : item.reason;

  return {
    ...createBaseEntry(item),
    status: 'skipped',
    skipReason: item.status,
    message: issue.message,
    path: issue.path
  };
}

function createFactoryFailedEntry(
  item: FrontstagePageCanvasRuntimeRunPlanReadyItem,
  error: Error
): FrontstagePageCanvasRuntimeSessionFactoryFailedEntry {
  return {
    ...createBaseEntry(item),
    status: 'factory_failed',
    message: error.message,
    error
  };
}

function createBaseEntry(
  item: FrontstagePageCanvasRuntimeRunPlanItem
): FrontstagePageCanvasRuntimeSessionEntryBase {
  return {
    blockId: item.blockId,
    sourceBlockId: item.sourceBlockId,
    codeRef: item.codeRef,
    sourceCodeRef: item.sourceCodeRef,
    sourceIndex: item.sourceIndex,
    slotIndex: item.slotIndex,
    sourceStatus: item.sourceStatus,
    runPlanStatus: item.status
  };
}

function createSnapshotsBySlot(
  entries: readonly FrontstagePageCanvasRuntimeSessionEntry[]
): Readonly<Record<number, RestrictedBlockRuntimeHostSnapshot>> {
  const snapshotsBySlot: Record<number, RestrictedBlockRuntimeHostSnapshot> =
    {};

  for (const entry of entries) {
    if ('snapshot' in entry) {
      snapshotsBySlot[entry.slotIndex] = entry.snapshot;
    }
  }

  return snapshotsBySlot;
}

function isErrorEntry(entry: FrontstagePageCanvasRuntimeSessionEntry): boolean {
  if (entry.status === 'factory_failed') {
    return true;
  }

  if (entry.status === 'skipped') {
    return (
      entry.skipReason !== 'source_not_ready' &&
      entry.skipReason !== 'artifact_lookup_pending'
    );
  }

  return entry.status === 'failed' || entry.status === 'timed_out';
}

function toPublicEntry(
  entry: InternalRuntimeSessionEntry
): FrontstagePageCanvasRuntimeSessionEntry {
  const publicEntry = { ...entry };
  delete (publicEntry as Partial<InternalRuntimeSessionEntry>).sessionKey;
  return publicEntry;
}

function toError(error: unknown): Error {
  return error instanceof Error
    ? error
    : new Error('frontstage page canvas runtime session failed');
}

function disposeAllRuntimeSessions(
  activeRuntimeSessions: Map<string, ActiveRuntimeSession>
): void {
  for (const [sessionKey, activeRuntimeSession] of [...activeRuntimeSessions]) {
    disposeRuntimeSession(
      activeRuntimeSessions,
      sessionKey,
      activeRuntimeSession
    );
  }
}

function disposeRuntimeSession(
  activeRuntimeSessions: Map<string, ActiveRuntimeSession>,
  sessionKey: string,
  activeRuntimeSession: ActiveRuntimeSession
): void {
  activeRuntimeSessions.delete(sessionKey);
  activeRuntimeSession.unsubscribe();
  activeRuntimeSession.session.dispose();
}

function observeRuntimeSnapshot(
  activeRuntimeSession: ActiveRuntimeSession,
  snapshot: RestrictedBlockRuntimeHostSnapshot,
  context: RuntimeObservationContext
): void {
  const stage = runtimeObservationStage(snapshot);
  if (!stage || stage === activeRuntimeSession.observedStage) {
    return;
  }
  const timestampMs = Date.now();
  recordFrontstageRuntimeObservation({
    ...context,
    stage,
    cacheTier: activeRuntimeSession.cacheTier,
    timestampMs,
    durationMs: Math.max(0, timestampMs - activeRuntimeSession.observedAtMs)
  });
  activeRuntimeSession.observedStage = stage;
  activeRuntimeSession.observedAtMs = timestampMs;
}

function runtimeObservationStage(
  snapshot: RestrictedBlockRuntimeHostSnapshot
): FrontstageRuntimeObservationStage | null {
  if (snapshot.status === 'ready') {
    return 'present';
  }
  switch (snapshot.phase) {
    case 'starting':
      return 'worker_boot';
    case 'compiling':
      return 'compile';
    case 'waiting_effect':
      return 'api_wait';
    case 'executing':
      return 'main';
    case 'validating_schema':
      return 'schema_validate';
    default:
      return null;
  }
}

function createRuntimeSessionKey(
  state: FrontstagePageCanvasRuntimeRunPlanState,
  item: FrontstagePageCanvasRuntimeRunPlanReadyItem,
  actorId: string,
  tabId: string | null,
  inputSignature?: string
): string {
  const request = item.runPlan.request;
  return stableSerialize({
    actorId,
    workspaceId: state.workspaceId,
    pageId: state.pageId,
    tabId,
    blockId: item.blockId,
    sourceBlockId: item.sourceBlockId,
    codeRef: item.codeRef,
    sourceCodeRef: item.sourceCodeRef,
    source_sha256: item.source_sha256,
    runtime: {
      kind: item.runtimeKind,
      entry: item.runtimeEntry
    },
    catalog: {
      id: item.catalogId,
      contributionCode: item.contributionCode
    },
    dependencies: {
      props: request.props,
      state: request.state,
      contextSnapshot: request.contextSnapshot,
      inputs: request.inputs,
      signalInputs: inputSignature ?? '',
      limits: request.limits,
      allowedImports:
        request.program.kind === 'source'
          ? request.program.allowedImports
          : request.program.fallback.allowedImports,
      schemaValidationOptions: item.runPlan.schemaValidationOptions,
      mediatorPolicy: item.runPlan.mediatorPolicy
    }
  });
}

function stableSerialize(value: unknown): string {
  return JSON.stringify(sortSerializableValue(value));
}

function toCachedBlockResult(
  snapshot: RestrictedBlockRuntimeHostSnapshot
): FrontstageCachedBlockResult {
  return {
    view: snapshot.view,
    outputs: snapshot.outputs,
    schemaValidationOptions: snapshot.schemaValidationOptions
  };
}

function createRestoredSnapshot(
  item: FrontstagePageCanvasRuntimeRunPlanReadyItem,
  cachedResult: FrontstageCachedBlockResult
): RestrictedBlockRuntimeHostSnapshot {
  return {
    status: 'ready',
    requestId: item.runPlan.request.requestId,
    blockId: item.blockId,
    schemaValidationOptions: cachedResult.schemaValidationOptions,
    view: cachedResult.view,
    outputs: cachedResult.outputs,
    logs: [],
    effects: [],
    rejections: []
  };
}

function utf8ByteLength(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function sortSerializableValue(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(sortSerializableValue);
  }

  if (value === null || typeof value !== 'object') {
    return value;
  }

  const sortedValue: Record<string, unknown> = {};
  for (const key of Object.keys(value).sort()) {
    sortedValue[key] = sortSerializableValue(
      (value as Record<string, unknown>)[key]
    );
  }
  return sortedValue;
}
