import fs from 'node:fs';
import path from 'node:path';

import { render, screen } from '@testing-library/react';
import { Grid } from 'antd';
import { vi } from 'vitest';
import {
  modelProviderCatalogEntries,
  primaryContractProviderModels
} from '../../../../test/model-provider-contract-fixtures';

const membersApi = vi.hoisted(() => ({
  settingsMembersQueryKey: ['settings', 'members'],
  fetchSettingsMembers: vi.fn(),
  createSettingsMember: vi.fn(),
  disableSettingsMember: vi.fn(),
  enableSettingsMember: vi.fn(),
  deleteSettingsMember: vi.fn(),
  resetSettingsMemberPassword: vi.fn(),
  replaceSettingsMemberRoles: vi.fn()
}));

const rolesApi = vi.hoisted(() => ({
  settingsRolesQueryKey: ['settings', 'roles'],
  settingsRolePermissionsQueryKey: vi.fn((roleCode: string) => [
    'settings',
    'roles',
    roleCode,
    'permissions'
  ]),
  fetchSettingsRoles: vi.fn(),
  createSettingsRole: vi.fn(),
  updateSettingsRole: vi.fn(),
  deleteSettingsRole: vi.fn(),
  fetchSettingsRolePermissions: vi.fn(),
  replaceSettingsRolePermissions: vi.fn()
}));

const permissionsApi = vi.hoisted(() => ({
  settingsPermissionsQueryKey: ['settings', 'permissions'],
  fetchSettingsPermissions: vi.fn()
}));

const docsApi = vi.hoisted(() => ({
  settingsApiDocsCatalogQueryKey: ['settings', 'docs', 'catalog'],
  settingsApiDocsCategoryOperationsQueryKey: vi.fn((categoryId: string) => [
    'settings',
    'docs',
    'category',
    categoryId,
    'operations'
  ]),
  settingsApiDocsOperationSpecQueryKey: vi.fn((operationId: string) => [
    'settings',
    'docs',
    'operation',
    operationId,
    'openapi'
  ]),
  fetchSettingsApiDocsCatalog: vi.fn(),
  fetchSettingsApiDocsCategoryOperations: vi.fn(),
  fetchSettingsApiDocsOperationSpec: vi.fn()
}));

const modelProvidersApi = vi.hoisted(() => ({
  settingsModelProviderCatalogQueryKey: [
    'settings',
    'model-providers',
    'catalog'
  ],
  settingsModelProviderInstancesQueryKey: [
    'settings',
    'model-providers',
    'instances'
  ],
  settingsModelProviderOptionsQueryKey: [
    'settings',
    'model-providers',
    'options'
  ],
  settingsModelProviderModelsQueryKey: vi.fn((instanceId: string) => [
    'settings',
    'model-providers',
    'models',
    instanceId
  ]),
  fetchSettingsModelProviderCatalog: vi.fn(),
  fetchSettingsModelProviderInstances: vi.fn(),
  fetchSettingsModelProviderOptions: vi.fn(),
  fetchSettingsModelProviderMainInstance: vi.fn(),
  fetchSettingsModelProviderModels: vi.fn(),
  previewSettingsModelProviderModels: vi.fn(),
  createSettingsModelProviderInstance: vi.fn(),
  updateSettingsModelProviderInstance: vi.fn(),
  updateSettingsModelProviderMainInstance: vi.fn(),
  revealSettingsModelProviderSecret: vi.fn(),
  validateSettingsModelProviderInstance: vi.fn(),
  refreshSettingsModelProviderModels: vi.fn(),
  deleteSettingsModelProviderInstance: vi.fn()
}));

const pluginsApi = vi.hoisted(() => ({
  settingsOfficialPluginsQueryKey: ['settings', 'plugins', 'official-catalog'],
  settingsPluginFamiliesQueryKey: ['settings', 'plugins', 'families'],
  fetchSettingsPluginFamilies: vi.fn(),
  fetchSettingsOfficialPluginCatalog: vi.fn(),
  installSettingsOfficialPlugin: vi.fn(),
  uploadSettingsPluginPackage: vi.fn(),
  upgradeSettingsPluginFamilyLatest: vi.fn(),
  switchSettingsPluginFamilyVersion: vi.fn(),
  deleteSettingsPluginFamily: vi.fn(),
  installSettingsPluginCurrentNodeArtifact: vi.fn(),
  refreshSettingsPluginCurrentNodeArtifact: vi.fn(),
  fetchSettingsPluginTask: vi.fn()
}));

