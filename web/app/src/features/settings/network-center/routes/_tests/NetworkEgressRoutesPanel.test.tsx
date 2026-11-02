import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { App } from 'antd';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const networkCenterApi = vi.hoisted(() => ({
  settingsNetworkEgressPoolsQueryKey: [
    'settings',
    'network-center',
    'pools'
  ] as const,
  settingsNetworkEgressRoutesQueryKey: [
    'settings',
    'network-center',
    'routes'
  ] as const,
  fetchSettingsNetworkEgressPools: vi.fn(),
  fetchSettingsNetworkEgressRoutes: vi.fn(),
  createSettingsNetworkEgressRoute: vi.fn(),
  updateSettingsNetworkEgressRoute: vi.fn(),
  deleteSettingsNetworkEgressRoute: vi.fn()
}));

const modelProviderApi = vi.hoisted(() => ({
  settingsModelProviderInstancesQueryKey: [
    'settings',
    'model-providers'
  ] as const,
  fetchSettingsModelProviderInstances: vi.fn()
}));

vi.mock('../../../api/network-center', () => networkCenterApi);
vi.mock('../../../api/model-providers', () => modelProviderApi);

import { AppI18nProvider } from '../../../../../app/AppI18nProvider';
import { useAuthStore } from '../../../../../state/auth-store';
import { NetworkEgressRoutesPanel } from '../NetworkEgressRoutesPanel';

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
  return render(<NetworkEgressRoutesPanel />, { wrapper });
}

