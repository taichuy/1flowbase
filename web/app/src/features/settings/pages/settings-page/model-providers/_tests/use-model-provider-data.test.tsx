import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { useModelProviderData } from '../use-model-provider-data';

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
  fetchSettingsModelProviderMainInstance: vi.fn(),
  fetchSettingsModelProviderModels: vi.fn(),
  fetchSettingsModelProviderOptions: vi.fn()
}));

vi.mock('../../../../api/model-providers', () => modelProvidersApi);

function createQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false }
    }
  });
}

function renderUseModelProviderData() {
  const queryClient = createQueryClient();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );

  return renderHook(
    () =>
      useModelProviderData({
        drawerState: {
          mode: 'create',
          providerCode: 'openai_compatible'
        },
        instanceModalState: null
      }),
    { wrapper }
  );
}

describe('useModelProviderData', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    modelProvidersApi.fetchSettingsModelProviderCatalog.mockResolvedValue([
      {
        installation_id: 'installation-1',
        provider_code: 'openai_compatible',
        plugin_id: 'openai_compatible@0.1.0',
        plugin_version: '0.1.0',
        plugin_type: 'model_provider',
        namespace: 'official',
        label_key: 'OpenAI Compatible',
        description_key: null,
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
    modelProvidersApi.fetchSettingsModelProviderModels.mockResolvedValue({
      provider_instance_id: 'provider-1',
      refresh_status: 'ready',
      source: 'hybrid',
      last_error_message: null,
      refreshed_at: null,
      models: []
    });
    modelProvidersApi.fetchSettingsModelProviderOptions.mockResolvedValue({
      locale_meta: {},
      i18n_catalog: {},
      providers: []
    });
    modelProvidersApi.fetchSettingsModelProviderMainInstance.mockResolvedValue({
      provider_code: 'openai_compatible',
      auto_include_new_instances: true,
      revision: 0,
      model_routing_policies: []
    });
  });

  test('uses main-instance settings as the create drawer inclusion default when provider options are empty', async () => {
    const view = renderUseModelProviderData();

    await waitFor(() => {
      expect(view.result.current.drawerDefaultIncludedInMain).toBe(true);
    });
    expect(
      modelProvidersApi.fetchSettingsModelProviderMainInstance
    ).toHaveBeenCalledWith('openai_compatible');
    expect(
      modelProvidersApi.fetchSettingsModelProviderCatalog
    ).toHaveBeenCalledWith('zh_Hans');
    expect(
      modelProvidersApi.fetchSettingsModelProviderCatalog
    ).toHaveBeenCalledTimes(1);
  });
});
