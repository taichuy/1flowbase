import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
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
  createSettingsNetworkEgressPoolStaticHttpMember: vi.fn(),
  addSettingsNetworkEgressProviderToPool: vi.fn(),
  updateSettingsNetworkEgressPoolMember: vi.fn(),
  deleteSettingsNetworkEgressPoolMember: vi.fn()
}));

vi.mock('../../../api/network-center', () => networkCenterApi);

import { AppI18nProvider } from '../../../../../app/AppI18nProvider';
import { useAuthStore } from '../../../../../state/auth-store';
import { NetworkEgressPoolsPanel } from '../NetworkEgressPoolsPanel';

const providers = [
  {
    id: 'provider-1',
    installation_id: 'installation-1',
    provider_code: 'edge',
    display_name: 'Edge provider',
    description: '',
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
    useAuthStore.setState({ csrfToken: 'csrf-123' });
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

  test('AC-NC16 adds a manually configured HTTP proxy from the target pool', async () => {
    networkCenterApi.createSettingsNetworkEgressPoolStaticHttpMember.mockResolvedValue({
      id: 'member-static'
    });
    renderPanel();

    fireEvent.click(await screen.findByRole('button', { name: /展开行|Expand row/ }));
    fireEvent.click(await screen.findByRole('button', { name: /添加出口|Add egress/ }));
    fireEvent.click(screen.getByLabelText(/手动 HTTP 代理|Manual HTTP proxy/));
    fireEvent.change(screen.getByLabelText(/名称|Name/), {
      target: { value: 'US proxy' }
    });
    fireEvent.change(screen.getByLabelText(/主机|Host/), {
      target: { value: '198.65.36.212' }
    });
    fireEvent.change(screen.getByLabelText(/端口|Port/), {
      target: { value: '37867' }
    });
    fireEvent.change(screen.getByLabelText(/用户名|Username/), {
      target: { value: 'suY8TMiTjpEb' }
    });
    fireEvent.change(screen.getByLabelText(/密码|Password/), {
      target: { value: '4BJiWEi3kHXY' }
    });
    fireEvent.click(screen.getByRole('button', { name: /保\s*存|Save/ }));

    await waitFor(() =>
      expect(
        networkCenterApi.createSettingsNetworkEgressPoolStaticHttpMember
      ).toHaveBeenCalledWith(
        'pool-1',
        expect.objectContaining({
          display_name: 'US proxy',
          host: '198.65.36.212',
          port: 37867,
          username: 'suY8TMiTjpEb',
          password: '4BJiWEi3kHXY'
        }),
        expect.any(String)
      )
    );
  });

  test('AC-NC17 adds every current egress from the selected extension instance to the target pool', async () => {
    networkCenterApi.addSettingsNetworkEgressProviderToPool.mockResolvedValue([]);
    renderPanel();

    fireEvent.click(await screen.findByRole('button', { name: /展开行|Expand row/ }));
    fireEvent.click(await screen.findByRole('button', { name: /添加出口|Add egress/ }));
    fireEvent.click(screen.getByLabelText(/扩展供应方|Extension provider/));
    fireEvent.mouseDown(
      await screen.findByLabelText(/供应方实例|Provider instance/)
    );
    fireEvent.click(await screen.findByText('Edge provider · 1'));
    fireEvent.click(screen.getByRole('button', { name: /保\s*存|Save/ }));

    await waitFor(() =>
      expect(networkCenterApi.addSettingsNetworkEgressProviderToPool).toHaveBeenCalledWith(
        'pool-1',
        { provider_id: 'provider-1', enabled: true, sequence: 0 },
        'csrf-123'
      )
    );
  });
});
