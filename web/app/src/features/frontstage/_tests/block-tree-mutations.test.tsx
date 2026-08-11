import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';
import { readFile } from 'node:fs/promises';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { frontstageBlockTreeQueryKeys } from '../api/block-tree';
import { useFrontstageBlockTreeMutations } from '../hooks/use-frontstage-block-tree-mutations';

const blockTreeApi = vi.hoisted(() => ({
  createFrontstageBlockNode: vi.fn(),
  deleteFrontstageBlockLeaf: vi.fn(),
  deleteFrontstageBlockSubtree: vi.fn(),
  moveFrontstageBlockNode: vi.fn(),
  saveFrontstageBlockNodeCode: vi.fn(),
  updateFrontstageBlockNode: vi.fn()
}));

vi.mock('../api/block-tree', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api/block-tree')>()),
  ...blockTreeApi
}));

function authenticate() {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'actor-1',
      account: 'normal-user',
      effective_display_role: 'developer',
      current_workspace_id: 'workspace-1'
    },
    me: null
  });
}

function setup() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false }
    }
  });
  const invalidateQueries = vi
    .spyOn(queryClient, 'invalidateQueries')
    .mockResolvedValue();
  const removeQueries = vi.spyOn(queryClient, 'removeQueries');
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  const view = renderHook(
    () => useFrontstageBlockTreeMutations('workspace-1', 'page-1'),
    { wrapper }
  );
  return { invalidateQueries, removeQueries, result: view.result };
}

describe('frontstage block tree feature mutations', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetAuthStore();
    authenticate();
    blockTreeApi.createFrontstageBlockNode.mockResolvedValue({
      block_id: 'created',
      parent_block_id: 'parent-new'
    });
    blockTreeApi.moveFrontstageBlockNode.mockResolvedValue({
      block_id: 'moving',
      parent_block_id: 'parent-new'
    });
    blockTreeApi.deleteFrontstageBlockLeaf.mockResolvedValue(undefined);
  });

  test('AC-009 keeps query ownership page-scoped', () => {
    expect(frontstageBlockTreeQueryKeys.roots('workspace-1', 'page-1')).toEqual([
      'frontstage',
      'workspace-1',
      'pages',
      'page-1',
      'block-tree',
      'roots',
      {}
    ]);
    expect(
      frontstageBlockTreeQueryKeys.block(
        'workspace-1',
        'page-1',
        'public-block-id'
      )
    ).toEqual([
      'frontstage',
      'workspace-1',
      'pages',
      'page-1',
      'block-tree',
      'blocks',
      'public-block-id',
      'detail'
    ]);
  });

  test('AC-009 refreshes only affected owners, detail and search after create/move/delete', async () => {
    const { invalidateQueries, removeQueries, result } = setup();
    const createInput = {
      tab_id: 'tab-1',
      title: 'Created',
      presentation: 'page' as const,
      parent_block_id: 'parent-new',
      before_block_id: null,
      after_block_id: null,
      code: 'export default Created',
      runtime_descriptor: null
    };

    await act(async () => {
      await result.current.create.mutateAsync(createInput);
      await result.current.move.mutateAsync({
        block_id: 'moving',
        previous_parent_block_id: 'parent-old',
        input: {
          parent_block_id: 'parent-new',
          before_block_id: null,
          after_block_id: 'created'
        }
      });
      await result.current.deleteLeaf.mutateAsync({
        block_id: 'moving',
        parent_block_id: 'parent-new'
      });
    });

    expect(blockTreeApi.createFrontstageBlockNode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      createInput,
      'csrf-123'
    );
    expect(blockTreeApi.moveFrontstageBlockNode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'moving',
      {
        parent_block_id: 'parent-new',
        before_block_id: null,
        after_block_id: 'created'
      },
      'csrf-123'
    );
    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: frontstageBlockTreeQueryKeys.children(
        'workspace-1',
        'page-1',
        'parent-old'
      ),
      refetchType: 'active'
    });
    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: frontstageBlockTreeQueryKeys.children(
        'workspace-1',
        'page-1',
        'parent-new'
      ),
      refetchType: 'active'
    });
    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: frontstageBlockTreeQueryKeys.block(
        'workspace-1',
        'page-1',
        'moving'
      ),
      refetchType: 'active'
    });
    expect(removeQueries).toHaveBeenCalledWith({
      queryKey: frontstageBlockTreeQueryKeys.code(
        'workspace-1',
        'page-1',
        'moving'
      )
    });
    expect(invalidateQueries).not.toHaveBeenCalledWith(
      expect.objectContaining({
        queryKey: frontstageBlockTreeQueryKeys.page(
          'workspace-1',
          'page-1'
        )
      })
    );
  });

  test('AC-009 removes the embedded child-container save entry from Studio', async () => {
    const drawerSource = await readFile(
      new URL(
        '../components/jsx-studio/FrontstageJsxStudioDrawer.tsx',
        import.meta.url
      ),
      'utf8'
    );
    const resourceSource = await readFile(
      new URL(
        '../components/jsx-studio/JsxStudioResourcePanel.tsx',
        import.meta.url
      ),
      'utf8'
    );

    expect(drawerSource).not.toContain("'child-containers'");
    expect(drawerSource).not.toContain('onSaveChildContainers');
    expect(resourceSource).not.toContain('JsxStudioChildContainersPanel');
    expect(resourceSource).not.toContain('onSaveChildContainers');
  });
});
