import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { useFrontstageBlockCode } from '../hooks/use-frontstage-block-code';

const frontstageApi = vi.hoisted(() => ({
  fetchFrontstageBlockCode: vi.fn(),
  frontstageBlockCodeQueryKey: vi.fn(
    (workspaceId: string, pageId: string, codeRef: string) =>
      [
        'frontstage',
        workspaceId,
        'pages',
        pageId,
        'block-code',
        codeRef
      ] as const
  ),
  saveFrontstageBlockCode: vi.fn()
}));

vi.mock('../api/block-code', () => frontstageApi);

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

function setupBlockCode(
  input: {
    workspaceId?: string | null;
    pageId?: string | null;
    codeRef?: string | null;
  } = {},
  queryClient = createQueryClient()
) {
  const invalidateQueriesSpy = vi
    .spyOn(queryClient, 'invalidateQueries')
    .mockResolvedValue();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  const view = renderHook(
    () =>
      useFrontstageBlockCode({
        workspaceId:
          input.workspaceId === undefined ? 'workspace-1' : input.workspaceId,
        pageId: input.pageId === undefined ? 'page-1' : input.pageId,
        codeRef: input.codeRef === undefined ? 'hero' : input.codeRef
      }),
    { wrapper }
  );

  return {
    invalidateQueriesSpy,
    result: view.result
  };
}

describe('useFrontstageBlockCode', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetAuthStore();
    authenticate();
    frontstageApi.fetchFrontstageBlockCode.mockResolvedValue({
      pageId: 'page-1',
      codeRef: 'hero',
      code: 'export default 1;'
    });
    frontstageApi.saveFrontstageBlockCode.mockResolvedValue({
      pageId: 'page-1',
      codeRef: 'hero',
      code: 'export default 2;'
    });
  });

  test('reads block code and exposes draft editing state', async () => {
    const { result } = setupBlockCode();

    await waitFor(() => {
      expect(result.current.code).toBe('export default 1;');
      expect(result.current.draft).toBe('export default 1;');
    });

    expect(frontstageApi.fetchFrontstageBlockCode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'hero'
    );
    expect(result.current.dirty).toBe(false);
    expect(result.current.loading).toBe(false);

    act(() => {
      result.current.setDraft('export default 2;');
    });

    expect(result.current.draft).toBe('export default 2;');
    expect(result.current.dirty).toBe(true);

    act(() => {
      result.current.reset();
    });

    expect(result.current.draft).toBe('export default 1;');
    expect(result.current.dirty).toBe(false);
  });

  test('does not request block code when pageId or codeRef is missing', async () => {
    const missingPage = setupBlockCode({ pageId: null });
    const missingCodeRef = setupBlockCode({ codeRef: null });

    await waitFor(() => {
      expect(missingPage.result.current.loading).toBe(false);
      expect(missingCodeRef.result.current.loading).toBe(false);
    });

    expect(frontstageApi.fetchFrontstageBlockCode).not.toHaveBeenCalled();
    expect(missingPage.result.current.code).toBe('');
    expect(missingCodeRef.result.current.draft).toBe('');
  });

  test('saves the current draft with csrf token and invalidates block code query', async () => {
    const { invalidateQueriesSpy, result } = setupBlockCode();

    await waitFor(() => {
      expect(result.current.code).toBe('export default 1;');
    });

    act(() => {
      result.current.setDraft('export default 2;');
    });

    await act(async () => {
      await result.current.save();
    });

    expect(frontstageApi.saveFrontstageBlockCode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      { codeRef: 'hero', code: 'export default 2;' },
      'csrf-123'
    );
    expect(invalidateQueriesSpy).toHaveBeenCalledWith({
      queryKey: [
        'frontstage',
        'workspace-1',
        'pages',
        'page-1',
        'block-code',
        'hero'
      ],
      refetchType: 'active'
    });
    await waitFor(() => {
      expect(result.current.code).toBe('export default 2;');
      expect(result.current.dirty).toBe(false);
    });
  });

  test('block code save rejects missing csrf token before calling feature api', async () => {
    authenticate(null);
    const { result } = setupBlockCode();
    let saveError: unknown;

    await waitFor(() => {
      expect(result.current.code).toBe('export default 1;');
    });

    await act(async () => {
      await result.current.save().catch((error: unknown) => {
        saveError = error;
      });
    });

    expect(saveError).toEqual(new Error('missing csrf token'));
    expect(frontstageApi.saveFrontstageBlockCode).not.toHaveBeenCalled();
    await waitFor(() => {
      expect(result.current.error).toEqual(new Error('missing csrf token'));
    });
  });

  test('marks block code read 403 as permission denied without changing the raw error', async () => {
    const forbiddenError = Object.assign(new Error('raw block permission detail'), {
      status: 403
    });
    frontstageApi.fetchFrontstageBlockCode.mockRejectedValue(forbiddenError);

    const { result } = setupBlockCode();

    await waitFor(() => {
      expect(result.current.error).toBe(forbiddenError);
      expect(result.current.permissionDenied).toBe(true);
    });
  });
});
