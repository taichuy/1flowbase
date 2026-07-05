import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';

import { useAuthStore } from '../../../state/auth-store';
import {
  fetchFrontstageBlockCode,
  frontstageBlockCodeQueryKey,
  saveFrontstageBlockCode,
  type FrontstageBlockCode
} from '../api/block-code';
import { isForbiddenResponseError } from '../lib/api-errors';

interface UseFrontstageBlockCodeInput {
  workspaceId: string | null | undefined;
  pageId: string | null | undefined;
  codeRef: string | null | undefined;
}

function requireValue(value: string | null | undefined, label: string): string {
  if (!value) {
    throw new Error(`missing ${label}`);
  }

  return value;
}

function requireCsrfToken(csrfToken: string | null): string {
  if (!csrfToken) {
    throw new Error('missing csrf token');
  }

  return csrfToken;
}

function toError(error: unknown): Error {
  return error instanceof Error
    ? error
    : new Error('frontstage block code operation failed');
}

export function useFrontstageBlockCode({
  workspaceId,
  pageId,
  codeRef
}: UseFrontstageBlockCodeInput) {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [draft, setDraft] = useState('');
  const [mutationError, setMutationError] = useState<Error | null>(null);
  const canRead = Boolean(workspaceId && pageId && codeRef);

  const queryKey = useMemo(
    () =>
      frontstageBlockCodeQueryKey(
        workspaceId ?? '',
        pageId ?? '',
        codeRef ?? ''
      ),
    [codeRef, pageId, workspaceId]
  );

  const blockCodeQuery = useQuery({
    queryKey,
    queryFn: () =>
      fetchFrontstageBlockCode(
        requireValue(workspaceId, 'workspace id'),
        requireValue(pageId, 'page id'),
        requireValue(codeRef, 'code ref')
      ),
    enabled: canRead
  });

  const code = blockCodeQuery.data?.code ?? '';

  useEffect(() => {
    setDraft(code);
  }, [code, queryKey]);

  const clearMutationError = () => {
    setMutationError(null);
  };

  const captureMutationError = (error: unknown) => {
    setMutationError(toError(error));
  };

  const saveMutation = useMutation({
    mutationFn: async () =>
      saveFrontstageBlockCode(
        requireValue(workspaceId, 'workspace id'),
        requireValue(pageId, 'page id'),
        {
          codeRef: requireValue(codeRef, 'code ref'),
          code: draft
        },
        requireCsrfToken(csrfToken)
      ),
    onMutate: clearMutationError,
    onError: captureMutationError,
    onSuccess: async (savedBlockCode: FrontstageBlockCode) => {
      queryClient.setQueryData(queryKey, savedBlockCode);
      await queryClient.invalidateQueries({ queryKey, refetchType: 'active' });
    }
  });

  const readOrMutationError =
    mutationError ??
    (blockCodeQuery.error ? toError(blockCodeQuery.error) : null);

  return {
    code,
    draft,
    dirty: draft !== code,
    loading: blockCodeQuery.isFetching,
    saving: saveMutation.isPending,
    error: readOrMutationError,
    permissionDenied: isForbiddenResponseError(readOrMutationError),
    setDraft,
    reset: () => {
      setDraft(code);
      setMutationError(null);
    },
    save: saveMutation.mutateAsync
  };
}
