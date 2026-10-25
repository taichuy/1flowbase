import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  createFrontstageGroup,
  createFrontstagePage,
  updateFrontstagePageNodeTitle,
  deleteFrontstagePageNode,
  createFrontstagePageTab,
  updateFrontstagePageTab,
  deleteFrontstagePageTab,
  type CreateFrontstagePageNodeInput,
  type UpdateFrontstagePageNodeTitleInput,
  type CreateFrontstagePageTabInput,
  type UpdateFrontstagePageTabInput
} from '@1flowbase/api-client';
import { useAuthStore } from '../../../../state/auth-store';
import { getFrontstageApiBaseUrl } from '../../../frontstage/api/page-tree';

export type RouteManagementAction =
  | {
      kind: 'create';
      nodeKind: 'page' | 'group';
      input: CreateFrontstagePageNodeInput;
    }
  | { kind: 'update'; id: string; input: UpdateFrontstagePageNodeTitleInput }
  | { kind: 'delete'; id: string }
  | { kind: 'create-tab'; pageId: string; input: CreateFrontstagePageTabInput }
  | {
      kind: 'update-tab';
      pageId: string;
      id: string;
      input: UpdateFrontstagePageTabInput;
    }
  | { kind: 'delete-tab'; pageId: string; id: string }
  | {
      kind: 'reorder-tabs';
      pageId: string;
      updates: Array<{ tabId: string; rank: string }>;
    };

export function useRouteManagementMutation(workspaceId: string) {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const client = useQueryClient();
  return useMutation({
    mutationFn: async (action: RouteManagementAction) => {
      if (!csrfToken) throw new Error('missing csrf token');
      const base = getFrontstageApiBaseUrl();
      switch (action.kind) {
        case 'create':
          return action.nodeKind === 'group'
            ? createFrontstageGroup(action.input, csrfToken, base)
            : createFrontstagePage(action.input, csrfToken, base);
        case 'update':
          return updateFrontstagePageNodeTitle(
            action.id,
            action.input,
            csrfToken,
            base
          );
        case 'delete':
          return deleteFrontstagePageNode(action.id, csrfToken, base);
        case 'create-tab':
          return createFrontstagePageTab(
            action.pageId,
            action.input,
            csrfToken,
            base
          );
        case 'update-tab':
          return updateFrontstagePageTab(
            action.pageId,
            action.id,
            action.input,
            csrfToken,
            base
          );
        case 'delete-tab':
          return deleteFrontstagePageTab(
            action.pageId,
            action.id,
            csrfToken,
            base
          );
        case 'reorder-tabs': {
          const results = await Promise.allSettled(
            action.updates.map(({ tabId, rank }) =>
              updateFrontstagePageTab(
                action.pageId,
                tabId,
                { rank },
                csrfToken,
                base
              )
            )
          );
          const failed = results.find((result) => result.status === 'rejected');
          if (failed?.status === 'rejected') throw failed.reason;
          return;
        }
      }
    },
    // Refetch even after a partial tab reorder failure; cached order is never authoritative.
    onSettled: async () => {
      await Promise.all([
        client.invalidateQueries({
          queryKey: ['frontstage', workspaceId, 'page-tree']
        }),
        client.invalidateQueries({
          queryKey: ['frontstage', workspaceId, 'pages']
        })
      ]);
    }
  });
}
