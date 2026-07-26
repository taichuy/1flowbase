import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { useFrontstageBlockCatalog } from '../hooks/use-frontstage-block-catalog';
import type { FrontstageBlockCatalogEntry } from '../api/block-catalog';
import type {
  FrontstageBlockCatalogDiagnostic,
  NormalizedFrontstageBlockCatalogEntry
} from '../lib/block-catalog';

const frontstageApi = vi.hoisted(() => ({
  fetchFrontstageBlockCatalog: vi.fn(),
  frontstageBlockCatalogQueryKeyPrefix: ['frontstage', 'block-catalog'],
  frontstageBlockCatalogQueryKey: vi.fn(
    ({ workspaceId, actorId, permissionFingerprint }) =>
      [
        'frontstage',
        'block-catalog',
        workspaceId,
        actorId,
        permissionFingerprint
      ] as const
  )
}));

const blockCatalogLib = vi.hoisted(() => ({
  normalizeFrontstageBlockCatalog: vi.fn()
}));

vi.mock('../api/block-catalog', () => frontstageApi);
vi.mock('../lib/block-catalog', () => blockCatalogLib);

function createQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false }
    }
  });
}

function authenticate({
  workspaceId = 'workspace-1',
  actorId = 'actor-1',
  permissions = ['frontstage.page.design']
}: {
  workspaceId?: string;
  actorId?: string;
  permissions?: string[];
} = {}) {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: actorId,
      account: `${actorId}@example.com`,
      effective_display_role: 'member',
      current_workspace_id: workspaceId
    },
    me: {
      id: actorId,
      account: `${actorId}@example.com`,
      email: `${actorId}@example.com`,
      phone: null,
      nickname: actorId,
      name: actorId,
      avatar_url: null,
      introduction: '',
      effective_display_role: 'member',
      permissions
    }
  });
}

function setupCatalog(
  workspaceId = 'workspace-1',
  queryClient = createQueryClient()
) {
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  return renderHook(
    ({ activeWorkspaceId }) =>
      useFrontstageBlockCatalog({ workspaceId: activeWorkspaceId }),
    { initialProps: { activeWorkspaceId: workspaceId }, wrapper }
  );
}

