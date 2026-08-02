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
  settingsInstalledExtensionsQueryKey: vi.fn(
    (cursor?: string, category?: string) => [
      'settings',
      'extension-center',
      'installed',
      category ?? 'all',
      cursor ?? 'start'
    ]
  ),
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
  getSettingsInstalledMcpExtensionIntegrityChallenge: vi.fn()
}));

const i18nCatalogApi = vi.hoisted(() => ({
  settingsI18nCatalogQueryKey: ['settings', 'i18n-catalog'] as const,
  previewSettingsInstalledI18nCatalog: vi.fn(),
  activateSettingsInstalledI18nCatalog: vi.fn()
}));

const applicationsApi = vi.hoisted(() => ({
  previewInstalledApplicationExtension: vi.fn(),
  importInstalledApplicationExtension: vi.fn()
}));

const mcpManagementApi = vi.hoisted(() => ({
  settingsMcpCatalogQueryKey: [
    'settings',
    'mcp-management',
    'catalog'
  ] as const,
  previewSettingsMcpBundle: vi.fn(),
  importSettingsMcpBundle: vi.fn(),
  previewSettingsOfficialMcpBundle: vi.fn(),
  importSettingsOfficialMcpBundle: vi.fn()
}));

const routerApi = vi.hoisted(() => ({ navigate: vi.fn() }));

