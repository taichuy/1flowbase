import { beforeEach, describe, expect, test, vi } from 'vitest';

const apiClient = vi.hoisted(() => ({
  fetchPublicLoginEntries: vi.fn()
}));

vi.mock('@1flowbase/api-client/auth', () => ({
  deleteConsoleSession: vi.fn(),
  fetchConsoleMe: vi.fn(),
  fetchConsoleSession: vi.fn(),
  fetchPublicLoginEntries: apiClient.fetchPublicLoginEntries,
  getDefaultApiBaseUrl: vi.fn(() => 'http://127.0.0.1:7800'),
  signInWithPassword: vi.fn(),
  switchConsoleSessionRole: vi.fn()
}));

import { fetchLoginEntries } from '../api/session';

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
});
