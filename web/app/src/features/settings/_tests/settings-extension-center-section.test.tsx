import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { message, Modal } from 'antd';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const extensionsApi = vi.hoisted(() => ({
  settingsInstalledExtensionsQueryKey: vi.fn((cursor?: string) => [
    'settings',
    'extension-center',
    'installed',
    cursor ?? 'start'
  ]),
  settingsExtensionCatalogQueryKey: vi.fn(
    (category: string, cursor?: string) => [
      'settings',
      'extension-center',
      'catalog',
      category,
      cursor ?? 'start'
    ]
  ),
  fetchSettingsInstalledExtensions: vi.fn(),
  fetchSettingsExtensionCatalog: vi.fn(),
  fetchSettingsExtensionCatalogEntry: vi.fn(),
  checkSettingsExtensionUpdates: vi.fn(),
  installSettingsExtension: vi.fn(),
  getSettingsExtensionRiskChallenge: vi.fn(),
  previewSettingsInstalledMcpExtension: vi.fn(),
  applySettingsInstalledMcpExtension: vi.fn(),
  getSettingsInstalledMcpExtensionConflict: vi.fn(),
  getSettingsInstalledMcpExtensionIntegrityChallenge: vi.fn(),
  previewSettingsInstalledI18nExtension: vi.fn(),
  activateSettingsInstalledI18nExtension: vi.fn()
}));

const applicationsApi = vi.hoisted(() => ({
  previewInstalledApplicationExtension: vi.fn(),
  importInstalledApplicationExtension: vi.fn()
}));

const routerApi = vi.hoisted(() => ({ navigate: vi.fn() }));

vi.mock('../api/extensions', () => extensionsApi);
vi.mock('../../applications/api/applications', () => applicationsApi);
vi.mock('../api/mcp-management', () => ({
  settingsMcpCatalogQueryKey: ['settings', 'mcp-management', 'catalog']
}));
vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => routerApi.navigate
}));

import { AppI18nProvider } from '../../../app/AppI18nProvider';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { SettingsExtensionCenterSection } from '../pages/settings-page/SettingsExtensionCenterSection';

const installedEntry = {
  id: 'extension-installation-1',
  category: 'runtime-extensions' as const,
  catalog_id: 'runtime-extensions:taichuy/openai',
  organization: 'taichuy',
  artifact_id: 'openai',
  version: '1.0.0',
  node_id: 'node-1',
  source: 'official',
  trust: 'official',
  warnings: [],
  local_path: '/api/plugins/openai',
  checksum: 'sha256:installed',
  signature_status: 'valid',
  signature_algorithm: 'ed25519',
  signing_key_id: 'official-key',
  status: 'installed',
  application_action: 'configure_model_provider' as const,
  application_status: 'available' as const,
  installed_by: 'user-1',
  created_at: '2026-08-01T10:00:00Z',
  updated_at: '2026-08-01T10:00:00Z',
  installed_versions: [
    {
      id: 'extension-installation-1',
      version: '1.0.0',
      source: 'official',
      trust: 'official',
      warnings: [],
      local_path: '/api/plugins/openai/1.0.0',
      checksum: 'sha256:installed',
      signature_status: 'valid',
      signature_algorithm: 'ed25519',
      signing_key_id: 'official-key',
      status: 'installed',
      installed_by: 'user-1',
      created_at: '2026-08-01T10:00:00Z',
      updated_at: '2026-08-01T10:00:00Z'
    },
    {
      id: 'extension-installation-0',
      version: '0.9.0',
      source: 'upload',
      trust: 'unknown',
      warnings: [],
      local_path: '/api/plugins/openai/0.9.0',
      checksum: 'sha256:previous',
      signature_status: 'missing',
      signature_algorithm: null,
      signing_key_id: null,
      status: 'installed',
      installed_by: 'user-1',
      created_at: '2026-07-01T10:00:00Z',
      updated_at: '2026-07-01T10:00:00Z'
    }
  ]
};

const catalogEntry = {
  category: 'runtime-extensions' as const,
  id: 'runtime-extensions:taichuy/openai',
  name: 'OpenAI Provider',
  organization: 'taichuy',
  artifact: 'openai',
  version: '1.1.0',
  description: 'Remote provider extension',
  host_version_requirement: '>=0.4.0',
  source: { kind: 'github_release' },
  signature: { key_id: 'official-key' },
  checksum: 'sha256:catalog-entry',
  download_locator: { url: 'https://example.com/openai.1flowbasepkg' },
  catalog_page: 1,
  catalog_source: 'official',
  current_version: '1.0.0',
  installation_status: 'installed',
  artifact_kind: 'model_provider',
  installation_source: 'official',
  trust: 'official',
  warnings: [],
  compatibility: null
};

