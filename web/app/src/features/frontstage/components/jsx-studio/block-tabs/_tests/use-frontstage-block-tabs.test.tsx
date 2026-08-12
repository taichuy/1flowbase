import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { resetAuthStore, useAuthStore } from '../../../../../../state/auth-store';
import type { FrontstageBlockNode } from '../../../../api/block-tree';
import { useFrontstageBlockTabs } from '../use-frontstage-block-tabs';

const api = vi.hoisted(() => ({
  fetchFrontstageBlockNode: vi.fn(),
  fetchFrontstageBlockNodeCode: vi.fn(),
  saveFrontstageBlockNodeCode: vi.fn()
}));

const legacy = vi.hoisted(() => ({
  useFrontstageBlockCode: vi.fn()
}));

vi.mock('../../../../api/block-tree', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../../../api/block-tree')>()),
  ...api
}));

vi.mock('../../../../hooks/use-frontstage-block-code', () => legacy);

function detail(blockId: string): FrontstageBlockNode {
  return {
    block_id: blockId,
    workspace_id: 'workspace-1',
    page_id: 'page-1',
    tab_id: 'tab-1',
    parent_block_id: blockId === 'root' ? null : 'root',
    rank: '001000',
    presentation: 'page',
    title: `Title ${blockId}`,
    description: null,
    schema_version: 1,
    input_mapping: {},
    output_mapping: {},
    runtime_descriptor: null,
    created_at: '2026-08-12T00:00:00Z',
    updated_at: '2026-08-12T00:00:00Z'
  };
}

function setup() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  return renderHook(
    () =>
      useFrontstageBlockTabs({
        workspaceId: 'workspace-1',
        pageId: 'page-1',
        initialBlockId: 'root',
        open: true
      }),
    { wrapper }
  );
}

