import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';

import { useAuthStore } from '../../../state/auth-store';
import {
  createFrontstagePageGroupNode,
  createFrontstagePageNode,
  deleteFrontstageNode,
  frontstagePageTreeQueryKey,
  moveFrontstageNode,
  renameFrontstagePageNode,
  updateFrontstagePageNodeMetadata,
  type CreateFrontstageNodeInput,
  type MoveFrontstageNodeInput,
  type RenameFrontstageNodeInput,
  type UpdateFrontstageNodeMetadataInput
} from '../api/page-tree';

function requireCsrfToken(csrfToken: string | null): string {
  if (!csrfToken) {
    throw new Error('missing csrf token');
  }

  return csrfToken;
}

function toError(error: unknown): Error {
  return error instanceof Error
    ? error
    : new Error('frontstage page tree mutation failed');
}

export function useFrontstagePageTreeMutations(workspaceId: string) {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const queryKey = frontstagePageTreeQueryKey(workspaceId);
  const [mutationError, setMutationError] = useState<Error | null>(null);

  const invalidatePageTree = async () => {
    await queryClient.invalidateQueries({ queryKey, refetchType: 'active' });
  };

  const invalidatePageTreeAndContent = async () => {
    await invalidatePageTree();
    await queryClient.invalidateQueries({
      queryKey: ['frontstage', workspaceId, 'pages'],
      refetchType: 'active'
    });
  };

  const clearMutationError = () => {
    setMutationError(null);
  };

  const captureMutationError = (error: unknown) => {
    setMutationError(toError(error));
  };

  const createGroupMutation = useMutation({
    mutationFn: (input: CreateFrontstageNodeInput) =>
      createFrontstagePageGroupNode(
        workspaceId,
        input,
        requireCsrfToken(csrfToken)
      ),
    onMutate: clearMutationError,
    onError: captureMutationError,
    onSuccess: invalidatePageTree
  });

  const createPageMutation = useMutation({
    mutationFn: (input: CreateFrontstageNodeInput) =>
      createFrontstagePageNode(workspaceId, input, requireCsrfToken(csrfToken)),
    onMutate: clearMutationError,
    onError: captureMutationError,
    onSuccess: invalidatePageTreeAndContent
  });

  const renameMutation = useMutation({
    mutationFn: ({
      pageNodeId,
      input
    }: {
      pageNodeId: string;
      input: RenameFrontstageNodeInput;
    }) =>
      renameFrontstagePageNode(
        workspaceId,
        pageNodeId,
        input,
        requireCsrfToken(csrfToken)
      ),
    onMutate: clearMutationError,
    onError: captureMutationError,
    onSuccess: invalidatePageTreeAndContent
  });

  const moveMutation = useMutation({
    mutationFn: ({
      pageNodeId,
      input
    }: {
      pageNodeId: string;
      input: MoveFrontstageNodeInput;
    }) =>
      moveFrontstageNode(
        workspaceId,
        pageNodeId,
        input,
        requireCsrfToken(csrfToken)
      ),
    onMutate: clearMutationError,
    onError: captureMutationError,
    onSuccess: invalidatePageTree
  });

  const metadataMutation = useMutation({
    mutationFn: ({
      pageNodeId,
      input
    }: {
      pageNodeId: string;
      input: UpdateFrontstageNodeMetadataInput;
    }) =>
      updateFrontstagePageNodeMetadata(
        workspaceId,
        pageNodeId,
        input,
        requireCsrfToken(csrfToken)
      ),
    onMutate: clearMutationError,
    onError: captureMutationError,
    onSuccess: invalidatePageTree
  });

  const deleteMutation = useMutation({
    mutationFn: (pageNodeId: string) =>
      deleteFrontstageNode(
        workspaceId,
        pageNodeId,
        requireCsrfToken(csrfToken)
      ),
    onMutate: clearMutationError,
    onError: captureMutationError,
    onSuccess: invalidatePageTree
  });

  return {
    isPending:
      createGroupMutation.isPending ||
      createPageMutation.isPending ||
      renameMutation.isPending ||
      moveMutation.isPending ||
      metadataMutation.isPending ||
      deleteMutation.isPending,
    error: mutationError,
    createGroup: createGroupMutation.mutateAsync,
    createPage: createPageMutation.mutateAsync,
    renameNode: (pageNodeId: string, input: RenameFrontstageNodeInput) =>
      renameMutation.mutateAsync({ pageNodeId, input }),
    moveNode: (pageNodeId: string, input: MoveFrontstageNodeInput) =>
      moveMutation.mutateAsync({ pageNodeId, input }),
    updateNodeMetadata: (
      pageNodeId: string,
      input: UpdateFrontstageNodeMetadataInput
    ) => metadataMutation.mutateAsync({ pageNodeId, input }),
    deleteNode: deleteMutation.mutateAsync
  };
}