vi.mock('../api/extensions', () => extensionsApi);
vi.mock('../api/i18n-catalog', () => i18nCatalogApi);
vi.mock('../../applications/api/applications', () => applicationsApi);
vi.mock('../api/mcp-management', () => mcpManagementApi);
vi.mock('../components/mcp-management/bundle/McpTemplateLibrary', () => ({
  McpTemplateLibrary: ({ variant }: { variant?: string }) => (
    <div data-testid="mcp-template-library" data-variant={variant} />
  )
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
  is_current: true,
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
      is_current: true,
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
      is_current: false,
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

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => {
    resolve = next;
  });
  return { promise, resolve };
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

  test('AC-005 loads installed inventory without a remote update check and checks only after the explicit action', async () => {
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
    expect(extensionsApi.checkSettingsExtensionUpdates).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '检查更新' }));
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

  test('AC-006 exposes contextual links to the MCP and language management owners', async () => {
    extensionsApi.fetchSettingsExtensionCatalog.mockImplementation(
      async (category: string) => ({
        category,
        catalog_page: 'page-1',
        catalog_page_number: 1,
        catalog_page_checksum: `sha256:${category}`,
        catalog_page_locator: `${category}/catalog/v1/pages/1.json`,
        limit: 20,
        next_cursor: null,
        total_entries: 0,
        entries: []
      })
    );
    const view = renderSection('mcp');

    expect(
      await screen.findByRole('link', { name: '前往 MCP 管理' })
    ).toHaveAttribute('href', '/settings/mcp-management?tab=instances');
    expect(screen.getByTestId('mcp-template-library')).toHaveAttribute(
      'data-variant',
      'compact'
    );
    expect(extensionsApi.fetchSettingsExtensionCatalog).not.toHaveBeenCalled();

    view.rerender(<SettingsExtensionCenterSection category="i18n" />);
    expect(
      await screen.findByRole('link', { name: '前往多语言管理' })
    ).toHaveAttribute('href', '/settings/i18n');
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

  test('AC-005 routes Agent Flow through the generic catalog and links to local template management', async () => {
    extensionsApi.fetchSettingsExtensionCatalog.mockResolvedValue({
      category: 'agent-flow',
      catalog_page: 'page-1',
      catalog_page_number: 1,
      catalog_page_checksum: 'sha256:agent-flow',
      catalog_page_locator: 'agent-flow/catalog/v1/pages/1.json',
      limit: 20,
      next_cursor: null,
      total_entries: 1,
      entries: [
        {
          ...catalogEntry,
          category: 'agent-flow',
          id: 'agent-flow:taichuy/fusion',
          name: 'Fusion',
          current_version: null,
          installation_status: 'not_installed'
        }
      ]
    });
    renderSection('agent-flow');
    const row = await screen.findByRole('row', { name: /Fusion/ });
    expect(within(row).getByRole('button', { name: '安装' })).toBeEnabled();
    expect(
      screen.getByRole('link', { name: '前往 Agent Flow 模板管理' })
    ).toHaveAttribute('href', '/templates');
  });

  test('Root-AC-004 resolves and performs an installed-row update instead of switching tabs', async () => {
    renderSection();
    const row = await screen.findByRole('row', { name: /openai/ });
    fireEvent.click(screen.getByRole('button', { name: '检查更新' }));
    await waitFor(() =>
      expect(within(row).getByRole('button', { name: '更新' })).toBeEnabled()
    );
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

  test('AC-002 keeps the installed target row loading for the complete update request', async () => {
    const secondInstalledEntry = {
      ...installedEntry,
      id: 'extension-installation-2',
      catalog_id: 'runtime-extensions:taichuy/anthropic',
      artifact_id: 'anthropic',
      installed_versions: []
    };
    extensionsApi.fetchSettingsInstalledExtensions.mockResolvedValue({
      limit: 20,
      total_entries: 2,
      next_cursor: null,
      entries: [installedEntry, secondInstalledEntry]
    });
    const updateResult = {
      installation: installedEntry,
      local_artifact_was_present: true,
      node_plugin_installation_id: 'plugin-installation-1',
      application_action: 'configure_model_provider' as const,
      application_status: 'available' as const
    };
    const pendingUpdate = deferred<typeof updateResult>();
    extensionsApi.installSettingsExtension.mockReturnValue(
      pendingUpdate.promise
    );
    extensionsApi.checkSettingsExtensionUpdates.mockResolvedValue({
      category: 'runtime-extensions',
      catalog_page: null,
      items: [
        {
          catalog_id: installedEntry.catalog_id,
          current_version: '1.0.0',
          latest_version: '1.1.0',
          status: 'update_available'
        },
        {
          catalog_id: secondInstalledEntry.catalog_id,
          current_version: '1.0.0',
          latest_version: '1.1.0',
          status: 'update_available'
        }
      ]
    });

    renderSection();
    const targetRow = await screen.findByRole('row', { name: /openai/ });
    const otherRow = await screen.findByRole('row', { name: /anthropic/ });
    const targetButton = within(targetRow).getByRole('button', {
      name: '更新'
    });
    const otherButton = within(otherRow).getByRole('button', {
      name: '更新'
    });
    fireEvent.click(screen.getByRole('button', { name: '检查更新' }));
    await waitFor(() => {
      expect(targetButton).toBeEnabled();
      expect(otherButton).toBeEnabled();
    });
    fireEvent.click(targetButton);

    await waitFor(() => {
      expect(extensionsApi.installSettingsExtension).toHaveBeenCalled();
      expect(targetButton).toHaveClass('ant-btn-loading');
      expect(otherButton).toBeDisabled();
      expect(otherButton).not.toHaveClass('ant-btn-loading');
    });

    pendingUpdate.resolve(updateResult);
    await waitFor(() => {
      expect(targetButton).not.toHaveClass('ant-btn-loading');
      expect(otherButton).toBeEnabled();
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
    expect(
      within(screen.getByRole('row', { name: /OpenAI Provider/ })).getByRole(
        'button',
        { name: /更新$/ }
      )
    ).toHaveClass('ant-btn-loading');
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
    await waitFor(() => {
      expect(
        within(screen.getByRole('row', { name: /OpenAI Provider/ })).getByRole(
          'button',
          { name: /更新$/ }
        )
      ).not.toHaveClass('ant-btn-loading');
    });
  });

  test('D5-AC-005 removes unified upload while keeping failed update status visible', async () => {
    extensionsApi.checkSettingsExtensionUpdates.mockRejectedValue(
      new Error('catalog unavailable')
    );
    const view = renderSection();

    const row = await screen.findByRole('row', { name: /openai/ });
    fireEvent.click(screen.getByRole('button', { name: '检查更新' }));
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

  test('Root-AC-004 keeps generic install for four catalog categories while MCP uses its library', async () => {
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
    const view = renderSection('capability-plugins');

    for (const category of [
      'capability-plugins',
      'host-extensions',
      'i18n',
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

    view.rerender(<SettingsExtensionCenterSection category="mcp" />);
    expect(screen.getByTestId('mcp-template-library')).toHaveAttribute(
      'data-variant',
      'compact'
    );
    expect(screen.queryByText('mcp Extension')).not.toBeInTheDocument();
  });

  test('AC-001 loads only the generic catalog row being installed and disables the other row', async () => {
    const fusionEntry = {
      ...catalogEntry,
      id: 'capability-plugins:taichuy/fusion',
      category: 'capability-plugins' as const,
      artifact: 'fusion',
      name: 'Fusion',
      current_version: null,
      installation_status: 'not_installed'
    };
    const deepseekEntry = {
      ...fusionEntry,
      id: 'capability-plugins:taichuy/deepseek-v4',
      artifact: 'deepseek-v4',
      name: 'Deepseek V4'
    };
    extensionsApi.fetchSettingsExtensionCatalog.mockResolvedValue({
      category: 'capability-plugins',
      catalog_page: 'start',
      catalog_page_number: 1,
      catalog_page_checksum: 'sha256:agent',
      catalog_page_locator: 'capability-plugins/catalog/v1/pages/1.json',
      limit: 20,
      next_cursor: null,
      total_entries: 2,
      entries: [fusionEntry, deepseekEntry]
    });
    const installResult = {
      installation: { ...installedEntry, id: 'agent-installation-1' },
      local_artifact_was_present: false,
      node_plugin_installation_id: null,
      application_action: 'none' as const,
      application_status: 'not_required' as const
    };
    const pendingInstall = deferred<typeof installResult>();
    extensionsApi.installSettingsExtension.mockReturnValue(
      pendingInstall.promise
    );

    renderSection('capability-plugins');
    const targetRow = await screen.findByRole('row', { name: /Fusion/ });
    await screen.findByRole('row', { name: /Deepseek V4/ });
    const targetButton = within(targetRow).getByRole('button', {
      name: '安装'
    });
    fireEvent.click(targetButton);

    await waitFor(() => {
      const currentTargetButton = within(
        screen.getByRole('row', { name: /Fusion/ })
      ).getByRole('button', { name: /安装$/ });
      const currentOtherButton = within(
        screen.getByRole('row', { name: /Deepseek V4/ })
      ).getByRole('button', { name: '安装' });
      expect(extensionsApi.installSettingsExtension).toHaveBeenCalledWith(
        fusionEntry,
        'csrf-123',
        {},
        false
      );
      expect(currentTargetButton).toHaveClass('ant-btn-loading');
      expect(currentOtherButton).toBeDisabled();
      expect(currentOtherButton).not.toHaveClass('ant-btn-loading');
    });

    pendingInstall.resolve(installResult);
    await waitFor(() => {
      const currentTargetButton = within(
        screen.getByRole('row', { name: /Fusion/ })
      ).getByRole('button', { name: '安装' });
      const currentOtherButton = within(
        screen.getByRole('row', { name: /Deepseek V4/ })
      ).getByRole('button', { name: '安装' });
      expect(currentTargetButton).not.toHaveClass('ant-btn-loading');
      expect(currentOtherButton).toBeEnabled();
    });
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

  test('AC-005 exposes Agent Flow management from its generic catalog', async () => {
    extensionsApi.fetchSettingsExtensionCatalog.mockResolvedValue({
      category: 'agent-flow',
      catalog_page: 'page-1',
      catalog_page_number: 1,
      catalog_page_checksum: 'sha256:agent-flow',
      catalog_page_locator: 'agent-flow/catalog/v1/pages/1.json',
      limit: 20,
      next_cursor: null,
      total_entries: 0,
      entries: []
    });
    renderSection('agent-flow');
    expect(
      await screen.findByRole('link', { name: '前往 Agent Flow 模板管理' })
    ).toHaveAttribute('href', '/templates');
    expect(extensionsApi.fetchSettingsExtensionCatalog).toHaveBeenCalledWith(
      'agent-flow',
      undefined
    );
    expect(extensionsApi.installSettingsExtension).not.toHaveBeenCalled();
    expect(
      applicationsApi.previewInstalledApplicationExtension
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
      workspace_application_status: 'ready_to_import',
      required_conflict_resolution: null,
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
        effect_summary: {
          changes: 1,
          already_present: 0,
          conflicts: 0,
          unavailable: 0,
          failed: 0
        },
        tools: [
          {
            id: 'tool.weather',
            effect: 'create',
            result: 'imported',
            reason: null
          }
        ],
        instances: [],
        connections: [],
        shared_tool_impacts: []
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
        effect_summary: {
          changes: 1,
          already_present: 0,
          conflicts: 0,
          unavailable: 0,
          failed: 0
        },
        tools: [
          {
            id: 'tool.weather',
            effect: 'create',
            result: 'imported',
            reason: null
          }
        ],
        instances: [],
        connections: [],
        shared_tool_impacts: []
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
        integrity_override: {
          reason: 'user_confirmed',
          acknowledged_warnings: ['checksum_mismatch']
        }
      });
    });
  });

  test('AC-002 reconciles an already-present MCP configuration and shows the changed application state', async () => {
    const mcpEntry = {
      ...installedEntry,
      id: 'mcp-installation-present',
      category: 'mcp' as const,
      catalog_id: 'mcp:taichuy/present',
      artifact_id: 'present',
      application_action: 'import_mcp' as const,
      application_status: 'not_applied' as const
    };
    const review = {
      manifest: {
        organization: 'taichuy',
        bundle_id: 'present',
        bundle_version: '1.0.0',
        locale: 'zh_Hans' as const,
        minimum_host_version: '*',
        exported_from_system_version: '1.0.0'
      },
      current_system_version: '1.0.0',
      version_status: 'same_system_version' as const,
      effect_summary: {
        changes: 0,
        already_present: 1,
        conflicts: 0,
        unavailable: 0,
        failed: 0
      },
      tools: [
        {
          id: 'tool.weather',
          effect: 'already_present' as const,
          result: 'already_present' as const,
          reason: null
        }
      ],
      instances: [],
      connections: [],
      shared_tool_impacts: []
    };
    extensionsApi.fetchSettingsInstalledExtensions.mockResolvedValue({
      limit: 20,
      total_entries: 1,
      next_cursor: null,
      entries: [mcpEntry]
    });
    extensionsApi.previewSettingsInstalledMcpExtension.mockResolvedValue({
      extension_installation_id: mcpEntry.id,
      artifact_installation_status: 'installed',
      workspace_application_status: 'already_present',
      required_conflict_resolution: null,
      integrity_warnings: [],
      required_integrity_override: null,
      preview: review
    });
    extensionsApi.applySettingsInstalledMcpExtension.mockResolvedValue({
      extension_installation_id: mcpEntry.id,
      artifact_installation_status: 'installed',
      workspace_application_status: 'imported',
      integrity_warnings: [],
      import_report: { ...review, status: 'already_applied' }
    });

    renderSection('installed');
    const row = await screen.findByRole('row', { name: /present/ });
    fireEvent.click(within(row).getByRole('button', { name: '应用到工作区' }));
    expect(await screen.findByText('tool.weather')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '确认导入并覆盖' }));

    await waitFor(() =>
      expect(
        extensionsApi.applySettingsInstalledMcpExtension
      ).toHaveBeenCalledWith(mcpEntry.id, 'csrf-123', {})
    );
    expect(
      await screen.findByText('配置已存在，扩展应用状态已同步')
    ).toBeInTheDocument();
  });

  test('AC-001/003 confirms instance overwrite without keep_existing conflict resolution', async () => {
    const mcpEntry = {
      ...installedEntry,
      id: 'mcp-installation-conflict',
      category: 'mcp' as const,
      catalog_id: 'mcp:taichuy/conflict',
      artifact_id: 'conflict',
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
      extension_installation_id: mcpEntry.id,
      artifact_installation_status: 'installed',
      workspace_application_status: 'confirmation_required',
      required_conflict_resolution: 'keep_existing',
      integrity_warnings: [],
      required_integrity_override: null,
      preview: {
        manifest: {
          organization: 'taichuy',
          bundle_id: 'conflict',
          bundle_version: '1.0.0',
          locale: 'zh_Hans',
          minimum_host_version: '*',
          exported_from_system_version: '1.0.0'
        },
        current_system_version: '1.0.0',
        version_status: 'same_system_version',
        effect_summary: {
          changes: 0,
          already_present: 0,
          conflicts: 1,
          unavailable: 0,
          failed: 0
        },
        tools: [
          {
            id: 'tool.weather',
            effect: 'conflict',
            result: 'skipped',
            reason: 'tool_id_conflict'
          }
        ],
        instances: [],
        connections: [],
        shared_tool_impacts: []
      }
    });

    renderSection('installed');
    const row = await screen.findByRole('row', { name: /conflict/ });
    fireEvent.click(within(row).getByRole('button', { name: '应用到工作区' }));

    expect(
      await screen.findByText(
        '导入会按 instance_id 创建或原子覆盖实例；模板同步不会修改已导入实例。'
      )
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '确认导入并覆盖' }));
    await waitFor(() =>
      expect(
        extensionsApi.applySettingsInstalledMcpExtension
      ).toHaveBeenCalledWith(mcpEntry.id, 'csrf-123', {})
    );
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
    i18nCatalogApi.previewSettingsInstalledI18nCatalog.mockResolvedValue({
      extension_installation_id: 'i18n-installation-1',
      application_status: 'not_applied',
      active_catalog_version: '2.0.0',
      installed_catalog_version: '2.0.1',
      revision: 4,
      integrity_warnings: [],
      required_integrity_override: null
    });
    i18nCatalogApi.activateSettingsInstalledI18nCatalog.mockResolvedValue({
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
    expect(await within(dialog).findByText('2.0.0')).toBeInTheDocument();
    expect(await within(dialog).findByText('2.0.1')).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole('button', { name: /激\s*活/ }));
    await waitFor(() => {
      expect(
        i18nCatalogApi.activateSettingsInstalledI18nCatalog
      ).toHaveBeenCalledWith(
        'i18n-installation-1',
        { expected_revision: 4 },
        'csrf-123'
      );
    });
  });
});
