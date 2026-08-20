import { beforeEach, describe, expect, test, vi } from 'vitest';

const apiClient = vi.hoisted(() => ({
  listConsoleNetworkEgressPools: vi.fn(),
  createConsoleNetworkEgressPool: vi.fn(),
  updateConsoleNetworkEgressPool: vi.fn(),
  deleteConsoleNetworkEgressPool: vi.fn(),
  createConsoleNetworkEgressPoolMember: vi.fn(),
  updateConsoleNetworkEgressPoolMember: vi.fn(),
  deleteConsoleNetworkEgressPoolMember: vi.fn()
}));

vi.mock('@1flowbase/api-client', () => apiClient);

import {
  createSettingsNetworkEgressPool,
  createSettingsNetworkEgressPoolMember,
  deleteSettingsNetworkEgressPool,
  deleteSettingsNetworkEgressPoolMember,
  fetchSettingsNetworkEgressPools,
  settingsNetworkEgressPoolsQueryKey,
  updateSettingsNetworkEgressPool,
  updateSettingsNetworkEgressPoolMember
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
