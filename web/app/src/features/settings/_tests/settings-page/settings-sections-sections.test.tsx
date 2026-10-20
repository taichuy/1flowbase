import { describe, test, expect } from 'vitest';
import {
  authenticateWithPermissions,
  consoleNavigationApi,
  dataModelsApi,
  fireEvent,
  hostInfrastructureApi,
  renderApp,
  screen,
  settingsConsoleNavigation,
  systemRuntimeApi,
  waitFor
} from './settings-sections.fixtures';

describe('SettingsPage', () => {
  test('redirects /settings/docs to API key when docs is hidden', async () => {
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
      settingsConsoleNavigation(['api-key-authentication'])
    );
    authenticateWithPermissions(['user.view.all']);

    renderApp('/settings/docs');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/api-key-authentication');
    });
    expect(screen.getByTestId('section-page-layout')).toHaveClass(
      'section-page-layout--viewport'
    );
    expect(
      await screen.findByRole('button', { name: /添加/ })
    ).toBeInTheDocument();
  });

  test('shows 数据源 when state_model.view.all is present', async () => {
    authenticateWithPermissions(['state_model.view.all']);

    renderApp('/settings/data-models');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/data-models');
    });
    expect(await screen.findByRole('link', { name: '数据源' })).toHaveAttribute(
      'href',
      '/settings/data-models'
    );
    await waitFor(
      () => expect(dataModelsApi.fetchSettingsDataSources).toHaveBeenCalled(),
      { timeout: 10000 }
    );
    expect(
      await screen.findByText('主数据源', {}, { timeout: 10000 })
    ).toBeInTheDocument();
  });

  test('shows 系统运行 when its SettingsFeature is present', async () => {
    authenticateWithPermissions([
      'settings_feature.access.system.system-runtime'
    ]);

    renderApp('/settings/system-runtime');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/system-runtime');
    });
    expect(
      await screen.findByText('资源监控', {}, { timeout: 10000 })
    ).toBeInTheDocument();
    expect(screen.queryByText('运行概览')).not.toBeInTheDocument();
    expect(screen.queryByText('运行环境')).not.toBeInTheDocument();
    expect(screen.queryByText('同机部署')).not.toBeInTheDocument();
    expect(screen.queryByText('zh_Hans')).not.toBeInTheDocument();
    expect(screen.queryByText('相关进程内存')).not.toBeInTheDocument();
    expect(
      screen.getByRole('img', { name: '运行资源实时曲线' })
    ).toBeInTheDocument();
    expect(
      screen.queryByText('Runtime Extension Host')
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('combobox', { name: '运行目标' })
    ).not.toBeInTheDocument();
    expect(
      systemRuntimeApi.fetchSettingsSystemRuntimeProfile
    ).toHaveBeenCalled();
  });

  test('retired infrastructure URL falls back without showing an infrastructure entry', async () => {
    authenticateWithPermissions(['plugin_config.view.all']);
    renderApp('/settings/host-infrastructure');
    await waitFor(
      () => {
        expect(window.location.pathname).not.toBe(
          '/settings/host-infrastructure'
        );
      },
      { timeout: 10000 }
    );
    expect(
      screen.queryByRole('link', { name: '基础设施' })
    ).not.toBeInTheDocument();
    expect(
      await screen.findByRole('link', { name: '内存观察' })
    ).toHaveAttribute('href', '/settings/memory-observation');
  });

  test('renders memory observation as a settings section route', async () => {
    authenticateWithPermissions(['plugin_config.view.all']);
    hostInfrastructureApi.fetchSettingsHostInfrastructureMemoryOverview.mockResolvedValue(
      {
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
      }
    );
    hostInfrastructureApi.fetchSettingsHostInfrastructureMemoryStats.mockResolvedValue(
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
        supported: true,
        inspection_path: [],
        entry_count: 1,
        sensitive_entry_count: 1,
        total_value_size_bytes: 317
      }
    );
    hostInfrastructureApi.fetchSettingsHostInfrastructureMemoryStatsOverview.mockResolvedValue(
      {
        inspection_path: [],
        entry_count: 1,
        sensitive_entry_count: 1,
        total_value_size_bytes: 317,
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
            supported: true,
            inspection_path: [],
            entry_count: 1,
            sensitive_entry_count: 1,
            total_value_size_bytes: 317
          }
        ]
      }
    );
    hostInfrastructureApi.fetchSettingsHostInfrastructureMemoryTree.mockResolvedValue(
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
        supported: true,
        inspection_path: [],
        nodes: [
          {
            node_ref: 'root-session-node',
            label: '00000000-0000-0000-0000-000000000001',
            inspection_path: ['00000000-0000-0000-0000-000000000001'],
            depth: 1,
            has_children: false
          }
        ],
        next_cursor: null,
        limit: 50,
        byte_limit: 65536,
        emitted_bytes: 0,
        truncated_by_byte_limit: false
      }
    );
    hostInfrastructureApi.fetchSettingsHostInfrastructureMemoryEntries.mockResolvedValue(
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
        supported: true,
        inspection_path: ['00000000-0000-0000-0000-000000000001'],
        entries: [
          {
            contract_code: 'session-store',
            group_code: '00000000-0000-0000-0000-000000000001',
            entry_ref: 'session:1',
            key: 'session:1',
            inspection_path: [
              '00000000-0000-0000-0000-000000000001',
              'session:1'
            ],
            entry_kind: 'session',
            status: 'active',
            owner: 'user-1',
            value_size_bytes: 317,
            metadata_size_bytes: 2,
            ttl_seconds: 600,
            created_at_unix: 1_700_000_000,
            expires_at_unix: 1_700_000_600,
            sensitive: true,
            metadata: {}
          }
        ],
        next_cursor: null,
        limit: 50,
        byte_limit: 65536,
        emitted_bytes: 128,
        truncated_by_byte_limit: false
      }
    );

    renderApp('/settings/memory-observation');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/memory-observation');
    });
    expect(
      await screen.findByRole('link', { name: '内存观察' }, { timeout: 10000 })
    ).toHaveAttribute('href', '/settings/memory-observation');
    expect(
      await screen.findByRole('tab', { name: 'Sessions' }, { timeout: 10000 })
    ).toBeInTheDocument();
    expect(
      await screen.findByRole('tab', { name: '统计' }, { timeout: 10000 })
    ).toHaveAttribute('aria-selected', 'true');
    fireEvent.click(screen.getByRole('tab', { name: 'Sessions' }));
    fireEvent.click(
      await screen.findByText('00000000-0000-0000-0000-000000000001')
    );
    expect(await screen.findByText('session:1')).toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: 'Provider 配置' })
    ).not.toBeInTheDocument();
  }, 10000);

  test('shows 文件管理 when file_table.view.own is present', async () => {
    authenticateWithPermissions(['file_table.view.own']);

    renderApp('/settings/files');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/files');
    });
    expect(
      await screen.findByRole('tab', { name: '文件表' }, { timeout: 10_000 })
    ).toBeInTheDocument();
  });

  test('uses API key as the baseline settings section', async () => {
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
      settingsConsoleNavigation(['api-key-authentication'])
    );
    authenticateWithPermissions([]);

    renderApp('/settings');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/api-key-authentication');
    });
    expect(
      await screen.findByRole('button', { name: /添加/ })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('navigation', { name: 'Section navigation' })
    ).toBeInTheDocument();
  });
});
