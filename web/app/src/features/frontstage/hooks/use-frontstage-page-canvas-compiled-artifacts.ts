import { useEffect, useMemo, useRef, useState } from 'react';

import type {
  FrontstageArtifactCacheIdentity,
  FrontstageArtifactCacheReadResult,
  FrontstageCompiledArtifactCache
} from '../lib/runtime-cache';
import { frontstageCompiledArtifactCache } from '../lib/runtime-cache';
import {
  resolveFrontstageRuntimeDemand,
  type FrontstageRuntimeDemandByBlockId
} from '../lib/page-canvas/runtime-demand';
import type {
  FrontstagePageCanvasReadyRuntimeSource,
  FrontstagePageCanvasRuntimeSourceState
} from '../lib/page-canvas/runtime-source';
import { getFrontstageRestrictedBlockRuntimeFingerprint } from '../lib/restricted-block-worker-factory';
import { createFrontstageArtifactCacheKey } from '../lib/runtime-cache';

export interface UseFrontstagePageCanvasCompiledArtifactsInput {
  actorId: string | null | undefined;
  workspaceId: string | null | undefined;
  sourceState: FrontstagePageCanvasRuntimeSourceState | null | undefined;
  demandsByBlockId?: FrontstageRuntimeDemandByBlockId;
  artifactCache?: Pick<FrontstageCompiledArtifactCache, 'get'>;
  runtimeFingerprint?: string;
}

export interface UseFrontstagePageCanvasCompiledArtifactsResult {
  sourceState: FrontstagePageCanvasRuntimeSourceState | null;
  loading: boolean;
}

interface ArtifactLookupCandidate {
  key: string;
  identity: FrontstageArtifactCacheIdentity;
}

export function useFrontstagePageCanvasCompiledArtifacts({
  actorId,
  workspaceId,
  sourceState,
  demandsByBlockId,
  artifactCache = frontstageCompiledArtifactCache,
  runtimeFingerprint = getFrontstageRestrictedBlockRuntimeFingerprint()
}: UseFrontstagePageCanvasCompiledArtifactsInput): UseFrontstagePageCanvasCompiledArtifactsResult {
  const [lookups, setLookups] = useState<
    Readonly<Record<string, FrontstageArtifactCacheReadResult>>
  >({});
  const inFlightRef = useRef(new Set<string>());
  const completedRef = useRef(new Set<string>());
  const candidates = useMemo(
    () =>
      !actorId || !workspaceId || !sourceState
        ? []
        : sourceState.sources.flatMap((source) => {
            if (
              source.status !== 'ready' ||
              resolveFrontstageRuntimeDemand(
                demandsByBlockId,
                source.blockId,
                source.slotIndex
              ) > 2
            ) {
              return [];
            }
            const identity = {
              actorId,
              workspaceId,
              runtimeFingerprint,
              sourceSha256: source.source_sha256
            };
            return [
              {
                key: createFrontstageArtifactCacheKey(identity),
                identity
              }
            ];
          }),
    [actorId, demandsByBlockId, runtimeFingerprint, sourceState, workspaceId]
  );

  useEffect(() => {
    for (const candidate of candidates) {
      if (
        completedRef.current.has(candidate.key) ||
        inFlightRef.current.has(candidate.key)
      ) {
        continue;
      }
      inFlightRef.current.add(candidate.key);
      void artifactCache
        .get(candidate.identity)
        .then((result) => {
          completedRef.current.add(candidate.key);
          setLookups((current) => ({ ...current, [candidate.key]: result }));
        })
        .catch(() => {
          completedRef.current.add(candidate.key);
          setLookups((current) => ({
            ...current,
            [candidate.key]: { status: 'unavailable', reason: 'read_failed' }
          }));
        })
        .finally(() => inFlightRef.current.delete(candidate.key));
    }
  }, [artifactCache, candidates]);

  const candidateKeys = useMemo(
    () => new Set(candidates.map((candidate) => candidate.key)),
    [candidates]
  );
  const nextSourceState = useMemo(() => {
    if (!sourceState) return null;
    return {
      ...sourceState,
      sources: sourceState.sources.map((source) =>
        enrichReadySource(
          source,
          actorId,
          workspaceId,
          runtimeFingerprint,
          candidateKeys,
          lookups
        )
      )
    };
  }, [
    actorId,
    candidateKeys,
    lookups,
    runtimeFingerprint,
    sourceState,
    workspaceId
  ]);

  return {
    sourceState: nextSourceState,
    loading: candidates.some((candidate) => !lookups[candidate.key])
  };
}

function enrichReadySource(
  source: FrontstagePageCanvasRuntimeSourceState['sources'][number],
  actorId: string | null | undefined,
  workspaceId: string | null | undefined,
  runtimeFingerprint: string,
  candidateKeys: ReadonlySet<string>,
  lookups: Readonly<Record<string, FrontstageArtifactCacheReadResult>>
): FrontstagePageCanvasRuntimeSourceState['sources'][number] {
  if (source.status !== 'ready' || !actorId || !workspaceId) return source;
  const key = createFrontstageArtifactCacheKey({
    actorId,
    workspaceId,
    runtimeFingerprint,
    sourceSha256: source.source_sha256
  });
  if (!candidateKeys.has(key)) return source;
  const lookup = lookups[key];
  if (!lookup) return { ...source, artifactLookupStatus: 'pending' };
  if (lookup.status === 'hit') {
    return {
      ...source,
      artifactLookupStatus: 'hit',
      compiledArtifact: lookup.artifact
    };
  }
  return {
    ...source,
    artifactLookupStatus: lookup.status,
    compiledArtifact: undefined
  } as FrontstagePageCanvasReadyRuntimeSource;
}
