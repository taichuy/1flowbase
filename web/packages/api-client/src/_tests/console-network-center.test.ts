import { describe, expect, test, vi } from 'vitest';

import * as transport from '../transport';
import {
  createConsoleNetworkEgressProvider,
  createConsoleNetworkEgressPool,
  createConsoleNetworkEgressPoolMember,
  deleteConsoleNetworkEgressPool,
  deleteConsoleNetworkEgressPoolMember,
  listConsoleNetworkEgressPools,
  syncConsoleNetworkEgressProvider,
  updateConsoleNetworkEgressPool,
  updateConsoleNetworkEgressPoolMember,
  updateConsoleNetworkEgressProviderLifecycle
} from '../console/network-center';

describe('console network egress providers client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('AC-002 uses the Provider lifecycle routes and keeps secret material opaque', async () => {
    await expect(
      createConsoleNetworkEgressProvider(
        {
          installation_id: 'installation-1',
          display_name: 'Mihomo edge',
          secret_ref: 'secret://system/network/mihomo'
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/network-center/providers',
      method: 'POST',
      csrfToken: 'csrf-123',
      body: {
        installation_id: 'installation-1',
        display_name: 'Mihomo edge',
        secret_ref: 'secret://system/network/mihomo'
      }
    });
    await expect(
      updateConsoleNetworkEgressProviderLifecycle(
        'provider-1',
        { lifecycle: 'active' },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/network-center/providers/provider-1',
      method: 'PATCH',
      csrfToken: 'csrf-123',
      body: { lifecycle: 'active' }
    });
    await expect(
      syncConsoleNetworkEgressProvider('provider-1', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/network-center/providers/provider-1/sync',
      method: 'POST',
      csrfToken: 'csrf-123'
    });
  });
});

describe('console network egress pools client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );
  vi.spyOn(transport, 'apiFetchVoid').mockImplementation(async () => undefined);

  test('AC-NC08 reads the pool projection without deriving member state', async () => {
    await expect(listConsoleNetworkEgressPools()).resolves.toMatchObject({
      path: '/api/console/network-center/pools'
    });
  });

  test('AC-NC08 sends only the stable provider reference when adding a member', async () => {
    await expect(
      createConsoleNetworkEgressPoolMember(
        'pool-1',
        {
          provider_id: 'provider-1',
          provider_egress_key: 'egress:eu-west',
          enabled: true,
          sequence: 10
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/network-center/pools/pool-1/members',
      method: 'POST',
      csrfToken: 'csrf-123',
      body: {
        provider_id: 'provider-1',
        provider_egress_key: 'egress:eu-west',
        enabled: true,
        sequence: 10
      }
    });
  });

  test('AC-NC08 keeps pool and member updates narrow', async () => {
    await expect(
      createConsoleNetworkEgressPool(
        { display_name: 'European exits' },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/network-center/pools',
      method: 'POST',
      csrfToken: 'csrf-123',
      body: { display_name: 'European exits' }
    });
    await expect(
      updateConsoleNetworkEgressPool(
        'pool-1',
        { display_name: 'European exits' },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/network-center/pools/pool-1',
      method: 'PATCH',
      csrfToken: 'csrf-123',
      body: { display_name: 'European exits' }
    });
    await expect(
      updateConsoleNetworkEgressPoolMember(
        'pool-1',
        'member-1',
        { enabled: false, sequence: 20 },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/network-center/pools/pool-1/members/member-1',
      method: 'PATCH',
      csrfToken: 'csrf-123',
      body: { enabled: false, sequence: 20 }
    });
  });

  test('AC-NC08 deletes pools and members through their own routes', async () => {
    await deleteConsoleNetworkEgressPool('pool-1', 'csrf-123');
    expect(transport.apiFetchVoid).toHaveBeenCalledWith({
      path: '/api/console/network-center/pools/pool-1',
      method: 'DELETE',
      csrfToken: 'csrf-123'
    });
    await deleteConsoleNetworkEgressPoolMember(
      'pool-1',
      'member-1',
      'csrf-123'
    );
    expect(transport.apiFetchVoid).toHaveBeenCalledWith({
      path: '/api/console/network-center/pools/pool-1/members/member-1',
      method: 'DELETE',
      csrfToken: 'csrf-123'
    });
  });
});
