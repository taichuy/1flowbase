import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

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

const legacyPluginLifecycleApi = vi.hoisted(() => ({
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

vi.mock('../../api/model-providers', () => modelProvidersApi);
vi.mock('../../api/plugins', () => legacyPluginLifecycleApi);

import { AppI18nProvider } from '../../../../app/AppI18nProvider';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import { SettingsModelProvidersSection } from '../../pages/settings-page/SettingsModelProvidersSection';

function renderSection() {
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
  return render(<SettingsModelProvidersSection canManage />, { wrapper });
}

describe('ModelProvidersPage - configuration-only responsibility', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetAuthStore();
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
        permissions: ['state_model.view.all', 'state_model.manage.all']
      }
    });
    modelProvidersApi.fetchSettingsModelProviderCatalog.mockResolvedValue([
      {
        installation_id: 'installation-1',
        provider_code: 'openai_compatible',
        plugin_id: 'openai_compatible@1.0.0',
        plugin_version: '1.0.0',
        plugin_type: 'model_provider',
        namespace: 'official',
        label_key: 'OpenAI Compatible',
        description_key: 'OpenAI API compatible provider',
        display_name: 'OpenAI Compatible',
        protocol: 'openai_compatible',
        help_url: null,
        default_base_url: 'https://api.openai.com/v1',
        model_discovery_mode: 'hybrid',
        supports_model_fetch_without_credentials: false,
        desired_state: 'installed',
        availability_status: 'ready',
        form_schema: [],
        predefined_models: []
      }
    ]);
    modelProvidersApi.fetchSettingsModelProviderInstances.mockResolvedValue([]);
    modelProvidersApi.fetchSettingsModelProviderOptions.mockResolvedValue({
      locale_meta: {},
      i18n_catalog: {},
      providers: []
    });
    modelProvidersApi.fetchSettingsModelProviderMainInstance.mockResolvedValue({
      provider_code: 'openai_compatible',
      auto_include_new_instances: true,
      revision: 1,
      model_routing_policies: []
    });
  });

  test('Root-AC-008 retains instance configuration but mounts no install, upload, update, repair, or version lifecycle', async () => {
    renderSection();

    expect(await screen.findByText('OpenAI Compatible')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '管理' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '新增' })).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '上传插件' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '安装' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '更新' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '修复' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('版本')).not.toBeInTheDocument();

    await waitFor(() => {
      expect(
        modelProvidersApi.fetchSettingsModelProviderCatalog
      ).toHaveBeenCalledTimes(1);
    });
    for (const request of Object.values(legacyPluginLifecycleApi)) {
      expect(request).not.toHaveBeenCalled();
    }
  });
});