const systemRuntimeApi = vi.hoisted(() => ({
  settingsSystemRuntimeQueryKey: ['settings', 'system-runtime'],
  fetchSettingsSystemRuntimeProfile: vi.fn()
}));

const fileManagementApi = vi.hoisted(() => ({
  settingsFileStoragesQueryKey: ['settings', 'files', 'storages'],
  settingsFileTablesQueryKey: ['settings', 'files', 'tables'],
  fetchSettingsFileStorages: vi.fn(),
  createSettingsFileStorage: vi.fn(),
  fetchSettingsFileTables: vi.fn(),
  createSettingsFileTable: vi.fn(),
  updateSettingsFileTableBinding: vi.fn()
}));

const consoleNavigationApi = vi.hoisted(() => ({
  settingsConsoleNavigationQueryKey: ['settings', 'console-navigation'],
  fetchSettingsConsoleNavigation: vi.fn()
}));

vi.mock('../../api/members', () => membersApi);
vi.mock('../../api/roles', () => rolesApi);
vi.mock('../../api/permissions', () => permissionsApi);
vi.mock('../../api/api-docs', () => docsApi);
vi.mock('../../api/model-providers', () => modelProvidersApi);
vi.mock('../../api/plugins', () => pluginsApi);
vi.mock('../../api/system-runtime', () => systemRuntimeApi);
vi.mock('../../api/file-management', () => fileManagementApi);
vi.mock('../../api/console-navigation', () => consoleNavigationApi);
vi.mock('@scalar/api-reference-react', () => ({
  ApiReferenceReact: () => <div data-testid="settings-page-scalar">Scalar</div>
}));

import { AppProviders } from '../../../../app/AppProviders';
import { AppRouterProvider } from '../../../../app/router';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import {
  buildMainInstanceSettings,
  buildSettingsModelProviderInstances,
  buildSettingsModelProviderOptions
} from '../model-provider-test-fixtures';
const useBreakpointSpy = vi.spyOn(Grid, 'useBreakpoint');

function authenticateWithPermissions(
  permissions: string[],
  effectiveDisplayRole: 'member' | 'root' = 'member'
) {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'user-1',
      account: effectiveDisplayRole,
      effective_display_role: effectiveDisplayRole,
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'user-1',
      account: effectiveDisplayRole,
      email: `${effectiveDisplayRole}@example.com`,
      phone: null,
      nickname: effectiveDisplayRole,
      name: effectiveDisplayRole,
      avatar_url: null,
      introduction: '',
      effective_display_role: effectiveDisplayRole,
      permissions
    }
  });
}

function authenticateAsModelProviderManager() {
  authenticateWithPermissions([
    'state_model.view.all',
    'state_model.manage.all'
  ]);
}

function renderApp(pathname: string) {
  window.history.pushState({}, '', pathname);

  return render(
    <AppProviders>
      <AppRouterProvider />
    </AppProviders>
  );
}

