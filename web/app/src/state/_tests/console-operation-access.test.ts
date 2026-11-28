import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import { evaluateConsoleAccess } from '@1flowbase/api-client/auth';

import { resetAuthStore, useAuthStore } from '../auth-store';
import {
  ensureConsoleOperationAccess,
  useConsoleOperationAccess,
  useConsoleOperationAccessStatus,
  useProjectedConsoleOperationAccess
} from '../console-operation-access';

vi.mock('@1flowbase/api-client/auth', () => ({
  evaluateConsoleAccess: vi.fn()
}));
const evaluate = vi.mocked(evaluateConsoleAccess);

function authenticate(
  role = 'member',
  fingerprint = 'v1',
  workspace = 'workspace'
) {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf',
    actor: {
      id: 'user',
      account: 'member',
      current_workspace_id: workspace,
      effective_display_role: role
    },
    me: {
      id: 'user',
      account: 'member',
      email: '',
      nickname: '',
      name: '',
      introduction: '',
      avatar_url: null,
      phone: null,
      effective_display_role: role,
      policy_fingerprint: fingerprint
    }
  });
}

beforeEach(() => {
  resetAuthStore();
  evaluate.mockReset();
  authenticate();
});

it('batches, deduplicates, reports loading, then shares positive and negative decisions', async () => {
  evaluate.mockResolvedValue({
    policy_fingerprint: 'v1',
    allowed: { read: true, write: false }
  });
  const { result } = renderHook(() => {
    const can = useConsoleOperationAccess();
    return [can('read'), can('read'), useConsoleOperationAccessStatus('write')];
  });
  expect(result.current).toEqual([false, false, 'loading']);
  expect(evaluate).not.toHaveBeenCalled();
  await waitFor(() => expect(result.current).toEqual([true, true, 'denied']));
  expect(evaluate).toHaveBeenCalledExactlyOnceWith(
    ['read', 'write'],
    'csrf',
    import.meta.env.VITE_API_BASE_URL
  );
  expect(
    await ensureConsoleOperationAccess(['read', 'read', ' ', 'write'])
  ).toEqual({ read: true, write: false });
  expect(evaluate).toHaveBeenCalledTimes(1);
});

it('invalidates the previous scope and discards late responses after a workspace change', async () => {
  let finish!: (
    value: Awaited<ReturnType<typeof evaluateConsoleAccess>>
  ) => void;
  evaluate.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      })
  );
  evaluate.mockResolvedValue({
    policy_fingerprint: 'v2',
    allowed: { write: false }
  });
  const late = ensureConsoleOperationAccess(['write']);
  await Promise.resolve();
  authenticate('member', 'v2', 'other');
  expect(await late).toEqual({ write: false });
  finish({ policy_fingerprint: 'v1', allowed: { write: true } });
  expect(await ensureConsoleOperationAccess(['write'])).toEqual({
    write: false
  });
  expect(evaluate).toHaveBeenCalledTimes(2);
});

it('drops cached grants when the profile fingerprint changes', async () => {
  evaluate.mockResolvedValueOnce({
    policy_fingerprint: 'v1',
    allowed: { write: true }
  });
  evaluate.mockResolvedValueOnce({
    policy_fingerprint: 'v2',
    allowed: { write: false }
  });
  expect(await ensureConsoleOperationAccess(['write'])).toEqual({
    write: true
  });
  authenticate('member', 'v2');
  expect(await ensureConsoleOperationAccess(['write'])).toEqual({
    write: false
  });
  expect(evaluate).toHaveBeenCalledTimes(2);
});

it('discards previous decisions when a later batch observes a new server fingerprint', async () => {
  evaluate.mockResolvedValueOnce({
    policy_fingerprint: 'v1',
    allowed: { write: true }
  });
  evaluate.mockResolvedValueOnce({
    policy_fingerprint: 'v2',
    allowed: { read: true }
  });
  evaluate.mockResolvedValueOnce({
    policy_fingerprint: 'v2',
    allowed: { write: false }
  });
  expect(await ensureConsoleOperationAccess(['write'])).toEqual({
    write: true
  });
  expect(await ensureConsoleOperationAccess(['write', 'read'])).toEqual({
    write: false,
    read: true
  });
  expect(await ensureConsoleOperationAccess(['write'])).toEqual({
    write: false
  });
});

it('uses the root fast path only for non-projected checks and drops decisions on logout', async () => {
  authenticate('root');
  evaluate.mockResolvedValue({
    policy_fingerprint: 'v1',
    allowed: { host: false }
  });
  const { result } = renderHook(() => {
    const can = useConsoleOperationAccess();
    const projected = useProjectedConsoleOperationAccess();
    return [can('ordinary'), projected('host')];
  });
  expect(result.current).toEqual([true, false]);
  await waitFor(() =>
    expect(evaluate).toHaveBeenCalledExactlyOnceWith(
      ['host'],
      'csrf',
      import.meta.env.VITE_API_BASE_URL
    )
  );
  act(() => useAuthStore.getState().setAnonymous());
  expect(result.current).toEqual([false, false]);
});

it('fails closed and suppresses repeated failed requests without claiming a backend denial', async () => {
  evaluate.mockRejectedValue(new Error('offline'));
  expect(await ensureConsoleOperationAccess(['write'])).toEqual({
    write: false
  });
  const { result } = renderHook(() => useConsoleOperationAccessStatus('write'));
  expect(result.current).toBe('loading');
  expect(await ensureConsoleOperationAccess(['write'])).toEqual({
    write: false
  });
  expect(evaluate).toHaveBeenCalledTimes(1);
});
