import { beforeEach, describe, expect, test, vi } from 'vitest';

const apiClient = vi.hoisted(() => ({
  listConsoleNetworkEgressPools: vi.fn(),
  createConsoleNetworkEgressPool: vi.fn(),
  updateConsoleNetworkEgressPool: vi.fn(),
  deleteConsoleNetworkEgressPool: vi.fn(),
  createConsoleNetworkEgressPoolMember: vi.fn(),
  updateConsoleNetworkEgressPoolMember: vi.fn(),
  deleteConsoleNetworkEgressPoolMember: vi.fn(),
  listConsoleNetworkEgressRoutes: vi.fn(),
  createConsoleNetworkEgressRoute: vi.fn(),
  updateConsoleNetworkEgressRoute: vi.fn(),
  deleteConsoleNetworkEgressRoute: vi.fn()
}));

vi.mock('@1flowbase/api-client', () => apiClient);

import {
  createSettingsNetworkEgressPool,
  createSettingsNetworkEgressPoolMember,
  deleteSettingsNetworkEgressPool,
  deleteSettingsNetworkEgressPoolMember,
  fetchSettingsNetworkEgressPools,
  fetchSettingsNetworkEgressRoutes,
  settingsNetworkEgressRoutesQueryKey,
  settingsNetworkEgressPoolsQueryKey,
  updateSettingsNetworkEgressPool,
  updateSettingsNetworkEgressPoolMember,
  createSettingsNetworkEgressRoute,
  updateSettingsNetworkEgressRoute,
  deleteSettingsNetworkEgressRoute
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

  test('AC-NC08 forwards only backend pool and member DTO fields', () => {
    const pool = { display_name: 'European exits' };
    const member = {
      provider_id: 'provider-1',
      provider_egress_key: 'egress:eu-west',
      enabled: true,
      sequence: 10
    };

    createSettingsNetworkEgressPool(pool, 'csrf-123');
    updateSettingsNetworkEgressPool('pool-1', pool, 'csrf-123');
    deleteSettingsNetworkEgressPool('pool-1', 'csrf-123');
    createSettingsNetworkEgressPoolMember('pool-1', member, 'csrf-123');
    updateSettingsNetworkEgressPoolMember(
      'pool-1',
      'member-1',
      { enabled: false, sequence: 20 },
      'csrf-123'
    );
    deleteSettingsNetworkEgressPoolMember('pool-1', 'member-1', 'csrf-123');

    expect(apiClient.createConsoleNetworkEgressPool).toHaveBeenCalledWith(
      pool,
      'csrf-123'
    );
    expect(apiClient.updateConsoleNetworkEgressPool).toHaveBeenCalledWith(
      'pool-1',
      pool,
      'csrf-123'
    );
    expect(apiClient.deleteConsoleNetworkEgressPool).toHaveBeenCalledWith(
      'pool-1',
      'csrf-123'
    );
    expect(apiClient.createConsoleNetworkEgressPoolMember).toHaveBeenCalledWith(
      'pool-1',
      member,
      'csrf-123'
    );
    expect(apiClient.updateConsoleNetworkEgressPoolMember).toHaveBeenCalledWith(
      'pool-1',
      'member-1',
      { enabled: false, sequence: 20 },
      'csrf-123'
    );
    expect(apiClient.deleteConsoleNetworkEgressPoolMember).toHaveBeenCalledWith(
      'pool-1',
      'member-1',
      'csrf-123'
    );
  });
});

describe('settings network egress routes API', () => {
  test('AC-NC13 forwards the typed route DTO without frontend aliases', () => {
    const route = {
      consumer_kind: 'http_node',
      consumer_reference: null,
      pool_id: 'pool-1',
      enabled: true
    };
    expect(settingsNetworkEgressRoutesQueryKey).toEqual([
      'settings',
      'network-center',
      'routes'
    ]);
    fetchSettingsNetworkEgressRoutes();
    createSettingsNetworkEgressRoute(route, 'csrf-123');
    updateSettingsNetworkEgressRoute(
      'route-1',
      { pool_id: 'pool-2', enabled: false },
      'csrf-123'
    );
    deleteSettingsNetworkEgressRoute('route-1', 'csrf-123');
    expect(apiClient.listConsoleNetworkEgressRoutes).toHaveBeenCalledWith();
    expect(apiClient.createConsoleNetworkEgressRoute).toHaveBeenCalledWith(
      route,
      'csrf-123'
    );
  });
});
