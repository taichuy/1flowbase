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

  test('AC-002 registers a provider with only the installation, display name, and opaque secret reference', async () => {
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
      screen.getByRole('button', { name: /Register provider|注册提供方/ })
    );
    fireEvent.change(
      screen.getByLabelText(/Provider installation|提供方安装/),
      { target: { value: 'installation-2' } }
    );
    fireEvent.change(screen.getByLabelText(/Provider name|提供方名称/), {
      target: { value: 'Backup edge' }
    });
    fireEvent.change(screen.getByLabelText(/Secret reference|密钥引用/), {
      target: { value: 'secret://system/network/mihomo' }
    });
    fireEvent.click(screen.getByRole('button', { name: /Create|创\s*建/ }));

    await waitFor(() =>
      expect(
        networkCenterApi.createSettingsNetworkEgressProvider
      ).toHaveBeenCalledWith(
        {
          installation_id: 'installation-2',
          display_name: 'Backup edge',
          secret_ref: 'secret://system/network/mihomo'
        },
        'csrf-123'
      )
    );
  });

  test('AC-002 controls lifecycle and sync through the Provider APIs', async () => {
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
      screen.getByRole('button', { name: /Sync exits|同步出口/ })
    );
    await waitFor(() =>
      expect(
        networkCenterApi.syncSettingsNetworkEgressProvider
      ).toHaveBeenCalledWith('provider-1', 'csrf-123')
    );
  });
});