describe('useFrontstageBlockTabs', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetAuthStore();
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
    api.fetchFrontstageBlockNode.mockImplementation(
      async (_workspaceId: string, _pageId: string, blockId: string) =>
        detail(blockId)
    );
    api.fetchFrontstageBlockNodeCode.mockImplementation(
      async (_workspaceId: string, _pageId: string, blockId: string) => ({
        block_id: blockId,
        page_id: 'page-1',
        code: `source:${blockId}`,
        source_sha256: `hash:${blockId}`
      })
    );
    api.saveFrontstageBlockNodeCode.mockImplementation(
      async (
        _workspaceId: string,
        _pageId: string,
        blockId: string,
        input: { code: string }
      ) => ({
        block_id: blockId,
        page_id: 'page-1',
        code: input.code,
        source_sha256: `saved:${blockId}`
      })
    );
  });

  test('AC-005 lazily opens real IDs and retains independent drafts across switches', async () => {
    const view = setup();
    await waitFor(() => expect(view.result.current.activeTab?.loading).toBe(false));

    act(() => view.result.current.setActiveDraft('draft:root'));
    act(() => view.result.current.openBlock('child'));
    await waitFor(() =>
      expect(view.result.current.activeTab?.block_id).toBe('child')
    );
    await waitFor(() => expect(view.result.current.activeTab?.loading).toBe(false));
    act(() => view.result.current.setActiveDraft('draft:child'));
    act(() => view.result.current.activateBlock('root'));

    expect(view.result.current.activeTab?.draft).toBe('draft:root');
    expect(view.result.current.anyDirty).toBe(true);
    expect(api.fetchFrontstageBlockNode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'child'
    );
    expect(api.fetchFrontstageBlockNodeCode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'child'
    );
  });

  test('AC-001 updates the draft owned by an identified editor document', async () => {
    const view = setup();
    await waitFor(() => expect(view.result.current.activeTab?.loading).toBe(false));
    act(() => view.result.current.openBlock('child'));
    await waitFor(() => expect(view.result.current.activeTab?.loading).toBe(false));
    act(() => view.result.current.activateBlock('root'));

    act(() => view.result.current.setDraft('child', ''));

    expect(view.result.current.activeTab?.draft).toBe('source:root');
    expect(
      view.result.current.tabs.find((tab) => tab.block_id === 'child')?.draft
    ).toBe('');
  });

  test('AC-005 saves and resets only the active public block tab', async () => {
    const view = setup();
    await waitFor(() => expect(view.result.current.activeTab?.loading).toBe(false));
    act(() => view.result.current.setActiveDraft('saved root source'));

    await act(async () => {
      await view.result.current.saveActive();
    });
    expect(api.saveFrontstageBlockNodeCode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'root',
      { code: 'saved root source' },
      'csrf-123'
    );
    expect(view.result.current.activeTab?.base_source).toBe(
      'saved root source'
    );
    expect(view.result.current.activeTab?.source_sha256).toBe('saved:root');

    act(() => view.result.current.setActiveDraft('discard me'));
    act(() => view.result.current.resetActive());
    expect(view.result.current.activeTab?.draft).toBe('saved root source');
  });

  test('AC-005 converges subtree deletion through open-detail 404s', async () => {
    const view = setup();
    await waitFor(() => expect(view.result.current.activeTab?.loading).toBe(false));
    act(() => view.result.current.openBlock('branch'));
    await waitFor(() => expect(view.result.current.activeTab?.loading).toBe(false));
    act(() => view.result.current.openBlock('nested'));
    await waitFor(() => expect(view.result.current.activeTab?.loading).toBe(false));

    api.fetchFrontstageBlockNode.mockImplementation(
      async (_workspaceId: string, _pageId: string, blockId: string) => {
        if (blockId === 'nested') {
          throw Object.assign(new Error('not found'), { status: 404 });
        }
        return detail(blockId);
      }
    );
    await act(async () => {
      await view.result.current.handleDeletedBlock({
        block_id: 'branch',
        subtree: true
      });
    });

    expect(view.result.current.tabs.map((tab) => tab.block_id)).toEqual([
      'root'
    ]);
    expect(view.result.current.activeBlockId).toBe('root');
  });

  test('AC-005 reports initial root deletion to the Studio owner', async () => {
    const view = setup();
    await waitFor(() => expect(view.result.current.activeTab?.loading).toBe(false));
    await expect(
      view.result.current.handleDeletedBlock({
        block_id: 'root',
        subtree: true
      })
    ).resolves.toBe('initial_root_deleted');
  });

  test('AC-005 preserves 404 load and 403 save errors without fallback', async () => {
    const view = setup();
    await waitFor(() => expect(view.result.current.activeTab?.loading).toBe(false));
    api.fetchFrontstageBlockNode.mockRejectedValueOnce(
      Object.assign(new Error('missing block'), { status: 404 })
    );
    act(() => view.result.current.openBlock('missing'));
    await waitFor(() => {
      expect(
        (view.result.current.activeTab?.error as { status?: number })?.status
      ).toBe(404);
    });

    act(() => view.result.current.activateBlock('root'));
    act(() => view.result.current.setActiveDraft('unsaved root'));
    api.saveFrontstageBlockNodeCode.mockRejectedValueOnce(
      Object.assign(new Error('forbidden'), { status: 403 })
    );
    await act(async () => {
      await view.result.current.saveActive().catch(() => undefined);
    });
    expect(
      (view.result.current.activeTab?.error as { status?: number })?.status
    ).toBe(403);
    expect(view.result.current.activeTab?.draft).toBe('unsaved root');
  });

  test('AC-005 stays on the public block-id dependency boundary', async () => {
    const view = setup();
    await waitFor(() => expect(view.result.current.activeTab?.loading).toBe(false));

    expect(api.fetchFrontstageBlockNodeCode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'root'
    );
    expect(legacy.useFrontstageBlockCode).not.toHaveBeenCalled();
  });
});
