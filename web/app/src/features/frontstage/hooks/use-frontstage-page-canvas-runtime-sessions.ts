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
}

export interface UseFrontstagePageCanvasRuntimeSessionsResult {
  entries: FrontstagePageCanvasRuntimeSessionEntry[];
  snapshotsBySlot: Readonly<Record<number, RestrictedBlockRuntimeHostSnapshot>>;
  running: boolean;
  hasError: boolean;
}

interface ActiveRuntimeSession {
  session: FrontstageRestrictedBlockRuntimeSession;
  unsubscribe: () => void;
  snapshot: RestrictedBlockRuntimeHostSnapshot;
}

type InternalRuntimeSessionEntry =
  FrontstagePageCanvasRuntimeSessionEntry & {
    sessionKey?: string;
  };

export function useFrontstagePageCanvasRuntimeSessions({
  runtimeRunPlanState,
  runtimeSessionFactory = createFrontstageRestrictedBlockRuntimeSession,
  handlers
}: UseFrontstagePageCanvasRuntimeSessionsInput): UseFrontstagePageCanvasRuntimeSessionsResult {
  const activeRuntimeSessionsRef = useRef(
    new Map<string, ActiveRuntimeSession>()
  );
  const [internalEntries, setInternalEntries] = useState<
    InternalRuntimeSessionEntry[]
  >([]);

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
    const nextEntries: InternalRuntimeSessionEntry[] = [];

    for (const item of runtimeRunPlanState.items) {
      if (item.status !== 'run_plan_ready') {
        nextEntries.push(createSkippedEntry(item));
        continue;
      }

      const sessionKey = createRuntimeSessionKey(runtimeRunPlanState, item);
      nextSessionKeys.add(sessionKey);

      const activeRuntimeSession = activeRuntimeSessions.get(sessionKey);
      if (activeRuntimeSession) {
        nextEntries.push({
          ...createSnapshotEntry(item, activeRuntimeSession.snapshot),
          sessionKey
        });
        continue;
      }

      const createdEntry = createAndRunRuntimeSession({
        item,
        sessionKey,
        runtimeSessionFactory,
        handlers,
        activeRuntimeSessions,
        setInternalEntries
      });
      nextEntries.push(createdEntry);
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

    setInternalEntries((currentEntries) =>
      areInternalEntriesEqual(currentEntries, nextEntries)
        ? currentEntries
        : nextEntries
    );
  }, [handlers, runtimeRunPlanState, runtimeSessionFactory]);

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

  return {
    entries,
    snapshotsBySlot,
    running,
    hasError
  };
}

function createAndRunRuntimeSession({
  item,
  sessionKey,
  runtimeSessionFactory,
  handlers,
  activeRuntimeSessions,
  setInternalEntries
}: {
  item: FrontstagePageCanvasRuntimeRunPlanReadyItem;
  sessionKey: string;
  runtimeSessionFactory: FrontstagePageCanvasRuntimeSessionFactory;
  handlers: JsBlockHostEffectHandlers | undefined;
  activeRuntimeSessions: Map<string, ActiveRuntimeSession>;
  setInternalEntries: Dispatch<SetStateAction<InternalRuntimeSessionEntry[]>>;
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
      setInternalEntries((currentEntries) =>
        updateInternalEntries(currentEntries, sessionKey, {
          ...createSnapshotEntry(item, snapshot),
          sessionKey
        })
      );
    });

    const snapshot = session.run();
    activeRuntimeSessions.set(sessionKey, {
      session,
      unsubscribe,
      snapshot
    });

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
