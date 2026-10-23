import { beforeEach, describe, expect, test, vi } from 'vitest';

const apiClient = vi.hoisted(() => ({
  listConsoleNetworkEgressProviders: vi.fn(),
  createConsoleNetworkEgressProvider: vi.fn(),
  updateConsoleNetworkEgressProviderLifecycle: vi.fn(),
  syncConsoleNetworkEgressProvider: vi.fn(),
  listConsoleNetworkEgressPools: vi.fn(),
  createConsoleNetworkEgressPool: vi.fn(),
  updateConsoleNetworkEgressPool: vi.fn(),
  deleteConsoleNetworkEgressPool: vi.fn(),
  createConsoleNetworkEgressPoolMember: vi.fn(),
  updateConsoleNetworkEgressPoolMember: vi.fn(),
  deleteConsoleNetworkEgressPoolMember: vi.fn(),
  deleteConsoleNetworkEgressPoolMembers: vi.fn(),
  listConsoleNetworkEgressRoutes: vi.fn(),
  createConsoleNetworkEgressRoute: vi.fn(),
  updateConsoleNetworkEgressRoute: vi.fn(),
  deleteConsoleNetworkEgressRoute: vi.fn()
}));

vi.mock('@1flowbase/api-client', () => apiClient);

import {
  fetchSettingsNetworkEgressPools,
  fetchSettingsNetworkEgressRoutes,
  settingsNetworkEgressRoutesQueryKey,
  settingsNetworkEgressPoolsQueryKey
} from '../network-center';

describe('settings network egress pools API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  test('AC-NC08 owns a single pool projection query key', () => {
    expect(settingsNetworkEgressPoolsQueryKey).toEqual([
      'settings',
      'network-center',
      'pools'
    ]);

    fetchSettingsNetworkEgressPools();

    expect(apiClient.listConsoleNetworkEgressPools).toHaveBeenCalledWith();
  });
});

describe('settings network egress routes API', () => {
  test('AC-NC13 owns the routes query key and list callback', () => {
    expect(settingsNetworkEgressRoutesQueryKey).toEqual([
      'settings',
      'network-center',
      'routes'
    ]);
    fetchSettingsNetworkEgressRoutes();

    expect(apiClient.listConsoleNetworkEgressRoutes).toHaveBeenCalledWith();
  });
});