describe('useFrontstageBlockCatalog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetAuthStore();
    authenticate();
    frontstageApi.fetchFrontstageBlockCatalog.mockResolvedValue([]);
    blockCatalogLib.normalizeFrontstageBlockCatalog.mockReturnValue({
      items: [],
      diagnostics: []
    });
  });

  test('waits for complete authenticated workspace context', () => {
    resetAuthStore();

    const { result } = setupCatalog();

    expect(frontstageApi.fetchFrontstageBlockCatalog).not.toHaveBeenCalled();
    expect(result.current.items).toEqual([]);
    expect(result.current.fetchStatus).toBe('idle');
  });

  test('removes actor-scoped catalog caches after logout', async () => {
    const queryClient = createQueryClient();
    const removeSpy = vi.spyOn(queryClient, 'removeQueries');
    const { rerender } = setupCatalog('workspace-1', queryClient);

    await waitFor(() => {
      expect(frontstageApi.fetchFrontstageBlockCatalog).toHaveBeenCalledTimes(
        1
      );
    });

    act(() => useAuthStore.getState().setAnonymous());
    rerender({ activeWorkspaceId: 'workspace-1' });

    await waitFor(() => {
      expect(removeSpy).toHaveBeenCalledWith({
        queryKey: frontstageApi.frontstageBlockCatalogQueryKeyPrefix
      });
    });
  });

  test('does not reuse catalog data across workspace, actor, or permission changes', async () => {
    const workspaceOneItems = [{ id: 'workspace-1-item' }];
    const workspaceTwoItems = [{ id: 'workspace-2-item' }];
    const actorTwoItems = [{ id: 'actor-2-item' }];
    const permissionItems = [{ id: 'permission-item' }];
    blockCatalogLib.normalizeFrontstageBlockCatalog
      .mockReturnValueOnce({ items: workspaceOneItems, diagnostics: [] })
      .mockReturnValueOnce({ items: workspaceTwoItems, diagnostics: [] })
      .mockReturnValueOnce({ items: actorTwoItems, diagnostics: [] })
      .mockReturnValueOnce({ items: permissionItems, diagnostics: [] });

    const { result, rerender } = setupCatalog();

    await waitFor(() => expect(result.current.items).toBe(workspaceOneItems));

    rerender({ activeWorkspaceId: 'workspace-2' });
    expect(result.current.items).toEqual([]);
    expect(frontstageApi.fetchFrontstageBlockCatalog).toHaveBeenCalledTimes(1);

    act(() => authenticate({ workspaceId: 'workspace-2' }));
    rerender({ activeWorkspaceId: 'workspace-2' });
    expect(result.current.items).toEqual([]);
    await waitFor(() => expect(result.current.items).toBe(workspaceTwoItems));

    act(() => authenticate({ workspaceId: 'workspace-2', actorId: 'actor-2' }));
    rerender({ activeWorkspaceId: 'workspace-2' });
    expect(result.current.items).toEqual([]);
    await waitFor(() => expect(result.current.items).toBe(actorTwoItems));

    act(() =>
      authenticate({
        workspaceId: 'workspace-2',
        actorId: 'actor-2',
        permissions: []
      })
    );
    rerender({ activeWorkspaceId: 'workspace-2' });
    expect(result.current.items).toEqual([]);
    await waitFor(() => expect(result.current.items).toBe(permissionItems));

    expect(frontstageApi.fetchFrontstageBlockCatalog).toHaveBeenCalledTimes(4);
    expect(
      frontstageApi.frontstageBlockCatalogQueryKey
    ).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({
        workspaceId: 'workspace-1',
        actorId: 'actor-1'
      })
    );
    expect(frontstageApi.frontstageBlockCatalogQueryKey).toHaveBeenCalledWith(
      expect.objectContaining({
        workspaceId: 'workspace-2',
        actorId: 'actor-2',
        permissionFingerprint: 'role:member|permissions:'
      })
    );
  });

  test('fetches the console catalog and exposes the normalized block catalog', async () => {
    const rawEntries: FrontstageBlockCatalogEntry[] = [
      {
        installation_id: 'installation-1',
        provider_code: 'official',
        plugin_id: 'official.blocks',
        plugin_version: '1.0.0',
        contribution_code: 'hero_banner',
        title: 'Hero Banner',
        runtime: 'native_react',
        entry: 'blocks/hero/index.html',
        code_modules: [],
        context_contract: {
          primitives: ['text'],
          input_schema: { type: 'object' }
        },
        permissions: {
          network: 'outbound_only',
          storage: 'none',
          secrets: 'none'
        },
        ui_capabilities: ['responsive']
      }
    ];
    const items = [
      {
        id: 'official:hero_banner',
        runtimeKind: 'native_react',
        installationId: 'installation-1',
        providerCode: 'official',
        pluginId: 'official.blocks',
        pluginVersion: '1.0.0',
        contributionCode: 'hero_banner',
        title: 'Hero Banner',
        entry: 'blocks/hero/index.html',
        permissions: {
          network: 'outbound_only',
          storage: 'none',
          secrets: 'none'
        },
        contextContract: {
          primitives: ['text'],
          inputSchema: { type: 'object' }
        },
        uiCapabilities: ['responsive'],
        raw: rawEntries[0]
      }
    ] satisfies NormalizedFrontstageBlockCatalogEntry[];
    const diagnostics = [
      {
        severity: 'warning',
        code: 'unknown_capability',
        providerCode: 'official',
        pluginId: 'official.blocks',
        contributionCode: 'hero_banner',
        field: 'ui_capabilities',
        value: 'legacy',
        message: 'Unsupported capability.'
      }
    ] satisfies FrontstageBlockCatalogDiagnostic[];

    frontstageApi.fetchFrontstageBlockCatalog.mockResolvedValue(rawEntries);
    blockCatalogLib.normalizeFrontstageBlockCatalog.mockReturnValue({
      items,
      diagnostics
    });

    const { result } = setupCatalog();

    await waitFor(() => {
      expect(result.current.items).toBe(items);
    });

    expect(frontstageApi.fetchFrontstageBlockCatalog).toHaveBeenCalledTimes(1);
    expect(
      blockCatalogLib.normalizeFrontstageBlockCatalog
    ).toHaveBeenCalledWith(rawEntries);
    expect(result.current.diagnostics).toBe(diagnostics);
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
    expect(result.current.status).toBe('success');
    expect(result.current.fetchStatus).toBe('idle');
    expect(result.current.isSuccess).toBe(true);
  });

  test('returns empty items and diagnostics for an empty catalog', async () => {
    const { result } = setupCatalog();

    await waitFor(() => {
      expect(result.current.isSuccess).toBe(true);
    });

    expect(frontstageApi.fetchFrontstageBlockCatalog).toHaveBeenCalledTimes(1);
    expect(
      blockCatalogLib.normalizeFrontstageBlockCatalog
    ).toHaveBeenCalledWith([]);
    expect(result.current.items).toEqual([]);
    expect(result.current.diagnostics).toEqual([]);
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  test('exposes query errors and supports refetching', async () => {
    const initialError = new Error('catalog unavailable');
    frontstageApi.fetchFrontstageBlockCatalog.mockRejectedValueOnce(
      initialError
    );

    const { result } = setupCatalog();

    await waitFor(() => {
      expect(result.current.error).toBe(initialError);
    });

    expect(result.current.items).toEqual([]);
    expect(result.current.diagnostics).toEqual([]);
    expect(result.current.isError).toBe(true);

    frontstageApi.fetchFrontstageBlockCatalog.mockResolvedValueOnce([]);

    await act(async () => {
      await result.current.refetch();
    });

    await waitFor(() => {
      expect(result.current.error).toBeNull();
    });

    expect(frontstageApi.fetchFrontstageBlockCatalog).toHaveBeenCalledTimes(2);
    expect(result.current.items).toEqual([]);
    expect(result.current.diagnostics).toEqual([]);
  });
});