function authenticate() {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'user-1',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'user-1',
      account: 'root',
      email: 'root@example.com',
      phone: null,
      nickname: 'root',
      name: 'root',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'root',
      permissions: ['system.extension-center.manage.all']
    }
  });
}

function renderSection(
  category:
    | 'installed'
    | 'agent-flow'
    | 'capability-plugins'
    | 'host-extensions'
    | 'i18n'
    | 'mcp'
    | 'runtime-extensions' = 'installed',
  cursor?: string
) {
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false }
    }
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <AppI18nProvider>
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    </AppI18nProvider>
  );

  return {
    ...render(
      <SettingsExtensionCenterSection category={category} cursor={cursor} />,
      { wrapper }
    ),
    queryClient: client
  };
}

describe('SettingsExtensionCenterSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetAuthStore();
    authenticate();
    extensionsApi.fetchSettingsInstalledExtensions.mockResolvedValue({
      limit: 20,
      total_entries: 1,
      next_cursor: null,
      entries: [installedEntry]
    });
    extensionsApi.fetchSettingsExtensionCatalog.mockResolvedValue({
      category: 'runtime-extensions',
      catalog_page: 'page-1',
      catalog_page_number: 1,
      catalog_page_checksum: 'sha256:page-1',
      catalog_page_locator: 'runtime-extensions/catalog/v1/pages/1.json',
      limit: 20,
      next_cursor: null,
      total_entries: 1,
      entries: [catalogEntry]
    });
    extensionsApi.fetchSettingsExtensionCatalogEntry.mockResolvedValue(
      catalogEntry
    );
    extensionsApi.checkSettingsExtensionUpdates.mockResolvedValue({
      category: 'runtime-extensions',
      catalog_page: null,
      items: [
        {
          catalog_id: 'runtime-extensions:taichuy/openai',
          current_version: '1.0.0',
          latest_version: '1.1.0',
          status: 'update_available'
        }
      ]
    });
    extensionsApi.installSettingsExtension.mockResolvedValue({
      installation: installedEntry,
      local_artifact_was_present: false,
      node_plugin_installation_id: 'plugin-installation-1',
      application_action: 'configure_model_provider',
      application_status: 'available'
    });
    extensionsApi.getSettingsExtensionRiskChallenge.mockReturnValue(null);
    extensionsApi.getSettingsInstalledMcpExtensionConflict.mockReturnValue(
      null
    );
    extensionsApi.getSettingsInstalledMcpExtensionIntegrityChallenge.mockReturnValue(
      null
    );
    applicationsApi.previewInstalledApplicationExtension.mockResolvedValue({
      extension_installation_id: 'agent-installation-1',
      application_status: 'not_applied',
      integrity_warnings: [],
      required_integrity_override: null,
      preview: {
        application: {
          application_type: 'agent_flow',
          name: 'Imported flow',
          description: 'Flow description',
          icon: null,
          icon_type: null,
          icon_background: null
        },
        dependencies: [],
        unresolved_nodes: [],
        flow_document: {}
      }
    });
    vi.spyOn(Modal, 'confirm').mockReturnValue({ destroy: vi.fn() } as never);
    vi.spyOn(message, 'error').mockImplementation(vi.fn());
  });

  test('Root-AC-002/003 renders seven tabs, loads installed inventory first, and checks only visible pages', async () => {
    renderSection();

    expect(await screen.findByText('openai')).toBeInTheDocument();
    expect(screen.getAllByRole('tab')).toHaveLength(7);
    expect(
      screen.getByRole('columnheader', { name: '来源' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '可信度' })
    ).toBeInTheDocument();
    expect(screen.getByText('可配置')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '配置供应商' })
    ).toBeInTheDocument();
    expect(extensionsApi.fetchSettingsExtensionCatalog).not.toHaveBeenCalled();
    await waitFor(() => {
      expect(extensionsApi.checkSettingsExtensionUpdates).toHaveBeenCalledWith(
        {
          category: 'runtime-extensions',
          catalog_page: null,
          items: [
            {
              catalog_id: 'runtime-extensions:taichuy/openai',
              current_version: '1.0.0',
              installed_versions: ['1.0.0', '0.9.0']
            }
          ]
        },
        'csrf-123'
      );
    });
    expect(
      screen
        .getByRole('button', { name: '更新' })
        .closest('[data-update-state]')
    ).toHaveAttribute('data-update-state', 'update_available');

    fireEvent.click(screen.getByRole('tab', { name: 'runtime-extensions' }));
    expect(routerApi.navigate).toHaveBeenCalledWith({
      to: '/settings/extension-center/$category',
      params: { category: 'runtime-extensions' },
      search: { cursor: undefined }
    });
  });

  test('D4-AC-013 renders one family row and keeps every installed version in the view drawer', async () => {
    renderSection();

    const openaiRows = await screen.findAllByRole('row', { name: /openai/ });
    expect(openaiRows).toHaveLength(1);
    fireEvent.click(
      within(openaiRows[0]).getByRole('button', { name: '查看' })
    );
    const drawer = await screen.findByRole('dialog');
    expect(within(drawer).getByText('已安装版本')).toBeInTheDocument();
    expect(within(drawer).getByText('0.9.0')).toBeInTheDocument();
    expect(
      within(drawer).getByText('/api/plugins/openai/0.9.0')
    ).toBeInTheDocument();
  });

  test('D4-AC-014 never renders a response from a different catalog category under the active tab', async () => {
    renderSection('agent-flow');
    await waitFor(() => {
      expect(extensionsApi.fetchSettingsExtensionCatalog).toHaveBeenCalledWith(
        'agent-flow',
        undefined
      );
    });
    await waitFor(() => {
      expect(screen.queryByText('OpenAI Provider')).not.toBeInTheDocument();
      expect(screen.queryByText('openai')).not.toBeInTheDocument();
    });
  });

  test('Root-AC-004 resolves and performs an installed-row update instead of switching tabs', async () => {
    renderSection();
    const row = await screen.findByRole('row', { name: /openai/ });
    fireEvent.click(within(row).getByRole('button', { name: '更新' }));

    await waitFor(() => {
      expect(
        extensionsApi.fetchSettingsExtensionCatalogEntry
      ).toHaveBeenCalledWith(
        'runtime-extensions',
        'runtime-extensions:taichuy/openai'
      );
    });
    await waitFor(() => {
      expect(extensionsApi.installSettingsExtension).toHaveBeenCalledWith(
        catalogEntry,
        'csrf-123',
        {},
        true
      );
    });
  });

  test('Root-AC-006 lists a structured risk challenge and retries with its exact codes', async () => {
    const riskError = new Error('confirmation required');
    const challenge = {
      warnings: [
        {
          code: 'signature_invalid',
          message: '签名与产物内容不一致，但仍可继续。',
          overridable: true
        },
        {
          code: 'policy_locked',
          message: '该警告不可由用户确认覆盖。',
          overridable: false
        }
      ],
      compatibility: null
    };
    extensionsApi.installSettingsExtension
      .mockRejectedValueOnce(riskError)
      .mockResolvedValueOnce({});
    extensionsApi.getSettingsExtensionRiskChallenge.mockImplementation(
      (error: unknown) => (error === riskError ? challenge : null)
    );
    renderSection('runtime-extensions');
    const row = await screen.findByRole('row', { name: /OpenAI Provider/ });
    fireEvent.click(within(row).getByRole('button', { name: '更新' }));

    await waitFor(() => expect(Modal.confirm).toHaveBeenCalledTimes(1));
    const riskConfirmation = vi.mocked(Modal.confirm).mock.calls.at(-1)?.[0];
    render(riskConfirmation?.content as ReactNode);
    expect(
      screen.getByText('签名与产物内容不一致，但仍可继续。')
    ).toBeInTheDocument();
    expect(screen.getByText('该警告不可由用户确认覆盖。')).toBeInTheDocument();
    await riskConfirmation?.onOk?.();

    await waitFor(() => {
      expect(extensionsApi.installSettingsExtension).toHaveBeenLastCalledWith(
        catalogEntry,
        'csrf-123',
        {
          risk_override: {
            reason: 'user_confirmed',
            acknowledged_warnings: ['signature_invalid']
          }
        },
        true
      );
    });
  });

  test('D5-AC-005 removes unified upload while keeping failed update status visible', async () => {
    extensionsApi.checkSettingsExtensionUpdates.mockRejectedValue(
      new Error('catalog unavailable')
    );
    const view = renderSection();

    const row = await screen.findByRole('row', { name: /openai/ });
    await waitFor(() => {
      expect(
        within(row).getByRole('button', { name: '更新' }).closest('span')
          ?.parentElement
      ).toHaveAttribute('data-update-state', 'unknown_error');
    });

    expect(
      screen.queryByRole('button', { name: '上传插件' })
    ).not.toBeInTheDocument();
    expect(document.querySelector('input[type="file"]')).toBeNull();
    view.unmount();
  });

  test('Root-AC-004 keeps install available for all six catalog categories without artifact_kind', async () => {
    extensionsApi.fetchSettingsExtensionCatalog.mockImplementation(
      async (category: string) => ({
        category,
        catalog_page: 'page-1',
        catalog_page_number: 1,
        catalog_page_checksum: `sha256:${category}`,
        catalog_page_locator: `${category}/catalog/v1/pages/1.json`,
        limit: 20,
        next_cursor: null,
        total_entries: 1,
        entries: [
          {
            ...catalogEntry,
            category,
            id: `${category}:taichuy/sample`,
            name: `${category} Extension`,
            artifact_kind: null,
            current_version: null,
            installation_status: 'not_installed'
          }
        ]
      })
    );
    const view = renderSection('agent-flow');

    for (const category of [
      'agent-flow',
      'capability-plugins',
      'host-extensions',
      'i18n',
      'mcp',
      'runtime-extensions'
    ] as const) {
      view.rerender(<SettingsExtensionCenterSection category={category} />);
      const row = await screen.findByRole('row', {
        name: new RegExp(`${category} Extension`)
      });
      expect(
        within(row).getByRole('button', { name: '安装' })
      ).toBeInTheDocument();
    }
  });

  test('D5-AC-006 restores cursor from route search and writes pagination to navigation', async () => {
    renderSection('runtime-extensions', 'cursor-2');
    await waitFor(() => {
      expect(extensionsApi.fetchSettingsExtensionCatalog).toHaveBeenCalledWith(
        'runtime-extensions',
        'cursor-2'
      );
    });
    fireEvent.click(screen.getByRole('button', { name: '上一页' }));
    expect(routerApi.navigate).toHaveBeenCalledWith({
      to: '/settings/extension-center/$category',
      params: { category: 'runtime-extensions' },
      search: { cursor: undefined }
    });
  });

  test('D6-AC-001 installs Agent Flow without a generic confirmation and opens the shared application import preview', async () => {
    const agentCatalogEntry = {
      ...catalogEntry,
      id: 'agent-flow:taichuy/fusion',
      category: 'agent-flow' as const,
      artifact: 'fusion',
      name: 'Fusion',
      current_version: null,
      installation_status: 'not_installed'
    };
    extensionsApi.fetchSettingsExtensionCatalog.mockResolvedValue({
      category: 'agent-flow',
      catalog_page: 'start',
      catalog_page_number: 1,
      catalog_page_checksum: 'sha256:agent',
      catalog_page_locator: 'agent-flow/catalog/v1/pages/1.json',
      limit: 20,
      next_cursor: null,
      total_entries: 1,
      entries: [agentCatalogEntry]
    });
    extensionsApi.installSettingsExtension.mockResolvedValue({
      installation: { ...installedEntry, id: 'agent-installation-1' },
      local_artifact_was_present: false,
      node_plugin_installation_id: null,
      application_action: 'import_agent_flow',
      application_status: 'not_applied'
    });
    renderSection('agent-flow');
    const row = await screen.findByRole('row', { name: /Fusion/ });
    fireEvent.click(within(row).getByRole('button', { name: '安装' }));

    await waitFor(() => {
      expect(extensionsApi.installSettingsExtension).toHaveBeenCalledWith(
        agentCatalogEntry,
        'csrf-123',
        {},
        false
      );
      expect(
        applicationsApi.previewInstalledApplicationExtension
      ).toHaveBeenCalledWith('agent-installation-1');
    });
    expect(Modal.confirm).not.toHaveBeenCalled();
    expect(
      await screen.findByRole('dialog', { name: /导入应用压缩包/ })
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /取\s*消/ }));
    expect(
      applicationsApi.importInstalledApplicationExtension
    ).not.toHaveBeenCalled();
  });

  test('D6-AC-002 reuses the complete MCP bundle review and applies exact preview decisions', async () => {
    const mcpEntry = {
      ...installedEntry,
      id: 'mcp-installation-1',
      category: 'mcp' as const,
      catalog_id: 'mcp:taichuy/sample',
      artifact_id: 'sample',
      application_action: 'import_mcp' as const,
      application_status: 'not_applied' as const
    };
    extensionsApi.fetchSettingsInstalledExtensions.mockResolvedValue({
      limit: 20,
      total_entries: 1,
      next_cursor: null,
      entries: [mcpEntry]
    });
    extensionsApi.previewSettingsInstalledMcpExtension.mockResolvedValue({
      extension_installation_id: 'mcp-installation-1',
      artifact_installation_status: 'installed',
      workspace_application_status: 'confirmation_required',
      required_conflict_resolution: 'keep_existing',
      integrity_warnings: [
        {
          code: 'checksum_mismatch',
          message: '本地产物校验值与安装记录不一致。',
          overridable: true
        }
      ],
      required_integrity_override: {
        warnings: [
          {
            code: 'checksum_mismatch',
            message: '本地产物校验值与安装记录不一致。',
            overridable: true
          }
        ],
        compatibility: null
      },
      preview: {
        manifest: {
          organization: 'taichuy',
          bundle_id: 'sample',
          bundle_version: '1.0.0',
          locale: 'zh_Hans',
          minimum_host_version: '*',
          exported_from_system_version: '1.0.0'
        },
        current_system_version: '1.0.0',
        version_status: 'same_system_version',
        tools: [{ id: 'tool.weather', result: 'imported', reason: null }],
        instances: [],
        connections: []
      }
    });
    extensionsApi.applySettingsInstalledMcpExtension.mockResolvedValue({
      extension_installation_id: 'mcp-installation-1',
      artifact_installation_status: 'installed',
      workspace_application_status: 'imported',
      integrity_warnings: [],
      import_report: {
        manifest: {
          organization: 'taichuy',
          bundle_id: 'sample',
          bundle_version: '1.0.0',
          locale: 'zh_Hans',
          minimum_host_version: '*',
          exported_from_system_version: '1.0.0'
        },
        current_system_version: '1.0.0',
        version_status: 'same_system_version',
        status: 'completed',
        tools: [{ id: 'tool.weather', result: 'imported', reason: null }],
        instances: [],
        connections: []
      }
    });
    renderSection('installed');
    const row = await screen.findByRole('row', { name: /sample/ });
    fireEvent.click(within(row).getByRole('button', { name: '应用到工作区' }));
    await waitFor(() => {
      expect(
        extensionsApi.previewSettingsInstalledMcpExtension
      ).toHaveBeenCalledWith('mcp-installation-1', 'csrf-123');
    });
    expect(
      await screen.findByText('本地产物校验值与安装记录不一致。')
    ).toBeInTheDocument();
    expect(screen.getByText('tool.weather')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /导入/ }));
    await waitFor(() => {
      expect(
        extensionsApi.applySettingsInstalledMcpExtension
      ).toHaveBeenCalledWith('mcp-installation-1', 'csrf-123', {
        conflict_resolution: 'keep_existing',
        integrity_override: {
          reason: 'user_confirmed',
          acknowledged_warnings: ['checksum_mismatch']
        }
      });
    });
  });

  test('D6-AC-003 previews and activates the installed local i18n catalog', async () => {
    const i18nRow = {
      ...installedEntry,
      id: 'i18n-installation-1',
      category: 'i18n' as const,
      artifact_id: 'platform',
      application_action: 'activate_i18n' as const,
      application_status: 'not_applied' as const
    };
    extensionsApi.fetchSettingsInstalledExtensions.mockResolvedValue({
      limit: 20,
      total_entries: 1,
      next_cursor: null,
      entries: [i18nRow]
    });
    extensionsApi.previewSettingsInstalledI18nExtension.mockResolvedValue({
      extension_installation_id: 'i18n-installation-1',
      application_status: 'not_applied',
      active_catalog_version: '2.0.0',
      installed_catalog_version: '2.0.1',
      revision: 4,
      integrity_warnings: [],
      required_integrity_override: null
    });
    extensionsApi.activateSettingsInstalledI18nExtension.mockResolvedValue({
      status: 'activated',
      catalog_version: '2.0.1',
      revision: 5
    });
    renderSection('installed');
    const row = await screen.findByRole('row', { name: /platform/ });
    fireEvent.click(within(row).getByRole('button', { name: '激活' }));
    const dialog = await screen.findByRole('dialog', {
      name: '激活多语言目录'
    });
    expect(within(dialog).getByText('2.0.0')).toBeInTheDocument();
    expect(within(dialog).getByText('2.0.1')).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole('button', { name: /激\s*活/ }));
    await waitFor(() => {
      expect(
        extensionsApi.activateSettingsInstalledI18nExtension
      ).toHaveBeenCalledWith(
        'i18n-installation-1',
        { expected_revision: 4 },
        'csrf-123'
      );
    });
  });
});
