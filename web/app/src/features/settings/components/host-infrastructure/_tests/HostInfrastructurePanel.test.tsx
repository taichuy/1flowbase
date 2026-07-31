import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import { resetAuthStore, useAuthStore } from '../../../../../state/auth-store';

const echartsMock = vi.hoisted(() => ({
  chart: {
    dispose: vi.fn(),
    resize: vi.fn(),
    setOption: vi.fn()
  },
  init: vi.fn()
}));

const api = vi.hoisted(() => ({
  settingsHostInfrastructureProvidersQueryKey: [
    'settings',
    'host-infrastructure',
    'providers'
  ],
  settingsHostInfrastructureMemoryOverviewQueryKey: [
    'settings',
    'host-infrastructure',
    'memory'
  ],
  settingsHostInfrastructureMemoryStatsOverviewQueryKey: [
    'settings',
    'host-infrastructure',
    'memory',
    'stats'
  ],
  settingsHostInfrastructureMemoryEntriesQueryKey: vi.fn(
    (
      contractCode: string | null,
      request?: {
        inspection_path?: string[];
        cursor?: string | null;
        limit?: number;
        byte_limit?: number;
      }
    ) => [
      'settings',
      'host-infrastructure',
      'memory',
      'contracts',
      contractCode,
      'entries',
      request?.inspection_path ?? [],
      request?.cursor ?? null,
      request?.limit ?? null,
      request?.byte_limit ?? null
    ]
  ),
  settingsHostInfrastructureMemoryTreeQueryKey: vi.fn(
    (
      contractCode: string | null,
      request?: {
        inspection_path?: string[];
        cursor?: string | null;
        limit?: number;
        byte_limit?: number;
      }
    ) => [
      'settings',
      'host-infrastructure',
      'memory',
      'contracts',
      contractCode,
      'tree',
      request?.inspection_path ?? [],
      request?.cursor ?? null,
      request?.limit ?? null,
      request?.byte_limit ?? null
    ]
  ),
  settingsHostInfrastructureMemoryStatsQueryKey: vi.fn(
    (
      contractCode: string | null,
      request?: {
        inspection_path?: string[];
      }
    ) => [
      'settings',
      'host-infrastructure',
      'memory',
      'contracts',
      contractCode,
      'stats',
      request?.inspection_path ?? []
    ]
  ),
  settingsHostInfrastructureMemorySearchQueryKey: vi.fn(
    (
      contractCode: string | null,
      request?: {
        q?: string;
        inspection_path?: string[];
        cursor?: string | null;
        limit?: number;
        byte_limit?: number;
      }
    ) => [
      'settings',
      'host-infrastructure',
      'memory',
      'contracts',
      contractCode,
      'search',
      request?.q ?? '',
      request?.inspection_path ?? [],
      request?.cursor ?? null,
      request?.limit ?? null,
      request?.byte_limit ?? null
    ]
  ),
  fetchSettingsHostInfrastructureProviders: vi.fn(),
  saveSettingsHostInfrastructureProviderConfig: vi.fn(),
  fetchSettingsHostInfrastructureMemoryOverview: vi.fn(),
  fetchSettingsHostInfrastructureMemoryStatsOverview: vi.fn(),
  fetchSettingsHostInfrastructureMemoryStats: vi.fn(),
  fetchSettingsHostInfrastructureMemoryEntries: vi.fn(),
  fetchSettingsHostInfrastructureMemoryTree: vi.fn(),
  searchSettingsHostInfrastructureMemoryEntries: vi.fn(),
  revealSettingsHostInfrastructureMemoryEntry: vi.fn()
}));

vi.mock('echarts/core', () => ({
  init: echartsMock.init,
  use: vi.fn()
}));
vi.mock('echarts/charts', () => ({
  BarChart: {},
  LineChart: {},
  PieChart: {},
  FunnelChart: {},
  GaugeChart: {},
  RadarChart: {}
}));
vi.mock('echarts/components', () => ({
  GridComponent: {},
  LegendComponent: {},
  RadarComponent: {},
  TooltipComponent: {},
  TitleComponent: {}
}));
vi.mock('echarts/renderers', () => ({
  CanvasRenderer: {}
}));
vi.mock('../../../api/host-infrastructure', () => api);

import { HostInfrastructurePanel } from '../HostInfrastructurePanel';
import { HostInfrastructureMemoryObservationPanel } from '../HostInfrastructureMemoryObservationPanel';

