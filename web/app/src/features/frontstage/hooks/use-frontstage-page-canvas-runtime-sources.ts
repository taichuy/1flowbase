import { useQueries } from '@tanstack/react-query';
import { useMemo } from 'react';

import {
  fetchFrontstageBlockCode,
  frontstageBlockCodeQueryKey
} from '../api/block-code';
import type { FrontstagePageRenderPlan } from '../lib/page-canvas/render-plan';
import {
  createFrontstagePageCanvasBlockCodeReadPlan,
  createFrontstagePageCanvasRuntimeSourceState,
  type FrontstagePageCanvasBlockCodeReadPlan,
  type FrontstagePageCanvasBlockCodeReadRequest,
  type FrontstagePageCanvasBlockCodeReadResult,
  type FrontstagePageCanvasRuntimeSourceState
} from '../lib/page-canvas/runtime-source';
import { recordFrontstageRuntimeObservation } from '../lib/page-canvas/runtime-observation';
import {
  resolveFrontstageRuntimeDemand,
  type FrontstageRuntimeDemandByBlockId
} from '../lib/page-canvas/runtime-demand';

export interface UseFrontstagePageCanvasRuntimeSourcesInput {
  actorId: string | null | undefined;
  actorWorkspaceId: string | null | undefined;
  workspaceId: string | null | undefined;
  tabId?: string | null;
  renderPlan: FrontstagePageRenderPlan | null | undefined;
  demandsByBlockId?: FrontstageRuntimeDemandByBlockId;
}

export interface UseFrontstagePageCanvasRuntimeSourcesResult {
  readPlan: FrontstagePageCanvasBlockCodeReadPlan | null;
  sourceState: FrontstagePageCanvasRuntimeSourceState | null;
  loading: boolean;
  hasError: boolean;
  errors: Error[];
}

const EMPTY_BLOCK_CODE_REQUESTS: FrontstagePageCanvasBlockCodeReadRequest[] = [];

function toError(error: unknown): Error {
  return error instanceof Error
    ? error
    : new Error('frontstage page canvas block code request failed');
}

function isNonEmptyCode(code: unknown): code is string {
  return typeof code === 'string' && code.trim().length > 0;
}

function createCodeResult(
  request: FrontstagePageCanvasBlockCodeReadRequest,
  query: {
    data?: { code?: unknown; source_sha256?: unknown };
    error: unknown;
    isError: boolean;
  },
  dormant: boolean
): FrontstagePageCanvasBlockCodeReadResult {
  if (dormant) {
    return { codeRef: request.codeRef, status: 'dormant' };
  }
  if (query.isError) {
    return {
      codeRef: request.codeRef,
      status: 'failed',
      error: query.error
    };
  }

  if (query.data) {
    if (
      isNonEmptyCode(query.data.code) &&
      isNonEmptyCode(query.data.source_sha256)
    ) {
      return {
        codeRef: request.codeRef,
        status: 'ready',
        code: query.data.code,
        source_sha256: query.data.source_sha256
      };
    }

    return {
      codeRef: request.codeRef,
      status: 'missing',
      message: `Block code is empty for ${request.codeRef}.`
    };
  }

  return {
    codeRef: request.codeRef,
    status: 'loading'
  };
}

export function useFrontstagePageCanvasRuntimeSources({
  actorId,
  actorWorkspaceId,
  workspaceId,
  tabId = null,
  renderPlan,
  demandsByBlockId
}: UseFrontstagePageCanvasRuntimeSourcesInput): UseFrontstagePageCanvasRuntimeSourcesResult {
  const readPlan = useMemo(() => {
    if (!workspaceId || !renderPlan) {
      return null;
    }

    return createFrontstagePageCanvasBlockCodeReadPlan({
      workspaceId,
      renderPlan
    });
  }, [renderPlan, workspaceId]);
  const requests = readPlan?.requests ?? EMPTY_BLOCK_CODE_REQUESTS;
  const canAccessWorkspace = Boolean(
    actorId && workspaceId && actorWorkspaceId === workspaceId
  );

  const blockCodeQueries = useQueries({
    queries: requests.map((request) => ({
      queryKey: frontstageBlockCodeQueryKey(
        request.workspaceId,
        request.pageId,
        request.codeRef,
        actorId ?? ''
      ),
      queryFn: async () => {
        const startedAt = Date.now();
        try {
          return await fetchFrontstageBlockCode(
            request.workspaceId,
            request.pageId,
            request.codeRef
          );
        } finally {
          recordFrontstageRuntimeObservation({
            stage: 'source_fetch',
            cacheTier: 'network',
            actorId: actorId ?? '',
            workspaceId: request.workspaceId,
            pageId: request.pageId,
            tabId,
            blockId: request.blockId,
            timestampMs: startedAt,
            durationMs: Math.max(0, Date.now() - startedAt)
          });
        }
      },
      enabled:
        canAccessWorkspace &&
        (!demandsByBlockId ||
          resolveFrontstageRuntimeDemand(
            demandsByBlockId,
            request.blockId,
            request.slotIndex
          ) <= 2),
      staleTime: Infinity,
      gcTime: Infinity,
      refetchOnMount: false,
      refetchOnWindowFocus: false,
      refetchOnReconnect: false
    }))
  });

  const codeResults = useMemo(
    () =>
      requests.map((request, index) =>
        createCodeResult(
          request,
          blockCodeQueries[index],
          Boolean(
            demandsByBlockId &&
              resolveFrontstageRuntimeDemand(
                demandsByBlockId,
                request.blockId,
                request.slotIndex
              ) === 3
          )
        )
      ),
    [blockCodeQueries, demandsByBlockId, requests]
  );

  const sourceState = useMemo(() => {
    if (!renderPlan || !readPlan) {
      return null;
    }

    return createFrontstagePageCanvasRuntimeSourceState({
      renderPlan,
      readPlan,
      codeResults
    });
  }, [codeResults, readPlan, renderPlan]);

  const errors = useMemo(
    () =>
      blockCodeQueries
        .filter((query) => query.isError)
        .map((query) => toError(query.error)),
    [blockCodeQueries]
  );

  return {
    readPlan,
    sourceState,
    loading: blockCodeQueries.some((query) => query.isFetching),
    hasError: errors.length > 0,
    errors
  };
}
