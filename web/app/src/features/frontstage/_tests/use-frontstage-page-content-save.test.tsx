import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { useFrontstagePageContentSave } from '../hooks/use-frontstage-page-content-save';

const frontstageApi = vi.hoisted(() => ({
  frontstagePageContentQueryKey: vi.fn(
    (workspaceId: string, pageId: string, tabId: string) =>
      ['frontstage', workspaceId, 'pages', pageId, 'tabs', tabId, 'content'] as const
  ),
  saveFrontstagePageContent: vi.fn()
}));

vi.mock('../api/page-content', () => frontstageApi);

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

function createPageContent(title = '页面 1') {
  return {
    page: {
      id: 'page-1',
      title,
      kind: 'page' as const,
      parentId: null,
      rank: '001000',
    },
    schema: {
      rootUid: 'root-1',
      payload: { blocks: [{ uid: 'hero' }] }
    },
    root: {
      uid: 'root-1',
      payload: {
        kind: 'frontstage.page.root',
        children: ['hero']
      }
    }
  };
}

function createSaveInput() {
  return {
    schema: {
      payload: { blocks: [{ uid: 'hero' }] }
    },
    root: {
      payload: {
        kind: 'frontstage.page.root',
        children: ['hero']
      }
    }
  };
}

function setupSave(queryClient = createQueryClient()) {
  const invalidateQueriesSpy = vi
    .spyOn(queryClient, 'invalidateQueries')
    .mockResolvedValue();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  const view = renderHook(
    () =>
      useFrontstagePageContentSave({
        workspaceId: 'workspace-1',
        pageId: 'page-1',
        tabId: 'tab-1'
      }),
    { wrapper }
  );

  return {
    invalidateQueriesSpy,
    queryClient,
    result: view.result
  };
}

describe('useFrontstagePageContentSave', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetAuthStore();
    authenticate();
    frontstageApi.saveFrontstagePageContent.mockResolvedValue(
      createPageContent('页面 已保存')
    );
  });

  test('saves page content with csrf token, writes the query cache, and invalidates active page content queries', async () => {
    const { invalidateQueriesSpy, queryClient, result } = setupSave();
    const input = createSaveInput();
    let savedContent: unknown;

    await act(async () => {
      savedContent = await result.current.save(input);
    });

    const queryKey = [
      'frontstage',
      'workspace-1',
      'pages',
      'page-1',
      'tabs',
      'tab-1',
      'content'
    ];

    expect(frontstageApi.saveFrontstagePageContent).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'tab-1',
      input,
      'csrf-123'
    );
    expect(savedContent).toEqual(createPageContent('页面 已保存'));
    expect(queryClient.getQueryData(queryKey)).toEqual(
      createPageContent('页面 已保存')
    );
    expect(invalidateQueriesSpy).toHaveBeenCalledWith({
      queryKey,
      refetchType: 'active'
    });
    expect(result.current.saving).toBe(false);
    expect(result.current.isPending).toBe(false);
    expect(result.current.error).toBeNull();
  });

  test('page content save rejects missing csrf token before calling feature api', async () => {
    authenticate(null);
    const { result } = setupSave();
    let saveError: unknown;

    await act(async () => {
      await result.current.save(createSaveInput()).catch((error: unknown) => {
        saveError = error;
      });
    });

    expect(saveError).toEqual(new Error('missing csrf token'));
    expect(frontstageApi.saveFrontstagePageContent).not.toHaveBeenCalled();
    await waitFor(() => {
      expect(result.current.error).toEqual(new Error('missing csrf token'));
    });
  });

  test('exposes mutation failures and clears them on reset or clearError', async () => {
    const firstFailure = new Error('save failed');
    const secondFailure = new Error('save failed again');
    frontstageApi.saveFrontstagePageContent
      .mockRejectedValueOnce(firstFailure)
      .mockRejectedValueOnce(secondFailure);
    const { result } = setupSave();

    await act(async () => {
      await result.current.save(createSaveInput()).catch(() => undefined);
    });

    await waitFor(() => {
      expect(result.current.error).toBe(firstFailure);
    });

    act(() => {
      result.current.clearError();
    });

    expect(result.current.error).toBeNull();

    await act(async () => {
      await result.current.save(createSaveInput()).catch(() => undefined);
    });

    await waitFor(() => {
      expect(result.current.error).toBe(secondFailure);
    });

    act(() => {
      result.current.reset();
    });

    expect(result.current.error).toBeNull();
  });
});