function renderPanel(canManage = true) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });

  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'user-1',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'workspace-1'
    },
    me: null
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <HostInfrastructurePanel canManage={canManage} />
    </QueryClientProvider>
  );
}

function renderMemoryObservationPanel(canManage = true) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });

  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'user-1',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'workspace-1'
    },
    me: null
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <HostInfrastructureMemoryObservationPanel canManage={canManage} />
    </QueryClientProvider>
  );
}

describe('HostInfrastructurePanel', () => {
  afterEach(() => {
    vi.clearAllMocks();
    resetAuthStore();
  });

  beforeEach(() => {
    echartsMock.init.mockReturnValue(echartsMock.chart);
    api.fetchSettingsHostInfrastructureMemoryOverview.mockResolvedValue({
      can_manage: true,
      contracts: []
    });
    api.fetchSettingsHostInfrastructureMemoryStatsOverview.mockResolvedValue({
      inspection_path: [],
      contracts: [],
      entry_count: 0,
      sensitive_entry_count: 0,
      total_value_size_bytes: 0
    });
    api.fetchSettingsHostInfrastructureMemoryStats.mockResolvedValue({
      contract_code: 'session-store',
      label: 'Sessions',
      provider_code: 'local',
      supported: true,
      inspection_path: [],
      entry_count: 0,
      sensitive_entry_count: 0,
      total_value_size_bytes: 0
    });
    api.fetchSettingsHostInfrastructureMemoryEntries.mockResolvedValue({
      contract_code: 'session-store',
      label: 'Sessions',
      provider_code: 'local',
      capabilities: {
        list_entries: true,
        reveal_value: true
      },
      supported: true,
      entries: []
    });
  });

  test('renders installed inactive provider config from manifest schema', async () => {
    api.fetchSettingsHostInfrastructureProviders.mockResolvedValue([
      {
        installation_id: 'installation-1',
        extension_id: 'redis-infra-host',
        provider_code: 'redis',
        display_name: 'Redis',
        description: 'Redis backed host infrastructure.',
        runtime_status: 'inactive',
        desired_state: 'disabled',
        config_ref: 'secret://system/redis-infra-host/config',
        contracts: ['storage-ephemeral', 'cache-store'],
        enabled_contracts: [],
        config_schema: [
          { key: 'host', label: 'Host', type: 'string', required: true },
          { key: 'port', label: 'Port', type: 'number', required: true }
        ],
        config_json: {},
        restart_required: false
      }
    ]);

    renderPanel();

    expect(await screen.findByText('Redis')).toBeInTheDocument();
    expect(screen.getByText('disabled')).toBeInTheDocument();
    expect(screen.getByText('inactive')).toBeInTheDocument();
    expect(screen.getByText('storage-ephemeral')).toBeInTheDocument();
    expect(screen.getByText('cache-store')).toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: '内存观察' })
    ).not.toBeInTheDocument();
  });

  test('renders pending restart state without claiming provider is active', async () => {
    api.fetchSettingsHostInfrastructureProviders.mockResolvedValue([
      {
        installation_id: 'installation-1',
        extension_id: 'redis-infra-host',
        provider_code: 'redis',
        display_name: 'Redis',
        description: null,
        runtime_status: 'inactive',
        desired_state: 'pending_restart',
        config_ref: 'secret://system/redis-infra-host/config',
        contracts: ['storage-ephemeral'],
        enabled_contracts: ['storage-ephemeral'],
        config_schema: [],
        config_json: {},
        restart_required: true
      }
    ]);

    renderPanel();

    expect(await screen.findByText('Redis')).toBeInTheDocument();
    expect(screen.getByText('pending_restart')).toBeInTheDocument();
    expect(screen.getByText('inactive')).toBeInTheDocument();
    expect(screen.getByText('重启后生效')).toBeInTheDocument();
  });

  test('saves config and contract selection as one pending restart change', async () => {
    api.fetchSettingsHostInfrastructureProviders.mockResolvedValue([
      {
        installation_id: 'installation-1',
        extension_id: 'redis-infra-host',
        provider_code: 'redis',
        display_name: 'Redis',
        description: null,
        runtime_status: 'inactive',
        desired_state: 'disabled',
        config_ref: 'secret://system/redis-infra-host/config',
        contracts: ['storage-ephemeral', 'cache-store'],
        enabled_contracts: [],
        config_schema: [
          { key: 'host', label: 'Host', type: 'string', required: true },
          {
            key: 'port',
            label: 'Port',
            type: 'number',
            required: true,
            default_value: 6379
          },
          {
            key: 'password_ref',
            label: 'Password Secret Ref',
            type: 'string',
            send_mode: 'secret_ref'
          }
        ],
        config_json: {},
        restart_required: false
      }
    ]);
    api.saveSettingsHostInfrastructureProviderConfig.mockResolvedValue({
      restart_required: true,
      installation_desired_state: 'pending_restart',
      provider_config_status: 'pending_restart'
    });

    renderPanel(true);

    fireEvent.click(await screen.findByRole('button', { name: '配置' }));
    fireEvent.change(screen.getByLabelText('Host'), {
      target: { value: 'localhost' }
    });
    fireEvent.change(screen.getByLabelText('Port'), {
      target: { value: '6379' }
    });
    fireEvent.click(screen.getByLabelText('storage-ephemeral'));
    fireEvent.click(screen.getByRole('button', { name: '保存并等待重启' }));

    await waitFor(() => {
      expect(
        api.saveSettingsHostInfrastructureProviderConfig
      ).toHaveBeenCalledWith(
        'installation-1',
        'redis',
        {
          enabled_contracts: ['storage-ephemeral'],
          config_json: {
            host: 'localhost',
            port: 6379
          }
        },
        'csrf-123'
      );
    });
    expect(
      await screen.findByText('已保存，重启 api-server 一次后生效。')
    ).toBeInTheDocument();
  });

  test('loads memory tree and paged entries after selecting a path', async () => {
    api.fetchSettingsHostInfrastructureProviders.mockResolvedValue([]);
    api.fetchSettingsHostInfrastructureMemoryOverview.mockResolvedValue({
      can_manage: true,
      contracts: [
        {
          contract_code: 'session-store',
          label: 'Sessions',
          provider_code: 'local',
          capabilities: {
            list_entries: true,
            list_tree: true,
            search_entries: true,
            reveal_value: true
          },
          supported: true
        }
      ]
    });
    api.fetchSettingsHostInfrastructureMemoryStats.mockResolvedValue({
      contract_code: 'session-store',
      label: 'Sessions',
      provider_code: 'local',
      supported: true,
      inspection_path: [],
      entry_count: 2,
      sensitive_entry_count: 1,
      total_value_size_bytes: 2048
    });
    api.fetchSettingsHostInfrastructureMemoryStatsOverview.mockResolvedValue({
      inspection_path: [],
      entry_count: 3,
      sensitive_entry_count: 1,
      total_value_size_bytes: 3072,
      contracts: [
        {
          contract_code: 'session-store',
          label: 'Sessions',
          provider_code: 'local',
          supported: true,
          inspection_path: [],
          entry_count: 2,
          sensitive_entry_count: 1,
          total_value_size_bytes: 2048
        },
        {
          contract_code: 'cache-store',
          label: 'Cache',
          provider_code: 'local',
          supported: true,
          inspection_path: [],
          entry_count: 1,
          sensitive_entry_count: 0,
          total_value_size_bytes: 1024
        }
      ]
    });
    api.fetchSettingsHostInfrastructureMemoryTree.mockImplementation(
      (_contractCode: string, request?: { inspection_path?: string[] }) =>
        Promise.resolve({
          contract_code: 'session-store',
          label: 'Sessions',
          provider_code: 'local',
          capabilities: {
            list_entries: true,
            list_tree: true,
            search_entries: true,
            reveal_value: true
          },
          supported: true,
          inspection_path: request?.inspection_path ?? [],
          nodes: request?.inspection_path?.length
            ? [
                {
                  node_ref: 'opaque-user-node',
                  label: 'user-1',
                  inspection_path: ['workspace-1', 'user-1'],
                  depth: 2,
                  has_children: false
                }
              ]
            : [
                {
                  node_ref: 'opaque-workspace-node',
                  label: 'workspace-1',
                  inspection_path: ['workspace-1'],
                  depth: 1,
                  has_children: true
                }
              ],
          next_cursor: null,
          limit: 50,
          byte_limit: 65536,
          emitted_bytes: 128,
          truncated_by_byte_limit: false
        })
    );
    api.fetchSettingsHostInfrastructureMemoryEntries.mockImplementation(
      (
        _contractCode: string,
        request?: { cursor?: string | null; inspection_path?: string[] }
      ) =>
        Promise.resolve({
          contract_code: 'session-store',
          label: 'Sessions',
          provider_code: 'local',
          capabilities: {
            list_entries: true,
            list_tree: true,
            search_entries: true,
            reveal_value: true
          },
          supported: true,
          inspection_path: request?.inspection_path ?? [
            'workspace-1',
            'user-1'
          ],
          entries: [
            {
              contract_code: 'session-store',
              group_code: 'sessions',
              entry_ref:
                request?.cursor === 'cursor-2' ? 'session:2' : 'session:1',
              key: request?.cursor === 'cursor-2' ? 'session:2' : 'session:1',
              inspection_path: [
                'workspace-1',
                'user-1',
                request?.cursor === 'cursor-2' ? 'session:2' : 'session:1'
              ],
              entry_kind: 'session',
              status: 'active',
              owner: 'user-1',
              value_size_bytes: 1024,
              metadata_size_bytes: 32,
              ttl_seconds: 60,
              created_at_unix: 1_700_000_000,
              expires_at_unix: 1_700_000_060,
              sensitive: true,
              metadata: { workspace_id: 'workspace-1' }
            }
          ],
          next_cursor: request?.cursor === 'cursor-2' ? null : 'cursor-2',
          limit: 50,
          byte_limit: 65536,
          emitted_bytes: 256,
          truncated_by_byte_limit: false
        })
    );

    renderMemoryObservationPanel(true);

    expect(
      await screen.findByRole('tab', { name: 'Sessions' })
    ).toBeInTheDocument();
    await waitFor(() => {
      expect(
        api.fetchSettingsHostInfrastructureMemoryEntries
      ).not.toHaveBeenCalled();
    });

    fireEvent.click(await screen.findByRole('tab', { name: 'Sessions' }));
    expect(await screen.findByText('workspace-1')).toBeInTheDocument();
    expect(api.fetchSettingsHostInfrastructureMemoryTree).toHaveBeenCalledWith(
      'session-store',
      { inspection_path: [], limit: 50 }
    );
    fireEvent.click(screen.getByTestId('host-memory-panel-tree-switcher'));
    expect(await screen.findByText('user-1')).toBeInTheDocument();
    expect(api.fetchSettingsHostInfrastructureMemoryTree).toHaveBeenCalledWith(
      'session-store',
      { inspection_path: ['workspace-1'], limit: 50 }
    );
    fireEvent.click(screen.getByText('user-1'));

    expect(await screen.findByText('session:1')).toBeInTheDocument();
    expect(screen.getByText('1m 0s')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /元数据/ })).toBeEnabled();
    expect(screen.getByRole('button', { name: /查看值/ })).toBeEnabled();
    expect(
      screen.queryByRole('button', { name: /清理/ })
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '下一页' }));
    expect(await screen.findByText('session:2')).toBeInTheDocument();
    expect(
      api.fetchSettingsHostInfrastructureMemoryEntries
    ).toHaveBeenCalledWith('session-store', {
      inspection_path: ['workspace-1', 'user-1'],
      cursor: 'cursor-2',
      limit: 50
    });
    fireEvent.click(screen.getByRole('button', { name: '上一页' }));
    expect(await screen.findByText('session:1')).toBeInTheDocument();
  }, 10000);

  test('renders memory contracts as tabs with tree and table panes', async () => {
    api.fetchSettingsHostInfrastructureProviders.mockResolvedValue([]);
    api.fetchSettingsHostInfrastructureMemoryOverview.mockResolvedValue({
      can_manage: true,
      contracts: [
        {
          contract_code: 'session-store',
          label: 'Sessions',
          provider_code: 'local',
          capabilities: {
            list_entries: true,
            list_tree: true,
            search_entries: true,
            reveal_value: true
          },
          supported: true
        },
        {
          contract_code: 'cache-store',
          label: 'Cache',
          provider_code: 'local',
          capabilities: {
            list_entries: true,
            list_tree: true,
            search_entries: true,
            reveal_value: true
          },
          supported: true
        }
      ]
    });
    api.fetchSettingsHostInfrastructureMemoryStats.mockResolvedValue({
      contract_code: 'session-store',
      label: 'Sessions',
      provider_code: 'local',
      supported: true,
      inspection_path: [],
      entry_count: 2,
      sensitive_entry_count: 1,
      total_value_size_bytes: 2048
    });
    api.fetchSettingsHostInfrastructureMemoryStatsOverview.mockResolvedValue({
      inspection_path: [],
      entry_count: 3,
      sensitive_entry_count: 1,
      total_value_size_bytes: 3072,
      contracts: [
        {
          contract_code: 'session-store',
          label: 'Sessions',
          provider_code: 'local',
          supported: true,
          inspection_path: [],
          entry_count: 2,
          sensitive_entry_count: 1,
          total_value_size_bytes: 2048
        },
        {
          contract_code: 'cache-store',
          label: 'Cache',
          provider_code: 'local',
          supported: true,
          inspection_path: [],
          entry_count: 1,
          sensitive_entry_count: 0,
          total_value_size_bytes: 1024
        }
      ]
    });
    api.fetchSettingsHostInfrastructureMemoryTree.mockImplementation(
      (contractCode: string) =>
        Promise.resolve({
          contract_code: contractCode,
          label: contractCode === 'cache-store' ? 'Cache' : 'Sessions',
          provider_code: 'local',
          capabilities: {
            list_entries: true,
            list_tree: true,
            search_entries: true,
            reveal_value: true
          },
          supported: true,
          inspection_path: [],
          nodes: [
            {
              node_ref:
                contractCode === 'cache-store'
                  ? 'cache-domain-node'
                  : 'session-workspace-node',
              label:
                contractCode === 'cache-store'
                  ? 'application-cache'
                  : 'workspace-1',
              inspection_path:
                contractCode === 'cache-store'
                  ? ['application-cache']
                  : ['workspace-1'],
              depth: 1,
              entry_count: 1,
              sensitive_entry_count: 0,
              total_value_size_bytes:
                contractCode === 'cache-store' ? 1024 : 512,
              has_children: false
            }
          ],
          next_cursor: null,
          limit: 50,
          byte_limit: 65536,
          emitted_bytes: 128,
          truncated_by_byte_limit: false
        })
    );
    api.fetchSettingsHostInfrastructureMemoryEntries.mockImplementation(
      (contractCode: string, request?: { inspection_path?: string[] }) =>
        Promise.resolve({
          contract_code: contractCode,
          label: contractCode === 'cache-store' ? 'Cache' : 'Sessions',
          provider_code: 'local',
          capabilities: {
            list_entries: true,
            list_tree: true,
            search_entries: true,
            reveal_value: true
          },
          supported: true,
          inspection_path: request?.inspection_path ?? [],
          entries: [
            {
              contract_code: contractCode,
              group_code: contractCode === 'cache-store' ? 'cache' : 'sessions',
              entry_ref:
                contractCode === 'cache-store' ? 'cache:key:1' : 'session:1',
              key: contractCode === 'cache-store' ? 'cache:key:1' : 'session:1',
              inspection_path:
                contractCode === 'cache-store'
                  ? ['application-cache', 'cache:key:1']
                  : ['workspace-1', 'session:1'],
              entry_kind:
                contractCode === 'cache-store' ? 'cache_entry' : 'session',
              status: 'active',
              owner: null,
              value_size_bytes: contractCode === 'cache-store' ? 1024 : 512,
              metadata_size_bytes: 24,
              ttl_seconds: null,
              created_at_unix: null,
              expires_at_unix: null,
              sensitive: false,
              metadata: {}
            }
          ],
          next_cursor: null,
          limit: 50,
          byte_limit: 65536,
          emitted_bytes: 128,
          truncated_by_byte_limit: false
        })
    );

    renderMemoryObservationPanel(true);

    expect(await screen.findByRole('tab', { name: /^统计$/ })).toHaveAttribute(
      'aria-selected',
      'true'
    );
    expect(screen.getByRole('tab', { name: /^Sessions$/ })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: /^Cache$/ })).toBeInTheDocument();
    expect(await screen.findByText('内存统计')).toBeInTheDocument();
    expect(await screen.findByText('3 条目')).toBeInTheDocument();
    expect(await screen.findByText('1 敏感项')).toBeInTheDocument();
    expect(await screen.findByText('3.0 KB')).toBeInTheDocument();
    expect(screen.getByLabelText('内存统计图表')).toBeInTheDocument();
    await waitFor(() => {
      expect(echartsMock.chart.setOption).toHaveBeenCalled();
    });
    expect(
      await screen.findByTestId('service-card-session-store')
    ).toBeInTheDocument();
    expect(screen.getByTestId('service-card-cache-store')).toBeInTheDocument();
    expect(
      api.fetchSettingsHostInfrastructureMemoryTree
    ).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('tab', { name: /^Sessions$/ }));
    expect(screen.getByTestId('host-memory-panel-content')).toBeInTheDocument();
    expect(screen.getByTestId('host-memory-panel-tree')).toBeInTheDocument();
    expect(screen.getByTestId('host-memory-panel-entries')).toBeInTheDocument();
    fireEvent.click(await screen.findByText('workspace-1'));
    expect(await screen.findByText('session:1')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('tab', { name: /^Cache$/ }));

    expect(await screen.findByText('application-cache')).toBeInTheDocument();
    expect(screen.queryByText('session:1')).not.toBeInTheDocument();
    fireEvent.click(await screen.findByText('application-cache'));
    expect(await screen.findByText('cache:key:1')).toBeInTheDocument();
    expect(
      api.fetchSettingsHostInfrastructureMemoryEntries
    ).toHaveBeenLastCalledWith('cache-store', {
      inspection_path: ['application-cache'],
      cursor: null,
      limit: 50
    });
  }, 10000);

  test('explains when memory observation succeeds but there are no contracts', async () => {
    api.fetchSettingsHostInfrastructureProviders.mockResolvedValue([]);
    api.fetchSettingsHostInfrastructureMemoryOverview.mockResolvedValue({
      can_manage: true,
      contracts: []
    });

    renderMemoryObservationPanel(true);

    expect(
      await screen.findByText('暂无可观察内存 contract')
    ).toBeInTheDocument();
  });

  test('keeps memory reveal actions unavailable for view-only users', async () => {
    api.fetchSettingsHostInfrastructureProviders.mockResolvedValue([]);
    api.fetchSettingsHostInfrastructureMemoryOverview.mockResolvedValue({
      can_manage: false,
      contracts: [
        {
          contract_code: 'session-store',
          label: 'Sessions',
          provider_code: 'local',
          capabilities: {
            list_entries: true,
            list_tree: true,
            search_entries: true,
            reveal_value: true
          },
          entry_count: 1,
          sensitive_entry_count: 1,
          total_value_size_bytes: 1024,
          supported: true
        }
      ]
    });
    api.fetchSettingsHostInfrastructureMemoryTree.mockResolvedValue({
      contract_code: 'session-store',
      label: 'Sessions',
      provider_code: 'local',
      capabilities: {
        list_entries: true,
        list_tree: true,
        search_entries: true,
        reveal_value: true
      },
      supported: true,
      inspection_path: [],
      nodes: [
        {
          node_ref: 'workspace-1',
          label: 'workspace-1',
          inspection_path: ['workspace-1'],
          depth: 1,
          entry_count: 1,
          sensitive_entry_count: 1,
          total_value_size_bytes: 1024,
          has_children: false
        }
      ],
      next_cursor: null,
      limit: 50,
      byte_limit: 65536,
      emitted_bytes: 128,
      truncated_by_byte_limit: false
    });
    api.fetchSettingsHostInfrastructureMemoryEntries.mockResolvedValue({
      contract_code: 'session-store',
      label: 'Sessions',
      provider_code: 'local',
      capabilities: {
        list_entries: true,
        list_tree: true,
        search_entries: true,
        reveal_value: true
      },
      supported: true,
      inspection_path: ['workspace-1'],
      entries: [
        {
          contract_code: 'session-store',
          group_code: 'sessions',
          entry_ref: 'session:1',
          key: 'session:1',
          inspection_path: ['workspace-1', 'session:1'],
          entry_kind: 'session',
          status: 'active',
          owner: null,
          value_size_bytes: 1024,
          metadata_size_bytes: 2,
          ttl_seconds: null,
          created_at_unix: null,
          expires_at_unix: null,
          sensitive: true,
          metadata: {}
        }
      ],
      next_cursor: null,
      limit: 50,
      byte_limit: 65536,
      emitted_bytes: 128,
      truncated_by_byte_limit: false
    });

    renderMemoryObservationPanel(false);

    fireEvent.click(await screen.findByRole('tab', { name: 'Sessions' }));
    expect(
      await screen.findByText('当前视图只展示 metadata。')
    ).toBeInTheDocument();
    fireEvent.click(await screen.findByText('workspace-1'));
    expect(await screen.findByText('session:1')).toBeInTheDocument();
    expect(await screen.findByRole('button', { name: /元数据/ })).toBeEnabled();
    expect(
      screen.queryByRole('button', { name: /查看值/ })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: /清理/ })
    ).not.toBeInTheDocument();
  }, 10000);

  test('reveals memory value directly for manage users', async () => {
    api.fetchSettingsHostInfrastructureProviders.mockResolvedValue([]);
    api.fetchSettingsHostInfrastructureMemoryOverview.mockResolvedValue({
      can_manage: true,
      contracts: [
        {
          contract_code: 'session-store',
          label: 'Sessions',
          provider_code: 'local',
          capabilities: {
            list_entries: true,
            list_tree: true,
            search_entries: true,
            reveal_value: true
          },
          entry_count: 1,
          sensitive_entry_count: 1,
          total_value_size_bytes: 1024,
          supported: true
        }
      ]
    });
    api.fetchSettingsHostInfrastructureMemoryTree.mockResolvedValue({
      contract_code: 'session-store',
      label: 'Sessions',
      provider_code: 'local',
      capabilities: {
        list_entries: true,
        list_tree: true,
        search_entries: true,
        reveal_value: true
      },
      supported: true,
      inspection_path: [],
      nodes: [
        {
          node_ref: 'workspace-1',
          label: 'workspace-1',
          inspection_path: ['workspace-1'],
          depth: 1,
          entry_count: 1,
          sensitive_entry_count: 1,
          total_value_size_bytes: 1024,
          has_children: false
        }
      ],
      next_cursor: null,
      limit: 50,
      byte_limit: 65536,
      emitted_bytes: 128,
      truncated_by_byte_limit: false
    });
    api.fetchSettingsHostInfrastructureMemoryEntries.mockResolvedValue({
      contract_code: 'session-store',
      label: 'Sessions',
      provider_code: 'local',
      capabilities: {
        list_entries: true,
        list_tree: true,
        search_entries: true,
        reveal_value: true
      },
      supported: true,
      inspection_path: ['workspace-1'],
      entries: [
        {
          contract_code: 'session-store',
          group_code: 'sessions',
          entry_ref: 'session:1',
          key: 'session:1',
          inspection_path: ['workspace-1', 'session:1'],
          entry_kind: 'session',
          status: 'active',
          owner: null,
          value_size_bytes: 1024,
          metadata_size_bytes: 2,
          ttl_seconds: null,
          created_at_unix: null,
          expires_at_unix: null,
          sensitive: true,
          metadata: {}
        }
      ],
      next_cursor: null,
      limit: 50,
      byte_limit: 65536,
      emitted_bytes: 128,
      truncated_by_byte_limit: false
    });
    api.revealSettingsHostInfrastructureMemoryEntry.mockResolvedValue({
      metadata: {
        contract_code: 'session-store',
        group_code: 'sessions',
        entry_ref: 'session:1',
        key: 'session:1',
        inspection_path: ['workspace-1', 'session:1'],
        entry_kind: 'session',
        status: 'active',
        owner: null,
        value_size_bytes: 1024,
        metadata_size_bytes: 2,
        ttl_seconds: null,
        created_at_unix: null,
        expires_at_unix: null,
        sensitive: true,
        metadata: {}
      },
      reveal_mode: 'preview',
      value_state: 'available',
      value: { flow_run: { status: 'succeeded' } },
      value_preview: null,
      preview_size_bytes: 1024,
      full_value_size_bytes: 1024
    });

    renderMemoryObservationPanel(true);
    fireEvent.click(await screen.findByRole('tab', { name: 'Sessions' }));
    fireEvent.click(await screen.findByText('workspace-1'));
    fireEvent.click(await screen.findByRole('button', { name: /查看值/ }));

    await waitFor(() => {
      expect(
        api.revealSettingsHostInfrastructureMemoryEntry
      ).toHaveBeenCalledWith(
        'session-store',
        'session:1',
        'csrf-123',
        'preview'
      );
    });
    expect(
      screen.queryByRole('button', { name: '查看并记录审计' })
    ).not.toBeInTheDocument();
    expect(await screen.findByText('条目值')).toBeInTheDocument();
    expect(screen.getByLabelText('内存值 JSON')).toHaveTextContent('succeeded');
  }, 10000);

  test('shows preview state before full reveal reports oversized value', async () => {
    api.fetchSettingsHostInfrastructureProviders.mockResolvedValue([]);
    api.fetchSettingsHostInfrastructureMemoryOverview.mockResolvedValue({
      can_manage: true,
      contracts: [
        {
          contract_code: 'cache-store',
          label: 'Cache',
          provider_code: 'local',
          capabilities: {
            list_entries: true,
            list_tree: true,
            search_entries: true,
            reveal_value: true
          },
          entry_count: 1,
          sensitive_entry_count: 1,
          total_value_size_bytes: 400000,
          supported: true
        }
      ]
    });
    api.fetchSettingsHostInfrastructureMemoryTree.mockResolvedValue({
      contract_code: 'cache-store',
      label: 'Cache',
      provider_code: 'local',
      capabilities: {
        list_entries: true,
        list_tree: true,
        search_entries: true,
        reveal_value: true
      },
      supported: true,
      inspection_path: [],
      nodes: [
        {
          node_ref: 'application-logs',
          label: 'application-logs',
          inspection_path: ['application-logs'],
          depth: 1,
          entry_count: 1,
          sensitive_entry_count: 1,
          total_value_size_bytes: 400000,
          has_children: false
        }
      ],
      next_cursor: null,
      limit: 50,
      byte_limit: 65536,
      emitted_bytes: 128,
      truncated_by_byte_limit: false
    });
    api.fetchSettingsHostInfrastructureMemoryEntries.mockResolvedValue({
      contract_code: 'cache-store',
      label: 'Cache',
      provider_code: 'local',
      capabilities: {
        list_entries: true,
        list_tree: true,
        search_entries: true,
        reveal_value: true
      },
      supported: true,
      inspection_path: ['application-logs'],
      entries: [
        {
          contract_code: 'cache-store',
          group_code: 'application-logs',
          entry_ref: 'application-logs:run:1',
          key: 'application-logs:run:1',
          inspection_path: ['application-logs', 'run', '1'],
          entry_kind: 'cache_entry',
          status: 'active',
          owner: null,
          value_size_bytes: 400000,
          metadata_size_bytes: 24,
          ttl_seconds: null,
          created_at_unix: null,
          expires_at_unix: null,
          sensitive: true,
          metadata: { domain_code: 'application-logs' }
        }
      ],
      next_cursor: null,
      limit: 50,
      byte_limit: 65536,
      emitted_bytes: 128,
      truncated_by_byte_limit: false
    });
    api.revealSettingsHostInfrastructureMemoryEntry.mockImplementation(
      async (
        _contractCode: string,
        _entryRef: string,
        _csrfToken: string,
        revealMode: 'preview' | 'full'
      ) => ({
        metadata: {
          contract_code: 'cache-store',
          group_code: 'application-logs',
          entry_ref: 'application-logs:run:1',
          key: 'application-logs:run:1',
          inspection_path: ['application-logs', 'run', '1'],
          entry_kind: 'cache_entry',
          status: 'active',
          owner: null,
          value_size_bytes: 400000,
          metadata_size_bytes: 24,
          ttl_seconds: null,
          created_at_unix: null,
          expires_at_unix: null,
          sensitive: true,
          metadata: { domain_code: 'application-logs' }
        },
        reveal_mode: revealMode,
        value_state: revealMode === 'preview' ? 'preview' : 'value_too_large',
        value: null,
        value_preview: revealMode === 'preview' ? '{"blob":"xxxx' : null,
        preview_size_bytes: revealMode === 'preview' ? 12 : 0,
        full_value_size_bytes: 400000
      })
    );

    renderMemoryObservationPanel(true);
    fireEvent.click(await screen.findByRole('tab', { name: 'Cache' }));
    fireEvent.click(await screen.findByText('application-logs'));
    fireEvent.click(await screen.findByRole('button', { name: /查看值/ }));

    expect((await screen.findAllByText('preview')).length).toBeGreaterThan(0);
    expect(screen.getByText('查看模式')).toBeInTheDocument();
    expect(screen.getByText('{"blob":"xxxx')).toBeInTheDocument();
    fireEvent.click(await screen.findByText('完整查看'));

    await waitFor(() => {
      expect(
        api.revealSettingsHostInfrastructureMemoryEntry
      ).toHaveBeenCalledWith(
        'cache-store',
        'application-logs:run:1',
        'csrf-123',
        'full'
      );
    });
    expect(
      (await screen.findAllByText('value_too_large')).length
    ).toBeGreaterThan(0);
  }, 10000);
});
