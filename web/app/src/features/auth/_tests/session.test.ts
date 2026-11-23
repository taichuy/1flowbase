import { beforeEach, describe, expect, test, vi } from 'vitest';

const apiClient = vi.hoisted(() => ({
  fetchPublicLoginEntries: vi.fn(),
  fetchConsoleWorkspaces: vi.fn(),
  switchConsoleSessionWorkspace: vi.fn(),
  redeemConsoleWorkspaceInvitation: vi.fn()
}));

vi.mock('@1flowbase/api-client/auth', () => ({
  deleteConsoleSession: vi.fn(),
  fetchConsoleMe: vi.fn(),
  fetchConsoleSession: vi.fn(),
  fetchConsoleWorkspaces: apiClient.fetchConsoleWorkspaces,
  fetchPublicLoginEntries: apiClient.fetchPublicLoginEntries,
  getDefaultApiBaseUrl: vi.fn(() => 'http://127.0.0.1:7800'),
  redeemConsoleWorkspaceInvitation: apiClient.redeemConsoleWorkspaceInvitation,
  signInWithPassword: vi.fn(),
  switchConsoleSessionRole: vi.fn(),
  switchConsoleSessionWorkspace: apiClient.switchConsoleSessionWorkspace
}));

import {
  fetchAccessibleWorkspaces,
  fetchLoginEntries,
  redeemWorkspaceInvitation,
  switchWorkspace
} from '../api/session';

describe('public login instance discovery', () => {
  beforeEach(() => {
    apiClient.fetchPublicLoginEntries.mockReset();
  });

  test('DV-F08 shares concurrent discovery and releases the completed flight', async () => {
    let resolveFirst: (value: {
      default_login_entry_id: string;
      login_entries: never[];
    }) => void = () => undefined;
    apiClient.fetchPublicLoginEntries.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveFirst = resolve;
        })
    );

    const first = fetchLoginEntries('http://example.test');
    const concurrent = fetchLoginEntries('http://example.test');

    expect(first).toBe(concurrent);
    expect(apiClient.fetchPublicLoginEntries).toHaveBeenCalledTimes(1);

    resolveFirst({ default_login_entry_id: 'password', login_entries: [] });
    await first;

    apiClient.fetchPublicLoginEntries.mockResolvedValueOnce({
      default_login_entry_id: 'password',
      login_entries: []
    });
    await fetchLoginEntries('http://example.test');

    expect(apiClient.fetchPublicLoginEntries).toHaveBeenCalledTimes(2);
  });

  test('forwards workspace list, switch, and invitation redemption to api-client', async () => {
    apiClient.fetchConsoleWorkspaces.mockResolvedValueOnce([
      {
        id: 'ws-1',
        name: 'Alpha',
        logo_url: null,
        introduction: '',
        is_current: true
      }
    ]);
    apiClient.switchConsoleSessionWorkspace.mockResolvedValueOnce({
      csrf_token: 'csrf-ws-2'
    });
    apiClient.redeemConsoleWorkspaceInvitation.mockResolvedValueOnce({
      csrf_token: 'csrf-inv'
    });

    await expect(
      fetchAccessibleWorkspaces('http://example.test')
    ).resolves.toHaveLength(1);
    expect(apiClient.fetchConsoleWorkspaces).toHaveBeenCalledWith(
      'http://example.test'
    );

    await switchWorkspace('ws-2', 'csrf-1', 'http://example.test');
    expect(apiClient.switchConsoleSessionWorkspace).toHaveBeenCalledWith(
      'ws-2',
      'csrf-1',
      'http://example.test'
    );

    await redeemWorkspaceInvitation('inv_123', 'csrf-1', 'http://example.test');
    expect(apiClient.redeemConsoleWorkspaceInvitation).toHaveBeenCalledWith(
      'inv_123',
      'csrf-1',
      'http://example.test'
    );
  });
});