describe('NetworkEgressRoutesPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAuthStore.setState({ csrfToken: 'csrf-123', actor: { id: 'root', account: 'root', effective_display_role: 'root', current_workspace_id: 'workspace-1' }, me: null });
    networkCenterApi.fetchSettingsNetworkEgressRoutes.mockResolvedValue([]);
    networkCenterApi.fetchSettingsNetworkEgressPools.mockResolvedValue([
      {
        id: 'global-pool',
        display_name: 'Global proxy pool',
        selection_strategy: 'healthy_first',
        members: [
          {
            id: 'proxy-us',
            provider_id: 'provider-us',
            provider_egress_key: 'us',
            provider_code: 'builtin_static_http',
            display_name: 'US proxy',
            address_summary: '198.51.100.10:3128',
            region: 'US',
            enabled: true,
            sequence: 10,
            health: 'healthy'
          },
          {
            id: 'proxy-eu',
            provider_id: 'provider-eu',
            provider_egress_key: 'eu',
            provider_code: 'builtin_static_http',
            display_name: 'EU proxy',
            address_summary: '203.0.113.20:3128',
            region: 'EU',
            enabled: true,
            sequence: 20,
            health: 'healthy'
          }
        ]
      }
    ]);
    modelProviderApi.fetchSettingsModelProviderInstances.mockResolvedValue([]);
    networkCenterApi.createSettingsNetworkEgressRoute.mockResolvedValue({
      id: 'route-1'
    });
  });

  test('AC-001 creates a routing rule with an explicit ordered proxy mapping', async () => {
    renderPanel();

    fireEvent.click(
      await screen.findByRole('button', {
        name: /创建路由规则|Create routing rule/
      })
    );
    const dialog = await screen.findByRole('dialog');
    const proxyMapping =
      within(dialog).getByLabelText(/代理映射|Proxy mapping/);
    fireEvent.mouseDown(proxyMapping);
    fireEvent.click(await screen.findByText('US proxy'));
    await within(dialog).findByRole('button', {
      name: /下移.*US proxy|Move down.*US proxy/
    });
    fireEvent.click(await screen.findByText('EU proxy'));
    fireEvent.click(
      await within(dialog).findByRole('button', {
        name: /上移.*EU proxy|Move up.*EU proxy/
      })
    );
    fireEvent.click(within(dialog).getByRole('button', { name: /确\s*定|OK/ }));

    await waitFor(() =>
      expect(
        networkCenterApi.createSettingsNetworkEgressRoute
      ).toHaveBeenCalledWith(
        {
          consumer_kind: 'github',
          consumer_reference: null,
          pool_member_ids: ['proxy-eu', 'proxy-us'],
          enabled: true
        },
        'csrf-123'
      )
    );
  });

  test('creates and reloads an Agent Logs exchange route with its exact consumer contract', async () => {
    networkCenterApi.createSettingsNetworkEgressRoute.mockImplementationOnce(
      async (input) => {
        const saved = {
          id: 'route-agent-logs',
          ...input,
          failure_policy: 'fail_closed'
        };
        networkCenterApi.fetchSettingsNetworkEgressRoutes.mockResolvedValue([
          saved
        ]);
        return saved;
      }
    );
    renderPanel();
    fireEvent.click(
      await screen.findByRole('button', {
        name: /创建路由规则|Create routing rule/
      })
    );
    const dialog = await screen.findByRole('dialog');
    fireEvent.mouseDown(within(dialog).getByLabelText(/消费方|Consumer/));
    fireEvent.click(
      await screen.findByText(/^(Agent Logs 日志交换|Agent Logs exchange)$/, {
        selector: '.ant-select-item-option-content'
      })
    );
    fireEvent.mouseDown(
      within(dialog).getByLabelText(/代理映射|Proxy mapping/)
    );
    fireEvent.click(await screen.findByText('US proxy'));
    fireEvent.click(within(dialog).getByRole('button', { name: /确\s*定|OK/ }));
    await waitFor(() =>
      expect(
        networkCenterApi.createSettingsNetworkEgressRoute
      ).toHaveBeenCalledWith(
        {
          consumer_kind: 'agent_logs_exchange',
          consumer_reference: null,
          pool_member_ids: ['proxy-us'],
          enabled: true
        },
        'csrf-123'
      )
    );
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
    const persistedConsumer = await screen.findByRole('cell', {
      name: /^(Agent Logs 日志交换|Agent Logs exchange)$/
    });
    const row = persistedConsumer.closest('tr');
    expect(row).not.toBeNull();
    expect(within(row!).getByText('US proxy')).toBeInTheDocument();
    fireEvent.click(within(row!).getByRole('button', { name: /编辑|Edit/ }));
    const edit = await screen.findByRole('dialog');
    expect(within(edit).getByLabelText(/消费方|Consumer/)).toBeDisabled();
    expect(
      within(edit).getByText(/^(Agent Logs 日志交换|Agent Logs exchange)$/)
    ).toBeInTheDocument();
  });
  test.each([false, true])('route delete grant %s preserves readable mappings without route create or edit', async (deleteGranted) => {
    setOperations(deleteGranted ? ['network_egress_routes.list', 'network_egress_routes.delete'] : ['network_egress_routes.list']);
    networkCenterApi.fetchSettingsNetworkEgressRoutes.mockResolvedValue([{ id: 'route-1', consumer_kind: 'github', consumer_reference: null, pool_member_ids: ['proxy-us'], enabled: true }]);
    renderPanel();
    const row = await screen.findByRole('row', { name: /github.*US proxy/ });
    expect(screen.getByRole('button', { name: /创建路由规则|Create routing rule/ })).toBeDisabled();
    expect(within(row).getByRole('button', { name: /删除|Delete/ }).hasAttribute('disabled')).toBe(!deleteGranted);
    fireEvent.click(within(row).getByRole('button', { name: /编辑|Edit/ }));
    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByLabelText(/代理映射|Proxy mapping/)).toBeDisabled();
    expect(within(dialog).getByRole('button', { name: /确\s*定|OK/ })).toBeDisabled();
    expect(networkCenterApi.updateSettingsNetworkEgressRoute).not.toHaveBeenCalled();
    expect(networkCenterApi.createSettingsNetworkEgressRoute).not.toHaveBeenCalled();
  });

});

function setOperations(permissions: string[]) {
  useAuthStore.setState({
    actor: { id: 'member', account: 'member', effective_display_role: 'member', current_workspace_id: 'workspace-1' },
    me: { id: 'member', account: 'member', email: '', phone: null, nickname: '', name: '', avatar_url: null, introduction: '', effective_display_role: 'member', permissions }
  });
}
