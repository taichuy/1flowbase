import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';
import { sha256Text } from '@1flowbase/page-runtime';

import {
  resetAuthStore,
  useAuthStore
} from '../../../../../../state/auth-store';
import type { FrontstageBlockNode } from '../../../../api/block-tree';
import {
  useFrontstageBlockTabs,
  type FrontstageExecutableSavePayload
} from '../use-frontstage-block-tabs';

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
    code_ref: `frontstage.block.${blockId}`,
    input_mapping: {},
    output_mapping: {},
    runtime_descriptor: null,
    created_at: '2026-08-12T00:00:00Z',
    updated_at: '2026-08-12T00:00:00Z'
  };
}

function executableCode(blockId: string, source = `source:${blockId}`) {
  return {
    block_id: blockId,
    page_id: 'page-1',
    source_code: source,
    source_sha256: sha256Text(source),
    dependency_lock: []
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
      async (_workspaceId: string, _pageId: string, blockId: string) =>
        executableCode(blockId)
    );
    api.saveFrontstageBlockNodeCode.mockImplementation(
      async (
        _workspaceId: string,
        _pageId: string,
        blockId: string,
        input: FrontstageExecutableSavePayload
      ) => ({
        ...executableCode(blockId, input.source_code),
        ...input
      })
    );
  });

  test('AC-005 lazily opens real IDs and retains independent drafts across switches', async () => {
    const view = setup();
    await waitFor(() =>
      expect(view.result.current.activeTab?.loading).toBe(false)
    );

    act(() => view.result.current.setActiveDraft('draft:root'));
    act(() => view.result.current.openBlock('child'));
    await waitFor(() =>
      expect(view.result.current.activeTab?.block_id).toBe('child')
    );
    await waitFor(() =>
      expect(view.result.current.activeTab?.loading).toBe(false)
    );
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
    await waitFor(() =>
      expect(view.result.current.activeTab?.loading).toBe(false)
    );
    act(() => view.result.current.openBlock('child'));
    await waitFor(() =>
      expect(view.result.current.activeTab?.loading).toBe(false)
    );
    act(() => view.result.current.activateBlock('root'));

    act(() => view.result.current.setDraft('child', ''));

    expect(view.result.current.activeTab?.draft).toBe('source:root');
    expect(
      view.result.current.tabs.find((tab) => tab.block_id === 'child')?.draft
    ).toBe('');
  });

  test('AC-005 saves and resets only the active public block tab', async () => {
    const savedSource = 'export default function SavedRoot() { return null; }';
    const view = setup();
    await waitFor(() =>
      expect(view.result.current.activeTab?.loading).toBe(false)
    );
    act(() => view.result.current.setActiveDraft(savedSource));

    await act(async () => {
      await view.result.current.saveActiveDraft();
    });
    expect(api.saveFrontstageBlockNodeCode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'root',
      {
        source_code: savedSource,
        expected_source_revision: sha256Text('source:root'),
        dependency_lock: []
      },
      'csrf-123'
    );
    expect(view.result.current.activeTab?.base_source).toBe(savedSource);
    expect(view.result.current.activeTab?.source_sha256).toBe(
      sha256Text(savedSource)
    );

    act(() => view.result.current.setActiveDraft('discard me'));
    act(() => view.result.current.resetActive());
    expect(view.result.current.activeTab?.draft).toBe(savedSource);
  });

  test('AC-005 converges subtree deletion through open-detail 404s', async () => {
    const view = setup();
    await waitFor(() =>
      expect(view.result.current.activeTab?.loading).toBe(false)
    );
    act(() => view.result.current.openBlock('branch'));
    await waitFor(() =>
      expect(view.result.current.activeTab?.loading).toBe(false)
    );
    act(() => view.result.current.openBlock('nested'));
    await waitFor(() =>
      expect(view.result.current.activeTab?.loading).toBe(false)
    );

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
    await waitFor(() =>
      expect(view.result.current.activeTab?.loading).toBe(false)
    );
    await expect(
      view.result.current.handleDeletedBlock({
        block_id: 'root',
        subtree: true
      })
    ).resolves.toBe('initial_root_deleted');
  });

  test('AC-005 preserves 404 load and 403 save errors without fallback', async () => {
    const view = setup();
    await waitFor(() =>
      expect(view.result.current.activeTab?.loading).toBe(false)
    );
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
    const unsavedSource =
      'export default function UnsavedRoot() { return null; }';
    act(() => view.result.current.setActiveDraft(unsavedSource));
    api.saveFrontstageBlockNodeCode.mockRejectedValueOnce(
      Object.assign(new Error('forbidden'), { status: 403 })
    );
    await act(async () => {
      await view.result.current.saveActiveDraft().catch(() => undefined);
    });
    expect(
      (view.result.current.activeTab?.error as { status?: number })?.status
    ).toBe(403);
    expect(view.result.current.activeTab?.draft).toBe(unsavedSource);
  });

  test('AC-005 stays on the public block-id dependency boundary', async () => {
    const view = setup();
    await waitFor(() =>
      expect(view.result.current.activeTab?.loading).toBe(false)
    );

    expect(api.fetchFrontstageBlockNodeCode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'root'
    );
    expect(legacy.useFrontstageBlockCode).not.toHaveBeenCalled();
  });

  test('source save removes the compile-only Tailwind module from runtime dependencies', async () => {
    const persistedDependencyLock = [
      {
        module_source: 'react',
        module_version: '18.3.1',
        binding: 'host' as const,
        assets: [],
        exports: ['default']
      },
      {
        module_source: 'tailwindcss',
        module_version: '4.3.3',
        binding: 'fetched' as const,
        assets: [
          {
            role: 'browser_module' as const,
            media_type: 'text/javascript',
            sha256: 'a'.repeat(64),
            url: `/locked-tailwind-${'a'.repeat(64)}`
          }
        ],
        exports: ['default']
      }
    ];
    api.fetchFrontstageBlockNodeCode.mockResolvedValueOnce({
      ...executableCode('root'),
      dependency_lock: persistedDependencyLock
    });
    const view = setup();
    await waitFor(() =>
      expect(view.result.current.activeTab?.loading).toBe(false)
    );
    act(() =>
      view.result.current.setActiveDraft(
        'import \'tailwindcss\'; export default () => <div className="p-4" />;'
      )
    );
    await act(async () => {
      await view.result.current.saveActiveDraft();
    });
    expect(api.saveFrontstageBlockNodeCode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'root',
      expect.objectContaining({
        source_code:
          'import \'tailwindcss\'; export default () => <div className="p-4" />;',
        expected_source_revision: sha256Text('source:root'),
        dependency_lock: [persistedDependencyLock[0]]
      }),
      'csrf-123'
    );
  });

  test.each([
    ['malformed dependency lock', { dependency_lock: [{ bad: true }] }]
  ])('fails closed on %s without invoking save', async (_label, override) => {
    api.fetchFrontstageBlockNodeCode.mockResolvedValueOnce({
      ...executableCode('root'),
      ...override
    });
    const view = setup();
    await waitFor(() =>
      expect(view.result.current.activeTab?.loading).toBe(false)
    );
    await act(async () => {
      await view.result.current.saveActiveDraft().catch(() => undefined);
    });
    expect(api.saveFrontstageBlockNodeCode).not.toHaveBeenCalled();
    expect(view.result.current.activeTab?.error).toBeInstanceOf(Error);
  });

  test('passes the loaded source revision so the backend can reject a stale save', async () => {
    api.fetchFrontstageBlockNodeCode.mockResolvedValueOnce({
      ...executableCode('root'),
      source_sha256: '0'.repeat(64)
    });
    const view = setup();
    await waitFor(() =>
      expect(view.result.current.activeTab?.loading).toBe(false)
    );

    await act(async () => {
      await view.result.current.saveActiveDraft();
    });
    expect(api.saveFrontstageBlockNodeCode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'root',
      expect.objectContaining({ expected_source_revision: '0'.repeat(64) }),
      'csrf-123'
    );
  });
});
