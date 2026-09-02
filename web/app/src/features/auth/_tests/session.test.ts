import { beforeEach, describe, expect, test, vi } from 'vitest';

const apiClient = vi.hoisted(() => ({
  fetchPublicLoginInstances: vi.fn()
}));

vi.mock('@1flowbase/api-client/auth', () => ({
  deleteConsoleSession: vi.fn(),
  fetchConsoleMe: vi.fn(),
  fetchConsoleSession: vi.fn(),
  fetchPublicLoginInstances: apiClient.fetchPublicLoginInstances,
  getDefaultApiBaseUrl: vi.fn(() => 'http://127.0.0.1:7800'),
  signInWithPassword: vi.fn(),
  switchConsoleSessionRole: vi.fn()
}));

import { fetchLoginInstances } from '../api/session';

describe('public login instance discovery', () => {
  beforeEach(() => {
    apiClient.fetchPublicLoginInstances.mockReset();
  });

  test('DV-F08 shares concurrent discovery and releases the completed flight', async () => {
    let resolveFirst: (value: {
      default_authenticator_id: string;
      login_instances: never[];
    }) => void = () => undefined;
    apiClient.fetchPublicLoginInstances.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveFirst = resolve;
        })
    );

    const first = fetchLoginInstances('http://example.test');
    const concurrent = fetchLoginInstances('http://example.test');

    expect(first).toBe(concurrent);
    expect(apiClient.fetchPublicLoginInstances).toHaveBeenCalledTimes(1);

    resolveFirst({ default_authenticator_id: 'password', login_instances: [] });
    await first;

    apiClient.fetchPublicLoginInstances.mockResolvedValueOnce({
      default_authenticator_id: 'password',
      login_instances: []
    });
    await fetchLoginInstances('http://example.test');

    expect(apiClient.fetchPublicLoginInstances).toHaveBeenCalledTimes(2);
  });
});
