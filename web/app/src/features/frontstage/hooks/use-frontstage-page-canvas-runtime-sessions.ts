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

export interface FrontstagePageCanvasRuntimeSessionSnapshotEntry
  extends FrontstagePageCanvasRuntimeSessionEntryBase {
  status: RestrictedBlockRuntimeHostSnapshotStatus;
  snapshot: RestrictedBlockRuntimeHostSnapshot;
}

export interface FrontstagePageCanvasRuntimeSessionSkippedEntry
  extends FrontstagePageCanvasRuntimeSessionEntryBase {
  status: 'skipped';
  skipReason: FrontstagePageCanvasRuntimeSessionSkippedReason;
  message: string;
  path: string;
}

export interface FrontstagePageCanvasRuntimeSessionFactoryFailedEntry
  extends FrontstagePageCanvasRuntimeSessionEntryBase {
  status: 'factory_failed';
  message: string;
  error: Error;
}

export type FrontstagePageCanvasRuntimeSessionEntry =
  | FrontstagePageCanvasRuntimeSessionSnapshotEntry
  | FrontstagePageCanvasRuntimeSessionSkippedEntry
  | FrontstagePageCanvasRuntimeSessionFactoryFailedEntry;

export interface UseFrontstagePageCanvasRuntimeSessionsInput {
  runtimeRunPlanState:
    | FrontstagePageCanvasRuntimeRunPlanState
    | null
    | undefined;
  runtimeSessionFactory?: FrontstagePageCanvasRuntimeSessionFactory;
  handlers?: JsBlockHostEffectHandlers;
  demandsByBlockId?: FrontstageRuntimeDemandByBlockId;
  maxConcurrent?: number;
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
}

const SESSION_CACHE_TTL_MS = 30_000;
const SESSION_CACHE_MAX_ENTRIES = 32;
const successfulSnapshotCache = new Map<
  string,
  { snapshot: RestrictedBlockRuntimeHostSnapshot; expiresAt: number }
>();

type InternalRuntimeSessionEntry =
  FrontstagePageCanvasRuntimeSessionEntry & {
    sessionKey?: string;
  };

