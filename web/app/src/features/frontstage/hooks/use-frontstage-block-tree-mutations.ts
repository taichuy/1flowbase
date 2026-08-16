import { useMutation, useQueryClient } from '@tanstack/react-query';

import { useAuthStore } from '../../../state/auth-store';
import {
  createFrontstageBlockNode,
  deleteFrontstageBlockLeaf,
  deleteFrontstageBlockSubtree,
  frontstageBlockTreeQueryKeys,
  moveFrontstageBlockNode,
  saveFrontstageBlockNodeCode,
  updateFrontstageBlockDescriptors,
  updateFrontstageBlockNode,
  type CreateFrontstageBlockNodeInput,
  type DeleteFrontstageBlockSubtreeInput,
  type MoveFrontstageBlockNodeInput,
  type SaveFrontstageBlockNodeCodeInput,
  type UpdateFrontstageBlockDescriptorsInput,
  type UpdateFrontstageBlockNodeInput
} from '../api/block-tree';

interface BlockOwner {
  block_id: string;
  parent_block_id: string | null;
  tab_id: string;
}

interface UpdateBlockVariables {
  block_id: string;
  input: UpdateFrontstageBlockNodeInput;
}

interface UpdateBlockDescriptorsVariables {
  tab_id: string;
  input: UpdateFrontstageBlockDescriptorsInput;
}

interface MoveBlockVariables {
  block_id: string;
  previous_parent_block_id: string | null;
  input: MoveFrontstageBlockNodeInput;
}

type DeleteBlockLeafVariables = BlockOwner;

interface DeleteBlockSubtreeVariables extends BlockOwner {
  input: DeleteFrontstageBlockSubtreeInput;
}

interface SaveBlockCodeVariables {
  block_id: string;
  input: SaveFrontstageBlockNodeCodeInput;
}

function requireCsrfToken(csrfToken: string | null): string {
  if (!csrfToken) throw new Error('missing csrf token');
  return csrfToken;
}

export function useFrontstageBlockTreeMutations(
  workspaceId: string,
  pageId: string
) {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();

  const invalidateSearches = () =>
    queryClient.invalidateQueries({
      queryKey: frontstageBlockTreeQueryKeys.searches(workspaceId, pageId),
      refetchType: 'active'
    });

  const invalidateOwner = (parentBlockId: string | null, tabId: string) =>
    queryClient.invalidateQueries({
      queryKey:
        parentBlockId === null
          ? frontstageBlockTreeQueryKeys.roots(workspaceId, pageId, {
              tab_id: tabId
            })
          : frontstageBlockTreeQueryKeys.children(
              workspaceId,
              pageId,
              parentBlockId
            ),
      refetchType: 'active'
    });

  const invalidateDetail = (blockId: string) =>
    queryClient.invalidateQueries({
      queryKey: frontstageBlockTreeQueryKeys.block(
        workspaceId,
        pageId,
        blockId
      ),
      refetchType: 'active'
    });

  const createMutation = useMutation({
    mutationFn: (input: CreateFrontstageBlockNodeInput) =>
      createFrontstageBlockNode(
        workspaceId,
        pageId,
        input,
        requireCsrfToken(csrfToken)
      ),
    onSuccess: async (node) => {
      await Promise.all([
        invalidateOwner(node.parent_block_id, node.tab_id),
        invalidateSearches()
      ]);
    }
  });

  const updateMutation = useMutation({
    mutationFn: ({ block_id, input }: UpdateBlockVariables) =>
      updateFrontstageBlockNode(
        workspaceId,
        pageId,
        block_id,
        input,
        requireCsrfToken(csrfToken)
      ),
    onSuccess: async (node) => {
      await Promise.all([
        invalidateOwner(node.parent_block_id, node.tab_id),
        invalidateDetail(node.block_id),
        invalidateSearches()
      ]);
    }
  });

  const updateDescriptorsMutation = useMutation({
    mutationFn: ({ tab_id, input }: UpdateBlockDescriptorsVariables) =>
      updateFrontstageBlockDescriptors(
        workspaceId,
        pageId,
        tab_id,
        input,
        requireCsrfToken(csrfToken)
      ),
    onSuccess: async (nodes, variables) => {
      await Promise.all([
        invalidateOwner(null, variables.tab_id),
        ...nodes.map((node) => invalidateDetail(node.block_id))
      ]);
    }
  });

  const moveMutation = useMutation({
    mutationFn: ({ block_id, input }: MoveBlockVariables) =>
      moveFrontstageBlockNode(
        workspaceId,
        pageId,
        block_id,
        input,
        requireCsrfToken(csrfToken)
      ),
    onSuccess: async (node, variables) => {
      const invalidations = [
        invalidateOwner(node.parent_block_id, node.tab_id),
        invalidateDetail(node.block_id),
        invalidateSearches()
      ];
      if (variables.previous_parent_block_id !== node.parent_block_id) {
        invalidations.push(
          invalidateOwner(variables.previous_parent_block_id, node.tab_id)
        );
      }
      await Promise.all(invalidations);
    }
  });

  const deleteLeafMutation = useMutation({
    mutationFn: ({ block_id }: DeleteBlockLeafVariables) =>
      deleteFrontstageBlockLeaf(
        workspaceId,
        pageId,
        block_id,
        requireCsrfToken(csrfToken)
      ),
    onSuccess: async (_result, variables) => {
      queryClient.removeQueries({
        queryKey: frontstageBlockTreeQueryKeys.block(
          workspaceId,
          pageId,
          variables.block_id
        )
      });
      queryClient.removeQueries({
        queryKey: frontstageBlockTreeQueryKeys.code(
          workspaceId,
          pageId,
          variables.block_id
        )
      });
      await Promise.all([
        invalidateOwner(variables.parent_block_id, variables.tab_id),
        invalidateSearches()
      ]);
    }
  });

  const deleteSubtreeMutation = useMutation({
    mutationFn: ({ block_id, input }: DeleteBlockSubtreeVariables) =>
      deleteFrontstageBlockSubtree(
        workspaceId,
        pageId,
        block_id,
        input,
        requireCsrfToken(csrfToken)
      ),
    onSuccess: async (_result, variables) => {
      queryClient.removeQueries({
        queryKey: frontstageBlockTreeQueryKeys.block(
          workspaceId,
          pageId,
          variables.block_id
        )
      });
      queryClient.removeQueries({
        queryKey: frontstageBlockTreeQueryKeys.code(
          workspaceId,
          pageId,
          variables.block_id
        )
      });
      await Promise.all([
        invalidateOwner(variables.parent_block_id, variables.tab_id),
        invalidateSearches()
      ]);
    }
  });

  const saveCodeMutation = useMutation({
    mutationFn: ({ block_id, input }: SaveBlockCodeVariables) =>
      saveFrontstageBlockNodeCode(
        workspaceId,
        pageId,
        block_id,
        input,
        requireCsrfToken(csrfToken)
      ),
    onSuccess: (code) => {
      queryClient.setQueryData(
        frontstageBlockTreeQueryKeys.code(workspaceId, pageId, code.block_id),
        code
      );
    }
  });

  return {
    create: createMutation,
    update: updateMutation,
    updateDescriptors: updateDescriptorsMutation,
    move: moveMutation,
    deleteLeaf: deleteLeafMutation,
    deleteSubtree: deleteSubtreeMutation,
    saveCode: saveCodeMutation
  };
}
