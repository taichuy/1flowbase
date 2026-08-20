import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen } from '@testing-library/react';
import { App } from 'antd';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const networkCenterApi = vi.hoisted(() => ({
  settingsNetworkEgressPoolsQueryKey: [
    'settings',
    'network-center',
    'pools'
  ] as const,
  fetchSettingsNetworkEgressPools: vi.fn(),
  createSettingsNetworkEgressPool: vi.fn(),
  updateSettingsNetworkEgressPool: vi.fn(),
  deleteSettingsNetworkEgressPool: vi.fn(),
  createSettingsNetworkEgressPoolMember: vi.fn(),
  updateSettingsNetworkEgressPoolMember: vi.fn(),
  deleteSettingsNetworkEgressPoolMember: vi.fn()
}));

vi.mock('../../../api/network-center', () => networkCenterApi);

import { AppI18nProvider } from '../../../../../app/AppI18nProvider';
import { NetworkEgressPoolsPanel } from '../NetworkEgressPoolsPanel';

const providers = [
  {
    id: 'provider-1',
    installation_id: 'installation-1',
    provider_code: 'edge',
    display_name: 'Edge provider',
    lifecycle: 'active',
    health_status: 'healthy',
    secret_configured: true,
    last_sync_error: null,
    last_synced_at: null,
    egresses: [
      {
        provider_egress_key: 'egress:eu-west',
        display_name: 'EU West',
        region: 'eu-west',
        tags: [],
        availability: 'available',
        synced_at: '2026-08-20T00:00:00Z'
      }
    ]
  }
];

function renderPanel() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } }
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <AppI18nProvider>
      <App>
        <QueryClientProvider client={client}>{children}</QueryClientProvider>
      </App>
    </AppI18nProvider>
  );

  return render(<NetworkEgressPoolsPanel providers={providers} />, { wrapper });
}

describe('NetworkEgressPoolsPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    networkCenterApi.fetchSettingsNetworkEgressPools.mockResolvedValue([
      {
        id: 'pool-1',
        display_name: 'European exits',
        selection_strategy: 'healthy_first',
        members: [
          {
            id: 'member-1',
            provider_id: 'provider-1',
            provider_egress_key: 'egress:eu-west',
            enabled: true,
            sequence: 10,
            health: 'invalid'
          }
        ]
      }
    ]);
  });

  test('AC-NC08 presents the backend member reference and its invalid health without lease fields', async () => {
    renderPanel();

    expect(await screen.findByText('European exits')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /展开行|Expand row/ }));
    expect(
      await screen.findByText('provider-1 · egress:eu-west')
    ).toBeInTheDocument();
    expect(screen.getByText(/Invalid|无效/)).toBeInTheDocument();
    expect(screen.queryByText(/lease/i)).not.toBeInTheDocument();
  });
});
