import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { useFrontstagePageTreeMutations } from '../hooks/use-frontstage-page-tree-mutations';

const frontstageApi = vi.hoisted(() => ({
  createFrontstagePageGroupNode: vi.fn(),
  createFrontstagePageNode: vi.fn(),
  deleteFrontstageNode: vi.fn(),
  frontstagePageTreeQueryKey: vi.fn((workspaceId: string) => [
    'frontstage',
    workspaceId,
    'page-tree'
  ]),
  moveFrontstageNode: vi.fn(),
  renameFrontstagePageNode: vi.fn(),
  updateFrontstagePageNodeMetadata: vi.fn()
}));

vi.mock('../api/page-tree', () => frontstageApi);

function authenticate(csrfToken: string | null = 'csrf-123') {
  if (!csrfToken) {
    resetAuthStore();
    return;
  }

  useAuthStore.getState().setAuthenticated({
    csrfToken,
    actor: {
      id: 'actor-1',
      account: 'normal-user',
      effective_display_role: 'developer',
      current_workspace_id: 'workspace-1'
    },
    me: null
  });
}

function createQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false }
    }
  });
}

function setupMutations(queryClient = createQueryClient()) {
  const invalidateQueriesSpy = vi
    .spyOn(queryClient, 'invalidateQueries')
    .mockResolvedValue();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  const view = renderHook(() => useFrontstagePageTreeMutations('workspace-1'), {
    wrapper
  });

  return {
    invalidateQueriesSpy,
    result: view.result
  };
}

describe('useFrontstagePageTreeMutations', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetAuthStore();
    authenticate();
    frontstageApi.createFrontstagePageGroupNode.mockResolvedValue({
      id: 'group-1',
      kind: 'group'
    });
    frontstageApi.createFrontstagePageNode.mockResolvedValue({
      id: 'page-1',
      kind: 'page'
    });
    frontstageApi.renameFrontstagePageNode.mockResolvedValue({
      id: 'page-1',
      kind: 'page'
    });
    frontstageApi.updateFrontstagePageNodeMetadata.mockResolvedValue({
      id: 'page-1',
      kind: 'page'
    });
    frontstageApi.moveFrontstageNode.mockResolvedValue({
      id: 'page-1',
      kind: 'page'
    });
    frontstageApi.deleteFrontstageNode.mockResolvedValue(undefined);
  });

  test('passes csrf token through mutations and refetches the workspace page tree', async () => {
    const { invalidateQueriesSpy, result } = setupMutations();

    await act(async () => {
      await result.current.createGroup({
        title: '分组 1',
        parentId: null,
        rank: '001000'
      });
      await result.current.createPage({
        title: '页面 1',
        parentId: 'group-1',
        rank: '001000'
      });
      await result.current.renameNode('page-1', { title: '页面 新名' });
      await result.current.updateNodeMetadata('page-1', {
        tooltip: '展示在页面树',
        isHidden: true
      });
      await result.current.moveNode('page-1', {
        parentId: null,
        rank: '000000'
      });
      await result.current.deleteNode('page-1');
    });

    expect(frontstageApi.createFrontstagePageGroupNode).toHaveBeenCalledWith(
      'workspace-1',
      { title: '分组 1', parentId: null, rank: '001000' },
      'csrf-123'
    );
    expect(frontstageApi.createFrontstagePageNode).toHaveBeenCalledWith(
      'workspace-1',
      { title: '页面 1', parentId: 'group-1', rank: '001000' },
      'csrf-123'
    );
    expect(frontstageApi.renameFrontstagePageNode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      { title: '页面 新名' },
      'csrf-123'
    );
    expect(frontstageApi.updateFrontstagePageNodeMetadata).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      { tooltip: '展示在页面树', isHidden: true },
      'csrf-123'
    );
    expect(frontstageApi.moveFrontstageNode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      { parentId: null, rank: '000000' },
      'csrf-123'
    );
    expect(frontstageApi.deleteFrontstageNode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'csrf-123'
    );
    expect(invalidateQueriesSpy).toHaveBeenCalledTimes(8);
    expect(invalidateQueriesSpy).toHaveBeenLastCalledWith({
      queryKey: ['frontstage', 'workspace-1', 'page-tree'],
      refetchType: 'active'
    });
  });

  test('exposes mutation failures and clears stale error on the next attempt', async () => {
    const failedRequest = new Error('rename failed');
    frontstageApi.renameFrontstagePageNode.mockRejectedValueOnce(failedRequest);
    const { result } = setupMutations();

    await act(async () => {
      await result.current
        .renameNode('page-1', { title: '页面 新名' })
        .catch(() => undefined);
    });

    await waitFor(() => {
      expect(result.current.error).toBe(failedRequest);
    });

    await act(async () => {
      await result.current.renameNode('page-1', { title: '页面 新名' });
    });

    expect(result.current.error).toBeNull();
  });

  test('rejects write mutations without csrf token before calling feature api', async () => {
    authenticate(null);
    const { result } = setupMutations();
    let mutationError: unknown;

    await act(async () => {
      await result.current
        .createPage({
          title: '页面 1',
          parentId: null,
          rank: '001000'
        })
        .catch((error: unknown) => {
          mutationError = error;
        });
    });

    expect(mutationError).toEqual(new Error('missing csrf token'));
    expect(frontstageApi.createFrontstagePageNode).not.toHaveBeenCalled();
    await waitFor(() => {
      expect(result.current.error).toEqual(new Error('missing csrf token'));
    });
  });
});
