import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useMemo, useState } from 'react';

import { useAuthStore } from '../../../state/auth-store';
import {
  frontstagePageContentQueryKey,
  saveFrontstagePageContent,
  type FrontstagePageContent,
  type SaveFrontstageTabDocumentInput
} from '../api/page-content';

interface UseFrontstagePageContentSaveInput {
  workspaceId: string | null | undefined;
  pageId: string | null | undefined;
  tabId: string | null | undefined;
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
    : new Error('frontstage page content save failed');
}

export function useFrontstagePageContentSave({
  workspaceId,
  pageId,
  tabId
}: UseFrontstagePageContentSaveInput) {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [mutationError, setMutationError] = useState<Error | null>(null);

  const queryKey = useMemo(
    () =>
      frontstagePageContentQueryKey(
        workspaceId ?? '',
        pageId ?? '',
        tabId ?? ''
      ),
    [pageId, tabId, workspaceId]
  );

  const clearMutationError = () => {
    setMutationError(null);
  };

  const captureMutationError = (error: unknown) => {
    setMutationError(toError(error));
  };

  const saveMutation = useMutation({
    mutationFn: (input: SaveFrontstageTabDocumentInput) =>
      saveFrontstagePageContent(
        requireValue(workspaceId, 'workspace id'),
        requireValue(pageId, 'page id'),
        requireValue(tabId, 'tab id'),
        input,
        requireCsrfToken(csrfToken)
      ),
    onMutate: clearMutationError,
    onError: captureMutationError,
    onSuccess: async (savedContent: FrontstagePageContent) => {
      queryClient.setQueryData(queryKey, savedContent);
      await queryClient.invalidateQueries({
        queryKey: ['frontstage', workspaceId ?? '', 'pages', pageId ?? '', 'tabs'],
        refetchType: 'active'
      });
    }
  });

  const reset = () => {
    saveMutation.reset();
    setMutationError(null);
  };

  return {
    save: saveMutation.mutateAsync,
    saving: saveMutation.isPending,
    isPending: saveMutation.isPending,
    error: mutationError,
    reset,
    clearError: clearMutationError
  };
}
