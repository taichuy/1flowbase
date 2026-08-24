import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { App } from 'antd';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const networkCenterApi = vi.hoisted(() => ({
  settingsNetworkEgressPoolsQueryKey: ['settings', 'network-center', 'pools'] as const,
  fetchSettingsNetworkEgressPools: vi.fn(),
  fetchSettingsNetworkEgressProviderTypes: vi.fn(),
  fetchSettingsNetworkEgressProviders: vi.fn(),
  createSettingsNetworkEgressProxy: vi.fn(),
  testSettingsNetworkEgressPoolMember: vi.fn(),
  updateSettingsNetworkEgressPoolMember: vi.fn(),
  deleteSettingsNetworkEgressPoolMember: vi.fn()
}));

vi.mock('../../../api/network-center', () => networkCenterApi);

import { AppI18nProvider } from '../../../../../app/AppI18nProvider';
import { useAuthStore } from '../../../../../state/auth-store';
import { NetworkEgressPoolsPanel } from '../NetworkEgressPoolsPanel';

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
  return render(<NetworkEgressPoolsPanel />, { wrapper });
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => {
    resolve = next;
  });
  return { promise, resolve };
}

describe('NetworkEgressPoolsPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAuthStore.setState({ csrfToken: 'csrf-123' });
    networkCenterApi.fetchSettingsNetworkEgressPools.mockResolvedValue([
      {
        id: 'global-pool',
        display_name: 'Global proxy pool',
        selection_strategy: 'healthy_first',
        members: []
      }
    ]);
    networkCenterApi.fetchSettingsNetworkEgressProviders.mockResolvedValue([]);
    networkCenterApi.fetchSettingsNetworkEgressProviderTypes.mockResolvedValue([
      {
        installation_id: null,
        provider_code: 'builtin_static_http',
        display_name: 'HTTP proxy',
        form_schema: {
          schema_version: '1flowbase.plugin.form/v1',
          fields: [
            {
              key: 'host',
              label: 'Hostname or IP',
              type: 'string',
              required: true
            },
            { key: 'port', label: 'Port', type: 'string', required: true },
            { key: 'username', label: 'Username', type: 'string' },
            { key: 'password', label: 'Password', type: 'string' }
          ]
        }
      },
      {
        installation_id: 'clash-installation',
        provider_code: 'clash-proxy',
        display_name: 'Clash / Mihomo Proxy',
        form_schema: {
          schema_version: '1flowbase.plugin.form/v1',
          fields: [
            {
              key: 'subscription_url',
              label: 'Subscription URL',
              type: 'string',
              required: true
            }
          ]
        }
      }
    ]);
  });

  test('AC-GP01 presents proxy creation in the shared fixed-height modal shell', async () => {
    renderPanel();

    fireEvent.click(await screen.findByRole('button', { name: /添加代理|Add proxy/ }));

    expect(screen.getByTestId('fixed-height-modal-scroll-body')).toBeInTheDocument();
  });

  test('AC-OP02 uses the shared data-table layout and field configuration', async () => {
    renderPanel();

    await screen.findByTestId('network-center-pools-shell');

    expect(document.querySelector('.data-table-layout')).toBeInTheDocument();
    expect(document.querySelector('.data-table')).toBeInTheDocument();
    expect(document.querySelector('.data-table__column-selector')).toBeInTheDocument();
  });

  test('AC-GP01 creates a manual proxy from the global pool without creating a pool', async () => {
    networkCenterApi.createSettingsNetworkEgressProxy.mockResolvedValue({
      id: 'provider-1'
    });
    renderPanel();
    fireEvent.click(await screen.findByRole('button', { name: /添加代理|Add proxy/ }));
    const createModal = screen.getByTestId('fixed-height-modal-scroll-body');
    fireEvent.mouseDown(within(createModal).getByLabelText(/代理类型|Proxy type/));
    fireEvent.click(await screen.findByText('HTTP proxy'));
    fireEvent.change(within(createModal).getByLabelText(/名称|Name/), {
      target: { value: 'US proxy' }
    });
    fireEvent.change(await within(createModal).findByLabelText('Hostname or IP'), {
      target: { value: '198.65.36.212' }
    });
    fireEvent.change(within(createModal).getByLabelText('Port'), {
      target: { value: '37867' }
    });
    fireEvent.click(screen.getByRole('button', { name: /保\s*存|Save/ }));
    await waitFor(() =>
      expect(networkCenterApi.createSettingsNetworkEgressProxy).toHaveBeenCalledWith(
        {
          provider_code: 'builtin_static_http',
          display_name: 'US proxy',
          description: '',
          config: { host: '198.65.36.212', port: '37867' }
        },
        'csrf-123'
      )
    );
    expect(networkCenterApi.testSettingsNetworkEgressPoolMember).not.toHaveBeenCalled();
    expect(screen.queryByText(/创建代理池|Create proxy pool/)).not.toBeInTheDocument();
  });

  test('AC-GP02 submits an extension parser form directly from proxy creation', async () => {
    networkCenterApi.createSettingsNetworkEgressProxy.mockResolvedValue({
      id: 'provider-2'
    });
    renderPanel();
    fireEvent.click(await screen.findByRole('button', { name: /添加代理|Add proxy/ }));
    const createModal = screen.getByTestId('fixed-height-modal-scroll-body');
    fireEvent.mouseDown(within(createModal).getByLabelText(/代理类型|Proxy type/));
    fireEvent.click(await screen.findByText('Clash / Mihomo Proxy'));
    fireEvent.change(within(createModal).getByLabelText(/名称|Name/), {
      target: { value: 'Subscription' }
    });
    fireEvent.change(await within(createModal).findByLabelText('Subscription URL'), {
      target: { value: 'https://example.com/subscription' }
    });
    fireEvent.click(screen.getByRole('button', { name: /保\s*存|Save/ }));
    await waitFor(() =>
      expect(networkCenterApi.createSettingsNetworkEgressProxy).toHaveBeenCalledWith(
        expect.objectContaining({
          provider_code: 'clash-proxy',
          config: { subscription_url: 'https://example.com/subscription' }
        }),
        'csrf-123'
      )
    );
  });

  test('AC-OP02 renders safe operating fields and tests one proxy through the backend', async () => {
    networkCenterApi.fetchSettingsNetworkEgressPools.mockResolvedValue([
      {
        id: 'global-pool',
        display_name: 'Global proxy pool',
        selection_strategy: 'healthy_first',
        members: [
          {
            id: 'member-1',
            provider_id: 'provider-1',
            provider_egress_key: 'static-http',
            provider_code: 'builtin_static_http',
            display_name: 'US proxy',
            address_summary: '198.65.36.212:37867',
            region: 'United States',
            enabled: true,
            sequence: 0,
            health: 'healthy',
            probe_status: 'succeeded',
            probe_latency_ms: 32,
            probe_exit_ip: '198.65.36.212',
            probe_error_code: null,
            last_probed_at: '2026-08-22T03:00:00Z'
          }
        ]
      }
    ]);
    networkCenterApi.testSettingsNetworkEgressPoolMember.mockResolvedValue({
      probe_status: 'succeeded'
    });
    renderPanel();

    expect(await screen.findByRole('columnheader', { name: /名称|Name/ })).toBeInTheDocument();
    expect(screen.getByRole('columnheader', { name: /代理类型|Proxy types/ })).toBeInTheDocument();
    expect(screen.getByRole('columnheader', { name: /延迟|Latency/ })).toBeInTheDocument();
    expect(await screen.findByText('HTTP proxy')).toBeInTheDocument();
    expect(screen.queryByText('builtin_static_http')).not.toBeInTheDocument();
    expect(await screen.findByText('198.65.36.212:37867')).toBeInTheDocument();
    const address = screen.queryByText('198.65.36.212:37867');
    expect(address).toBeInTheDocument();
    expect(address?.closest('code')).not.toBeInTheDocument();
    expect(screen.getByText('32ms')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /测试|Test/ }));

    await waitFor(() => expect(networkCenterApi.testSettingsNetworkEgressPoolMember).toHaveBeenCalledWith('global-pool', 'member-1', 'csrf-123'));
  });

  test('AC-OP02 keeps testing feedback independent for each proxy member', async () => {
    const pendingFirstProbe = deferred<{ probe_status: string }>();
    const pendingSecondProbe = deferred<{ probe_status: string }>();
    networkCenterApi.fetchSettingsNetworkEgressPools.mockResolvedValue([
      {
        id: 'global-pool',
        display_name: 'Global proxy pool',
        selection_strategy: 'healthy_first',
        members: [
          {
            id: 'member-1',
            provider_id: 'provider-1',
            provider_egress_key: 'static-http',
            provider_code: 'builtin_static_http',
            display_name: 'US proxy',
            address_summary: '198.65.36.212:37867',
            region: null,
            enabled: true,
            sequence: 0,
            health: 'healthy',
            probe_status: 'not_tested',
            probe_http_status: 'not_tested',
            probe_https_status: 'not_tested',
            probe_latency_ms: 0,
            probe_exit_ip: null,
            probe_exit_region: null,
            probe_error_code: null,
            last_probed_at: null
          },
          {
            id: 'member-2',
            provider_id: 'provider-2',
            provider_egress_key: 'static-http',
            provider_code: 'builtin_static_http',
            display_name: 'EU proxy',
            address_summary: '203.0.113.2:3128',
            region: null,
            enabled: true,
            sequence: 1,
            health: 'healthy',
            probe_status: 'not_tested',
            probe_http_status: 'not_tested',
            probe_https_status: 'not_tested',
            probe_latency_ms: 0,
            probe_exit_ip: null,
            probe_exit_region: null,
            probe_error_code: null,
            last_probed_at: null
          }
        ]
      }
    ]);
    networkCenterApi.testSettingsNetworkEgressPoolMember.mockImplementation((_, memberId) => (memberId === 'member-1' ? pendingFirstProbe.promise : pendingSecondProbe.promise));
    renderPanel();

    const testButtons = await screen.findAllByRole('button', {
      name: /测试|Test/
    });
    fireEvent.click(testButtons[0]);

    await waitFor(() => expect(networkCenterApi.testSettingsNetworkEgressPoolMember).toHaveBeenCalledWith('global-pool', 'member-1', 'csrf-123'));
    expect(testButtons[0]).toHaveClass('ant-btn-loading');
    expect(testButtons[1]).not.toHaveClass('ant-btn-loading');
    expect(testButtons[1]).toBeEnabled();

    fireEvent.click(testButtons[1]);
    await waitFor(() => expect(networkCenterApi.testSettingsNetworkEgressPoolMember).toHaveBeenCalledWith('global-pool', 'member-2', 'csrf-123'));
    expect(testButtons[0]).toHaveClass('ant-btn-loading');
    expect(testButtons[1]).toHaveClass('ant-btn-loading');

    pendingFirstProbe.resolve({ probe_status: 'succeeded' });
    pendingSecondProbe.resolve({ probe_status: 'succeeded' });
  });

  test('AC-OP05 shows a persisted latency column and defaults an untested proxy to 0ms', async () => {
    networkCenterApi.fetchSettingsNetworkEgressPools.mockResolvedValue([
      {
        id: 'global-pool',
        display_name: 'Global proxy pool',
        selection_strategy: 'healthy_first',
        members: [
          {
            id: 'member-1',
            provider_id: 'provider-1',
            provider_egress_key: 'static-http',
            provider_code: 'builtin_static_http',
            display_name: 'Untested proxy',
            address_summary: '198.65.36.212:37867',
            region: null,
            enabled: true,
            sequence: 0,
            health: 'healthy',
            probe_status: 'not_tested',
            probe_http_status: 'not_tested',
            probe_https_status: 'not_tested',
            probe_latency_ms: 0,
            probe_exit_ip: null,
            probe_exit_region: null,
            probe_error_code: null,
            last_probed_at: null
          }
        ]
      }
    ]);
    renderPanel();

    expect(await screen.findByRole('columnheader', { name: /延迟|Latency/ })).toBeInTheDocument();
    expect(await screen.findByText('Untested proxy')).toBeInTheDocument();
    expect(screen.getByText('0ms')).toBeInTheDocument();
  });

  test('AC-OP03 provides test, edit, and delete actions, and edits only the selected proxy member', async () => {
    networkCenterApi.fetchSettingsNetworkEgressPools.mockResolvedValue([
      {
        id: 'global-pool',
        display_name: 'Global proxy pool',
        selection_strategy: 'healthy_first',
        members: [
          {
            id: 'member-1',
            provider_id: 'provider-1',
            provider_egress_key: 'static-http',
            provider_code: 'builtin_static_http',
            display_name: 'US proxy',
            description: 'Initial proxy',
            address_summary: '198.65.36.212:37867',
            region: null,
            enabled: true,
            sequence: 0,
            health: 'healthy',
            probe_status: 'not_tested',
            probe_latency_ms: 0,
            probe_exit_ip: null,
            probe_error_code: null,
            last_probed_at: null
          }
        ]
      }
    ]);
    networkCenterApi.updateSettingsNetworkEgressPoolMember.mockResolvedValue({
      id: 'member-1'
    });
    renderPanel();

    expect(await screen.findByRole('button', { name: '测试' })).toBeInTheDocument();
    expect(screen.getByText('HTTP 未测试')).toBeInTheDocument();
    expect(screen.getByText('HTTPS 未测试')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '测试连接' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '编辑' }));
    fireEvent.change(screen.getByLabelText(/成员顺序|Member sequence/), {
      target: { value: '2' }
    });
    fireEvent.click(screen.getByRole('button', { name: /确\s*定|OK|Confirm/ }));

    await waitFor(() => expect(networkCenterApi.updateSettingsNetworkEgressPoolMember).toHaveBeenCalledWith('global-pool', 'member-1', { enabled: true, sequence: 2 }, 'csrf-123'));
    expect(screen.getByRole('button', { name: /删除|Delete/ })).toBeInTheDocument();
  });

  test('AC-OP04 exposes a successful HTTP egress and failed HTTPS CONNECT separately', async () => {
    networkCenterApi.fetchSettingsNetworkEgressPools.mockResolvedValue([
      {
        id: 'global-pool',
        display_name: 'Global proxy pool',
        selection_strategy: 'healthy_first',
        members: [
          {
            id: 'member-1',
            provider_id: 'provider-1',
            provider_egress_key: 'static-http',
            provider_code: 'builtin_static_http',
            display_name: 'US proxy',
            address_summary: '198.65.36.212:37867',
            region: 'California',
            enabled: true,
            sequence: 0,
            health: 'healthy',
            probe_status: 'failed',
            probe_http_status: 'succeeded',
            probe_https_status: 'failed',
            probe_latency_ms: 42,
            probe_exit_ip: '198.65.36.212',
            probe_exit_region: 'California',
            probe_error_code: 'https_connect_failed',
            last_probed_at: '2026-08-22T03:00:00Z'
          }
        ]
      }
    ]);
    renderPanel();

    expect(await screen.findByText('HTTP 可用')).toBeInTheDocument();
    expect(screen.getByText('HTTPS 不可用')).toBeInTheDocument();
    expect(screen.getByText('HTTPS CONNECT 被拒绝')).toBeInTheDocument();
    expect(screen.getByText('California')).toBeInTheDocument();
  });
});