describe('ModelProvidersPage - layout and style', () => {
  beforeEach(() => {
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue({
      route_definitions: [
        {
          route_id: 'settings.model-providers',
          surface_key: 'model-providers',
          path: '/settings/model-providers',
          surface_kind: 'system'
        }
      ],
      navigation_items: [
        {
          item_id: 'model-providers',
          route_id: 'settings.model-providers',
          parent_item_id: 'settings',
          label_key: 'auto.model_provider',
          navigation_slot: 'settings',
          order: 1
        }
      ],
      permission_bindings: []
    });
    resetAuthStore();
    useBreakpointSpy.mockReturnValue({
      xs: true,
      sm: true,
      md: true,
      lg: true,
      xl: false,
      xxl: false
    });
    membersApi.fetchSettingsMembers.mockResolvedValue([]);
    rolesApi.fetchSettingsRoles.mockResolvedValue([]);
    rolesApi.fetchSettingsRolePermissions.mockResolvedValue({
      role_code: 'member',
      permission_codes: []
    });
    permissionsApi.fetchSettingsPermissions.mockResolvedValue([]);
    docsApi.fetchSettingsApiDocsCatalog.mockResolvedValue({
      title: '1flowbase API',
      version: '0.1.0',
      categories: []
    });
    docsApi.fetchSettingsApiDocsCategoryOperations.mockResolvedValue({
      id: 'console',
      label: '控制面',
      operations: []
    });
    docsApi.fetchSettingsApiDocsOperationSpec.mockResolvedValue({
      openapi: '3.1.0',
      info: { title: '1flowbase API', version: '0.1.0' },
      paths: {},
      components: {}
    });
    modelProvidersApi.fetchSettingsModelProviderCatalog.mockResolvedValue(
      modelProviderCatalogEntries
    );
    modelProvidersApi.fetchSettingsModelProviderInstances.mockResolvedValue(
      buildSettingsModelProviderInstances()
    );
    modelProvidersApi.fetchSettingsModelProviderOptions.mockResolvedValue(
      buildSettingsModelProviderOptions()
    );
    modelProvidersApi.fetchSettingsModelProviderMainInstance.mockResolvedValue(
      buildMainInstanceSettings()
    );
    modelProvidersApi.updateSettingsModelProviderMainInstance.mockResolvedValue(
      buildMainInstanceSettings(false)
    );
    modelProvidersApi.previewSettingsModelProviderModels.mockResolvedValue({
      models: [
        {
          model_id: 'gpt-4o-mini',
          display_name: 'gpt-4o-mini',
          source: 'dynamic',
          supports_streaming: true,
          supports_tool_call: true,
          supports_multimodal: false,
          context_window: null,
          max_output_tokens: null,
          parameter_form: null,
          provider_metadata: {}
        }
      ],
      preview_token: 'preview-1',
      expires_at: '2026-04-22T12:00:00Z'
    });
    modelProvidersApi.fetchSettingsModelProviderModels.mockResolvedValue({
      provider_instance_id: 'provider-1',
      refresh_status: 'ready',
      source: 'hybrid',
      last_error_message: null,
      refreshed_at: '2026-04-18T10:01:00Z',
      models: primaryContractProviderModels
    });
    modelProvidersApi.revealSettingsModelProviderSecret.mockResolvedValue({
      key: 'api_key',
      value: 'super-secret'
    });
    pluginsApi.fetchSettingsPluginFamilies.mockResolvedValue([
      {
        provider_code: 'openai_compatible',
        display_name: 'OpenAI Compatible',
        protocol: 'openai_compatible',
        help_url: 'https://platform.openai.com/docs/api-reference',
        default_base_url: 'https://api.openai.com/v1',
        model_discovery_mode: 'hybrid',
        current_installation_id: 'installation-1',
        current_version: '0.1.0',
        current_local_artifact: {
          node_id: 'test-node',
          installation_id: 'installation-1',
          local_version: '0.1.0',
          local_checksum: null,
          installed_path: '/tmp/plugins/openai_compatible/0.1.0',
          artifact_status: 'ready',
          runtime_status: 'inactive',
          checked_at: '2026-04-18T10:00:00Z',
          last_error: null
        },
        latest_version: '0.2.0',
        has_update: true,
        installed_versions: [
          {
            installation_id: 'installation-2',
            plugin_version: '0.2.0',
            source_kind: 'official_registry',
            trust_level: 'verified_official',
            created_at: '2026-04-19T09:00:00Z',
            is_current: false
          },
          {
            installation_id: 'installation-1',
            plugin_version: '0.1.0',
            source_kind: 'official_registry',
            trust_level: 'verified_official',
            created_at: '2026-04-18T09:00:00Z',
            is_current: true
          }
        ]
      }
    ]);
    pluginsApi.fetchSettingsOfficialPluginCatalog.mockResolvedValue({
      locale_meta: { resolved_locale: 'zh_Hans', fallback_locale: 'en_US' },
      page: { limit: 20, next_cursor: null },
      entries: []
    });
    pluginsApi.installSettingsOfficialPlugin.mockResolvedValue({
      installation: {
        id: 'installation-1',
        provider_code: 'openai_compatible',
        plugin_id: 'openai_compatible@0.1.0',
        plugin_version: '0.1.0',
        contract_version: '1flowbase.provider/v1',
        protocol: 'openai_compatible',
        display_name: 'OpenAI Compatible',
        source_kind: 'official_registry',
        trust_level: 'verified_official',
        verification_status: 'valid',
        enabled: true,
        install_path: '/tmp/openai-compatible',
        checksum: 'sha256:abc123',
        signature_status: 'unsigned',
        signature_algorithm: null,
        signing_key_id: null,
        metadata_json: {},
        created_at: '2026-04-18T21:00:00Z',
        updated_at: '2026-04-18T21:00:00Z'
      },
      task: {
        id: 'task-1',
        installation_id: 'installation-1',
        workspace_id: 'workspace-1',
        provider_code: 'openai_compatible',
        task_kind: 'assign',
        status: 'success',
        status_message: 'assigned',
        detail_json: {},
        created_at: '2026-04-18T21:00:00Z',
        updated_at: '2026-04-18T21:00:00Z',
        finished_at: '2026-04-18T21:00:00Z'
      }
    });
    pluginsApi.upgradeSettingsPluginFamilyLatest.mockResolvedValue({
      id: 'task-upgrade',
      installation_id: 'installation-2',
      workspace_id: 'workspace-1',
      provider_code: 'openai_compatible',
      task_kind: 'switch_version',
      status: 'success',
      status_message: 'switched',
      detail_json: {
        previous_installation_id: 'installation-1',
        previous_version: '0.1.0',
        target_installation_id: 'installation-2',
        target_version: '0.2.0',
        migrated_instance_count: 2
      },
      created_at: '2026-04-19T10:00:00Z',
      updated_at: '2026-04-19T10:00:00Z',
      finished_at: '2026-04-19T10:00:00Z'
    });
    pluginsApi.switchSettingsPluginFamilyVersion.mockResolvedValue({
      id: 'task-switch',
      installation_id: 'installation-1',
      workspace_id: 'workspace-1',
      provider_code: 'openai_compatible',
      task_kind: 'switch_version',
      status: 'success',
      status_message: 'switched',
      detail_json: {
        previous_installation_id: 'installation-2',
        previous_version: '0.2.0',
        target_installation_id: 'installation-1',
        target_version: '0.1.0',
        migrated_instance_count: 2
      },
      created_at: '2026-04-19T10:05:00Z',
      updated_at: '2026-04-19T10:05:00Z',
      finished_at: '2026-04-19T10:05:00Z'
    });
    pluginsApi.deleteSettingsPluginFamily.mockResolvedValue({
      id: 'task-delete',
      installation_id: 'installation-1',
      workspace_id: 'workspace-1',
      provider_code: 'openai_compatible',
      task_kind: 'uninstall',
      status: 'success',
      status_message: 'deleted',
      detail_json: {
        deleted_instance_count: 2,
        deleted_installation_count: 1
      },
      created_at: '2026-04-19T10:10:00Z',
      updated_at: '2026-04-19T10:10:00Z',
      finished_at: '2026-04-19T10:10:00Z'
    });
    pluginsApi.fetchSettingsPluginTask.mockResolvedValue({
      id: 'task-1',
      installation_id: 'installation-1',
      workspace_id: 'workspace-1',
      provider_code: 'openai_compatible',
      task_kind: 'assign',
      status: 'success',
      status_message: 'assigned',
      detail_json: {},
      created_at: '2026-04-18T21:00:00Z',
      updated_at: '2026-04-18T21:00:00Z',
      finished_at: '2026-04-18T21:00:00Z'
    });
    systemRuntimeApi.fetchSettingsSystemRuntimeProfile.mockResolvedValue({
      topology: { relationship: 'same_host' },
      hosts: []
    });
    fileManagementApi.fetchSettingsFileStorages.mockResolvedValue([]);
    fileManagementApi.fetchSettingsFileTables.mockResolvedValue([]);
  });

  test('uses explicit viewport section layout without page-private height selectors', async () => {
    authenticateAsModelProviderManager();

    renderApp('/settings/model-providers');

    expect(
      await screen.findByText('OpenAI Compatible', {}, { timeout: 10_000 })
    ).toBeInTheDocument();
    expect(screen.getByTestId('section-page-layout')).toHaveClass(
      'section-page-layout--wide',
      'section-page-layout--viewport'
    );
    expect(screen.getByTestId('section-page-layout')).not.toHaveClass(
      'section-page-layout--full'
    );
  });

  test('keeps fixed-height ownership out of the model provider panel stylesheet', () => {
    const cssSource = fs.readFileSync(
      path.resolve(
        import.meta.dirname,
        '../../components/model-providers/model-provider-panel.css'
      ),
      'utf8'
    );

    expect(cssSource).not.toContain('body:has(.model-provider-panel)');
    expect(cssSource).not.toContain(
      '.section-page-layout:has(.model-provider-panel)'
    );
    expect(cssSource).not.toContain('overflow: hidden !important');
    expect(cssSource).toContain('height: calc(100% - 24px);');
    expect(cssSource).not.toContain('@media (max-width: 1380px)');
    expect(cssSource).toContain('@media (max-width: 768px)');
  });

  test('keeps model provider tab bodies filling their available height', () => {
    const tabsSource = fs.readFileSync(
      path.resolve(
        import.meta.dirname,
        '../../pages/settings-page/ModelProviderSettingsTabs.tsx'
      ),
      'utf8'
    );
    const tabsCssSource = fs.readFileSync(
      path.resolve(
        import.meta.dirname,
        '../../pages/settings-page/model-provider-settings-tabs.css'
      ),
      'utf8'
    );
    const requestLogsCssSource = fs.readFileSync(
      path.resolve(
        import.meta.dirname,
        '../../components/model-provider-request-logs/model-provider-request-logs-panel.css'
      ),
      'utf8'
    );

    expect(tabsSource).toContain('className="model-provider-settings-tabs"');
    expect(tabsCssSource).toMatch(
      /\.model-provider-settings-tabs > \.ant-tabs-body-holder,[\s\S]*?\.model-provider-settings-tabs \.ant-tabs-body,[\s\S]*?\.model-provider-settings-tabs \.ant-tabs-content\s*\{[\s\S]*?display:\s*flex;[\s\S]*?flex:\s*1 1 auto;[\s\S]*?min-height:\s*0;[\s\S]*?overflow:\s*hidden;/u
    );
    expect(tabsCssSource).toContain(
      '.model-provider-settings-tabs .ant-tabs-tabpane'
    );
    expect(tabsCssSource).toContain('min-height: 0;');
    expect(requestLogsCssSource).toContain('height: 100%;');
    expect(requestLogsCssSource).not.toContain('100dvh');
    expect(requestLogsCssSource).not.toContain('@media (max-height:');
  });

  test('AC-001 constrains provider modal tab panels to the scroll shell height', () => {
    const cssSource = fs.readFileSync(
      path.resolve(
        import.meta.dirname,
        '../../components/model-providers/model-provider-panel.css'
      ),
      'utf8'
    );

    expect(cssSource).toContain(
      '.fixed-height-modal__scroll-body.model-provider-panel__instances-modal'
    );
    expect(cssSource).toContain('.model-provider-panel__instances-tabs');
    expect(cssSource).toContain(
      '.model-provider-panel__instances-tabs .ant-tabs-tabpane'
    );
    expect(cssSource).toContain('overflow-y: auto;');
    expect(cssSource).toContain('min-height: 0;');
    expect(cssSource).toMatch(
      /\.model-provider-panel__instances-tabs \.ant-tabs-tabpane\s*\{[^}]*height:\s*100%;[^}]*overflow-y:\s*auto;/s
    );
  });
});