export function useFrontstagePageCanvasRuntimeSessions({
  runtimeRunPlanState,
  runtimeSessionFactory = createFrontstageRestrictedBlockRuntimeSession,
  handlers,
  demandsByBlockId,
  maxConcurrent = 2
}: UseFrontstagePageCanvasRuntimeSessionsInput): UseFrontstagePageCanvasRuntimeSessionsResult {
  const activeRuntimeSessionsRef = useRef(
    new Map<string, ActiveRuntimeSession>()
  );
  const [internalEntries, setInternalEntries] = useState<
    InternalRuntimeSessionEntry[]
  >([]);
  const [runtimeRevision, setRuntimeRevision] = useState(0);
  const [pageVisible, setPageVisible] = useState(
    () => typeof document === 'undefined' || document.visibilityState !== 'hidden'
  );

  useEffect(() => {
    if (typeof document === 'undefined') {
      return;
    }
    const handleVisibilityChange = () => {
      setPageVisible(document.visibilityState !== 'hidden');
    };
    document.addEventListener('visibilitychange', handleVisibilityChange);
    return () => document.removeEventListener('visibilitychange', handleVisibilityChange);
  }, []);

  useEffect(() => {
    const activeRuntimeSessions = activeRuntimeSessionsRef.current;

    if (!runtimeRunPlanState) {
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
        const sessionKey = createRuntimeSessionKey(runtimeRunPlanState, item);
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

    const createdEntries = new Map<string, InternalRuntimeSessionEntry>();
    if (pageVisible) {
      const runningCount = [...activeRuntimeSessions.values()].filter(
        (session) => session.executing
      ).length;
      const candidates = readyItems
        .filter(({ sessionKey }) => !activeRuntimeSessions.has(sessionKey))
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
          return priorityDifference || left.item.slotIndex - right.item.slotIndex;
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
          cachedSnapshot: readSuccessfulSnapshot(sessionKey)
        });
        createdEntries.set(sessionKey, createdEntry);
      }
    }

    const nextEntries: InternalRuntimeSessionEntry[] = runtimeRunPlanState.items.map(
      (item) => {
        if (item.status !== 'run_plan_ready') {
          return createSkippedEntry(item);
        }
        const sessionKey = createRuntimeSessionKey(runtimeRunPlanState, item);
        const createdEntry = createdEntries.get(sessionKey);
        if (createdEntry?.status === 'factory_failed') {
          return createdEntry;
        }
        const active = activeRuntimeSessions.get(sessionKey);
        const snapshot = active?.snapshot ?? readSuccessfulSnapshot(sessionKey);
        return snapshot
          ? { ...createSnapshotEntry(item, snapshot), sessionKey }
          : createQueuedEntry(item, sessionKey);
      }
    );

    setInternalEntries((currentEntries) =>
      areInternalEntriesEqual(currentEntries, nextEntries)
        ? currentEntries
        : nextEntries
    );
  }, [
    demandsByBlockId,
    handlers,
    maxConcurrent,
    pageVisible,
    runtimeRevision,
    runtimeRunPlanState,
    runtimeSessionFactory
  ]);

  useEffect(
    () => () => {
      disposeAllRuntimeSessions(activeRuntimeSessionsRef.current);
    },
    []
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
    for (const [sessionKey, activeRuntimeSession] of [...activeRuntimeSessions]) {
      if (activeRuntimeSession.snapshot.blockId !== blockId) {
        continue;
      }
      successfulSnapshotCache.delete(sessionKey);
      disposeRuntimeSession(activeRuntimeSessions, sessionKey, activeRuntimeSession);
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
  cachedSnapshot
}: {
  item: FrontstagePageCanvasRuntimeRunPlanReadyItem;
  sessionKey: string;
  runtimeSessionFactory: FrontstagePageCanvasRuntimeSessionFactory;
  handlers: JsBlockHostEffectHandlers | undefined;
  activeRuntimeSessions: Map<string, ActiveRuntimeSession>;
  setInternalEntries: Dispatch<SetStateAction<InternalRuntimeSessionEntry[]>>;
  setRuntimeRevision: Dispatch<SetStateAction<number>>;
  cachedSnapshot: RestrictedBlockRuntimeHostSnapshot | undefined;
}): InternalRuntimeSessionEntry {
  let session: FrontstageRestrictedBlockRuntimeSession | null = null;
  let unsubscribe: (() => void) | null = null;

  try {
    const runtimeOptions: FrontstageRestrictedBlockRuntimeHostOptions = {
      runPlan: item.runPlan
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
      if (snapshot.status === 'ready') {
        cacheSuccessfulSnapshot(sessionKey, snapshot);
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
    activeRuntimeSessions.set(sessionKey, {
      session,
      unsubscribe,
      snapshot: cachedSnapshot ?? snapshot,
      executing: true
    });

    return {
      ...createSnapshotEntry(item, cachedSnapshot ?? snapshot),
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
      currentEntry.snapshot === nextEntry.snapshot
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
  const snapshotsBySlot: Record<number, RestrictedBlockRuntimeHostSnapshot> = {};

  for (const entry of entries) {
    if ('snapshot' in entry) {
      snapshotsBySlot[entry.slotIndex] = entry.snapshot;
    }
  }

  return snapshotsBySlot;
}

function isErrorEntry(
  entry: FrontstagePageCanvasRuntimeSessionEntry
): boolean {
  if (entry.status === 'factory_failed') {
    return true;
  }

  if (entry.status === 'skipped') {
    return entry.skipReason !== 'source_not_ready';
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
  for (const [sessionKey, activeRuntimeSession] of [
    ...activeRuntimeSessions
  ]) {
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

function createRuntimeSessionKey(
  state: FrontstagePageCanvasRuntimeRunPlanState,
  item: FrontstagePageCanvasRuntimeRunPlanReadyItem
): string {
  return stableSerialize([
    state.workspaceId,
    state.pageId,
    item.sourceIndex,
    item.slotIndex,
    item.blockId,
    item.codeRef,
    item.runPlan.request,
    item.runPlan.schemaValidationOptions,
    item.runPlan.mediatorPolicy
  ]);
}

function stableSerialize(value: unknown): string {
  return JSON.stringify(sortSerializableValue(value));
}

function readSuccessfulSnapshot(
  sessionKey: string
): RestrictedBlockRuntimeHostSnapshot | undefined {
  const cached = successfulSnapshotCache.get(sessionKey);
  if (!cached) {
    return undefined;
  }
  if (cached.expiresAt <= Date.now()) {
    successfulSnapshotCache.delete(sessionKey);
    return undefined;
  }
  successfulSnapshotCache.delete(sessionKey);
  successfulSnapshotCache.set(sessionKey, cached);
  return cached.snapshot;
}

function cacheSuccessfulSnapshot(
  sessionKey: string,
  snapshot: RestrictedBlockRuntimeHostSnapshot
): void {
  successfulSnapshotCache.delete(sessionKey);
  successfulSnapshotCache.set(sessionKey, {
    snapshot,
    expiresAt: Date.now() + SESSION_CACHE_TTL_MS
  });
  while (successfulSnapshotCache.size > SESSION_CACHE_MAX_ENTRIES) {
    const oldestKey = successfulSnapshotCache.keys().next().value as
      | string
      | undefined;
    if (!oldestKey) {
      break;
    }
    successfulSnapshotCache.delete(oldestKey);
  }
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
