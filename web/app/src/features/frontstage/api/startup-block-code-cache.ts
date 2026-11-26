import {
  getConsoleFrontstageBlockNodeCode,
  type ConsoleFrontstageBlockNodeCode
} from '@1flowbase/api-client';

import { resolveRuntimeConcurrencyProfile } from '../../../shared/utils/runtime-concurrency-profile';
import { subscribeFrontstageSourceChanges } from '../lib/runtime-cache/source-changes';
import { getFrontstageApiBaseUrl } from './page-tree';

const STARTUP_BLOCK_CODE_TTL_MS = 30_000;

interface StartupBlockCodeFlight {
  createdAt: number;
  promise: Promise<ConsoleFrontstageBlockNodeCode>;
}

export interface StartupPrefetchedBlockCodeEntry {
  blockId: string;
  promise: Promise<ConsoleFrontstageBlockNodeCode>;
}

const startupBlockCodeFlights = new Map<string, StartupBlockCodeFlight>();

function flightKey(
  actorId: string,
  workspaceId: string,
  pageId: string,
  blockId: string
): string {
  return `${actorId}:${workspaceId}:${pageId}:${blockId}`;
}

function pruneExpiredStartupBlockCodeFlights(now: number): void {
  for (const [existingKey, existing] of startupBlockCodeFlights) {
    if (now - existing.createdAt > STARTUP_BLOCK_CODE_TTL_MS) {
      startupBlockCodeFlights.delete(existingKey);
    }
  }
}

export function prefetchStartupBlockCodeEntries({
  actorId,
  workspaceId,
  pageId,
  blockIds
}: {
  actorId: string;
  workspaceId: string;
  pageId: string;
  blockIds: readonly string[];
}): ReadonlyArray<StartupPrefetchedBlockCodeEntry> {
  if (blockIds.length === 0) return [];
  const now = Date.now();
  pruneExpiredStartupBlockCodeFlights(now);
  const results: StartupPrefetchedBlockCodeEntry[] = [];
  const seen = new Set<string>();

  const maxPrefetchBlocks =
    resolveRuntimeConcurrencyProfile().maxStartupPrefetchBlocks;

  for (const blockId of blockIds) {
    if (!blockId || seen.has(blockId)) continue;
    seen.add(blockId);
    if (results.length >= maxPrefetchBlocks) break;

    const key = flightKey(actorId, workspaceId, pageId, blockId);
    let entry = startupBlockCodeFlights.get(key);
    if (!entry) {
      const promise = getConsoleFrontstageBlockNodeCode(
        pageId,
        blockId,
        getFrontstageApiBaseUrl()
      );
      const createdEntry: StartupBlockCodeFlight = { createdAt: now, promise };
      startupBlockCodeFlights.set(key, createdEntry);
      void promise.catch(() => {
        if (startupBlockCodeFlights.get(key) === createdEntry) {
          startupBlockCodeFlights.delete(key);
        }
      });
      entry = createdEntry;
    }
    results.push({ blockId, promise: entry.promise });
  }

  return results;
}

export function seedStartupPrefetchedBlockNodeCodes({
  actorId,
  workspaceId,
  pageId,
  codes
}: {
  actorId: string;
  workspaceId: string;
  pageId: string;
  codes: readonly ConsoleFrontstageBlockNodeCode[];
}): ReadonlyArray<StartupPrefetchedBlockCodeEntry> {
  if (codes.length === 0) return [];
  const now = Date.now();
  pruneExpiredStartupBlockCodeFlights(now);
  const results: StartupPrefetchedBlockCodeEntry[] = [];
  const seen = new Set<string>();

  for (const code of codes) {
    if (!code?.block_id || seen.has(code.block_id)) continue;
    seen.add(code.block_id);
    const key = flightKey(actorId, workspaceId, pageId, code.block_id);
    const promise = Promise.resolve(code);
    startupBlockCodeFlights.set(key, { createdAt: now, promise });
    results.push({ blockId: code.block_id, promise });
  }

  return results;
}

export function readStartupPrefetchedBlockNodeCode({
  actorId,
  workspaceId,
  pageId,
  blockId
}: {
  actorId: string;
  workspaceId: string;
  pageId: string;
  blockId: string;
}): Promise<ConsoleFrontstageBlockNodeCode> | null {
  const key = flightKey(actorId, workspaceId, pageId, blockId);
  const entry = startupBlockCodeFlights.get(key);
  if (!entry) return null;
  if (Date.now() - entry.createdAt > STARTUP_BLOCK_CODE_TTL_MS) {
    startupBlockCodeFlights.delete(key);
    return null;
  }
  return entry.promise;
}

export function consumeStartupPrefetchedBlockNodeCode({
  actorId,
  workspaceId,
  pageId,
  blockId,
  promise
}: {
  actorId: string;
  workspaceId: string;
  pageId: string;
  blockId: string;
  promise?: Promise<ConsoleFrontstageBlockNodeCode>;
}): void {
  const key = flightKey(actorId, workspaceId, pageId, blockId);
  const entry = startupBlockCodeFlights.get(key);
  if (!entry) return;
  if (!promise || entry.promise === promise) {
    startupBlockCodeFlights.delete(key);
  }
}

export function invalidateStartupPrefetchedBlockNodeCode({
  actorId,
  workspaceId,
  pageId,
  blockId
}: {
  actorId?: string | null;
  workspaceId: string;
  pageId: string;
  blockId: string;
}): void {
  if (actorId) {
    startupBlockCodeFlights.delete(
      flightKey(actorId, workspaceId, pageId, blockId)
    );
    return;
  }
  const suffix = `:${workspaceId}:${pageId}:${blockId}`;
  for (const existingKey of startupBlockCodeFlights.keys()) {
    if (existingKey.endsWith(suffix)) {
      startupBlockCodeFlights.delete(existingKey);
    }
  }
}

export function clearStartupPrefetchedBlockNodeCodes(): void {
  startupBlockCodeFlights.clear();
}

subscribeFrontstageSourceChanges((change) => {
  invalidateStartupPrefetchedBlockNodeCode({
    workspaceId: change.workspaceId,
    pageId: change.pageId,
    blockId: change.blockId
  });
});
