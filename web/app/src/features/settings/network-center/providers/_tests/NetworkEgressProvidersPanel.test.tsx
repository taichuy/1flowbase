import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { App } from 'antd';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const networkCenterApi = vi.hoisted(() => ({
  settingsNetworkEgressProvidersQueryKey: [
    'settings',
    'network-center',
    'providers'
  ] as const,
  fetchSettingsNetworkEgressProviders: vi.fn(),
  fetchSettingsNetworkEgressProviderTypes: vi.fn(),
  createSettingsNetworkEgressProvider: vi.fn(),
  updateSettingsNetworkEgressProviderLifecycle: vi.fn(),
  syncSettingsNetworkEgressProvider: vi.fn()
}));

vi.mock('../../../api/network-center', () => networkCenterApi);

import { AppI18nProvider } from '../../../../../app/AppI18nProvider';
import { useAuthStore } from '../../../../../state/auth-store';
import { NetworkEgressProvidersPanel } from '../NetworkEgressProvidersPanel';

const provider = {
  id: 'provider-1',
  installation_id: 'installation-1',
  provider_code: 'clash_proxy',
  display_name: 'Mihomo edge',
  description: 'Primary subscription',
  lifecycle: 'active',
  health_status: 'healthy',
  secret_configured: true,
  last_sync_error: 'upstream temporarily unavailable',
  last_synced_at: '2026-08-21T08:00:00Z',
  egresses: [
    {
      provider_egress_key: 'edge:de',
      display_name: 'Germany edge',
      region: 'DE',
      tags: ['eu'],
      availability: 'available',
      synced_at: '2026-08-21T08:00:00Z'
    }
  ]
};

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

  return render(<NetworkEgressProvidersPanel />, { wrapper });
}

describe('NetworkEgressProvidersPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAuthStore.setState({ csrfToken: 'csrf-123' });
    networkCenterApi.fetchSettingsNetworkEgressProviders.mockResolvedValue([
      provider
    ]);
    networkCenterApi.fetchSettingsNetworkEgressProviderTypes.mockResolvedValue([
      {
        installation_id: 'installation-2',
        provider_code: 'clash-proxy',
        display_name: 'Clash / Mihomo Proxy',
        form_schema: {
          schema_version: '1flowbase.plugin.form/v1',
          fields: [
            {
              key: 'subscription_url',
              label: 'Subscription URL',
              type: 'string',
              control: 'url',
              required: true,
              send_mode: 'secret'
            }
          ]
        }
      }
    ]);
    networkCenterApi.createSettingsNetworkEgressProvider.mockResolvedValue(
      provider
    );
    networkCenterApi.updateSettingsNetworkEgressProviderLifecycle.mockResolvedValue(
      provider
    );
    networkCenterApi.syncSettingsNetworkEgressProvider.mockResolvedValue(
      provider
    );
  });

  test('QF-002 adds a proxy type from a selected installed extension without exposing an installation ID or secret reference', async () => {
    renderPanel();

    expect(await screen.findByText('Mihomo edge')).toBeInTheDocument();
    expect(screen.getByText(/^(Healthy|健康)$/)).toBeInTheDocument();
    expect(
      screen.getByText('upstream temporarily unavailable')
    ).toBeInTheDocument();
    expect(
      screen.queryByText('secret://system/network/mihomo')
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /展开行|Expand row/ }));
    expect(await screen.findByText('Germany edge')).toBeInTheDocument();

    fireEvent.click(
      screen.getByRole('button', { name: /Add proxy type|添加代理类型/ })
    );
    expect(screen.queryByLabelText(/ID/)).not.toBeInTheDocument();
    expect(screen.queryByText(/secret:\/\//)).not.toBeInTheDocument();
    expect(
      await screen.findByLabelText('Subscription URL')
    ).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText(/Proxy type name|代理类型名称/), {
      target: { value: 'Backup edge' }
    });
    fireEvent.change(screen.getByLabelText('Subscription URL'), {
      target: { value: 'https://example.invalid/subscription' }
    });
    fireEvent.click(screen.getByRole('button', { name: /Create|创\s*建/ }));

    await waitFor(() =>
      expect(
        networkCenterApi.createSettingsNetworkEgressProvider
      ).toHaveBeenCalledWith(
        {
          installation_id: 'installation-2',
          display_name: 'Backup edge',
          description: '',
          config: { subscription_url: 'https://example.invalid/subscription' }
        },
        'csrf-123'
      )
    );
  });

  test('AC-002 controls lifecycle and sync through the proxy type APIs', async () => {
    renderPanel();

    await screen.findByText('Mihomo edge');
    fireEvent.click(screen.getByRole('button', { name: /Disable|停用/ }));
    await waitFor(() =>
      expect(
        networkCenterApi.updateSettingsNetworkEgressProviderLifecycle
      ).toHaveBeenCalledWith(
        'provider-1',
        { lifecycle: 'disabled' },
        'csrf-123'
      )
    );

    fireEvent.click(
      screen.getByRole('button', { name: /Sync proxies|同步代理/ })
    );
    await waitFor(() =>
      expect(
        networkCenterApi.syncSettingsNetworkEgressProvider
      ).toHaveBeenCalledWith('provider-1', 'csrf-123')
    );
  });
});
