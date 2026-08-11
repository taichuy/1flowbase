import type {
  ConsoleFrontstageBlockNode,
  ConsoleFrontstageBlockNodeSummary
} from '@1flowbase/api-client';
import { useEffect, useMemo, useState } from 'react';

import { fetchFrontstageBlockNode } from '../api/block-tree';
import {
  createFrontstageRuntimeBlockProjection,
  type FrontstageBlockInstance,
  type FrontstagePageDocumentDiagnostic
} from '../lib/page-document';

interface CanonicalBlockRuntimeInput {
  workspaceId: string;
  current?: ConsoleFrontstageBlockNode;
  ancestors?: readonly ConsoleFrontstageBlockNodeSummary[];
}

interface LoadedRuntimeDetails {
  key: string;
  details: ConsoleFrontstageBlockNode[];
  loading: boolean;
  error: unknown | null;
}

export function useFrontstageCanonicalBlockRuntime({
  workspaceId,
  current,
  ancestors = []
}: CanonicalBlockRuntimeInput) {
  const key = current
    ? [
        workspaceId,
        current.page_id,
        ...ancestors.map((node) => node.block_id),
        current.block_id
      ].join(':')
    : '';
  const ancestorBlockIdsKey = ancestors
    .map((node) => node.block_id)
    .join('\u0000');
  const [loaded, setLoaded] = useState<LoadedRuntimeDetails>({
    key: '',
    details: [],
    loading: false,
    error: null
  });

  useEffect(() => {
    if (!current) {
      setLoaded((previous) =>
        previous.key === '' && previous.details.length === 0
          ? previous
          : { key: '', details: [], loading: false, error: null }
      );
      return;
    }
    if (ancestorBlockIdsKey.length === 0) {
      setLoaded({ key, details: [current], loading: false, error: null });
      return;
    }

    let active = true;
    setLoaded({ key, details: [current], loading: true, error: null });
    void Promise.all(
      ancestorBlockIdsKey
        .split('\u0000')
        .map((blockId) =>
          fetchFrontstageBlockNode(workspaceId, current.page_id, blockId)
        )
    ).then(
      (ancestorDetails) => {
        if (active) {
          setLoaded({
            key,
            details: [...ancestorDetails, current],
            loading: false,
            error: null
          });
        }
      },
      (error: unknown) => {
        if (active) {
          setLoaded({ key, details: [current], loading: false, error });
        }
      }
    );
    return () => {
      active = false;
    };
  }, [ancestorBlockIdsKey, current, key, workspaceId]);

  return useMemo(() => {
    const details =
      loaded.key === key ? loaded.details : current ? [current] : [];
    const diagnostics: FrontstagePageDocumentDiagnostic[] = [];
    let projectionError: Error | null = null;
    const blocks = details.flatMap((detail, order) => {
      const projection = createFrontstageRuntimeBlockProjection({
        blockId: detail.block_id,
        descriptor: detail.runtime_descriptor,
        order
      });
      diagnostics.push(...projection.diagnostics);
      if (!projection.block) {
        projectionError = new Error(
          `Canonical Block ${detail.block_id} has an invalid runtime descriptor.`
        );
      }
      return projection.block ? [projection.block] : [];
    });
    return {
      blocks: blocks as FrontstageBlockInstance[],
      diagnostics,
      loading:
        loaded.key !== key ? ancestorBlockIdsKey.length > 0 : loaded.loading,
      error: loaded.key === key ? (loaded.error ?? projectionError) : null
    };
  }, [ancestorBlockIdsKey, current, key, loaded]);
}
