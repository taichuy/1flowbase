import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import type { SettingsModelProviderInstance } from '../../../../api/model-providers';
import { useModelProviderMutations } from '../use-model-provider-mutations';

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
  createSettingsModelProviderInstance: vi.fn(),
  deleteSettingsModelProviderInstance: vi.fn(),
  previewSettingsModelProviderModels: vi.fn(),
  refreshSettingsModelProviderModels: vi.fn(),
  revealSettingsModelProviderSecret: vi.fn(),
  updateSettingsModelProviderInstance: vi.fn(),
  updateSettingsModelProviderMainInstance: vi.fn(),
  validateSettingsModelProviderInstance: vi.fn()
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

function setupMutations(queryClient = createQueryClient()) {
  const stateSetter = vi.fn();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );

  const view = renderHook(
    () =>
      useModelProviderMutations({
        csrfToken: 'csrf-123',
        queryClient,
        setDrawerState: stateSetter
      }),
    { wrapper }
  );

  return view.result;
}

function buildInstance(): SettingsModelProviderInstance {
  return {
    id: 'provider-1',
    installation_id: 'installation-1',
    provider_code: 'openai_compatible',
    protocol: 'openai_compatible',
    display_name: 'OpenAI Production',
    status: 'ready',
    included_in_main: true,
    config_json: {
      base_url: 'https://api.openai.com/v1',
      api_key: 'sk-M****ODA='
    },
    configured_models: [
      {
        model_id: 'gpt-4o-mini',
        enabled: true,
        context_window_override_tokens: null,
        supports_multimodal: true
      }
    ],
    enabled_model_ids: ['gpt-4o-mini'],
    catalog_refresh_status: 'ready',
    catalog_last_error_message: null,
    catalog_refreshed_at: '2026-04-18T10:05:00Z',
    model_count: 1
  };
}

describe('useModelProviderMutations', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    modelProvidersApi.updateSettingsModelProviderInstance.mockResolvedValue(
      buildInstance()
    );
  });

  test('toggling main-instance inclusion does not submit masked secret config', async () => {
    const mutations = setupMutations();

    await act(async () => {
      await mutations.current.updateInstanceInclusionMutation.mutateAsync({
        instance: buildInstance(),
        included_in_main: false
      });
    });

    expect(
      modelProvidersApi.updateSettingsModelProviderInstance
    ).toHaveBeenCalledWith(
      'provider-1',
      {
        display_name: 'OpenAI Production',
        included_in_main: false,
        configured_models: [
          {
            model_id: 'gpt-4o-mini',
            enabled: true,
            context_window_override_tokens: null,
            supports_multimodal: true
          }
        ],
        config: {}
      },
      'csrf-123'
    );
  });

  test('AC-002 sends the complete routing policy and revision when updating main-instance settings', async () => {
    const mutations = setupMutations();
    modelProvidersApi.updateSettingsModelProviderMainInstance.mockResolvedValue(
      {
        provider_code: 'openai_compatible',
        auto_include_new_instances: true,
        revision: 8,
        model_routing_policies: [
          {
            model_id: 'gpt-4o-mini',
            distribution_rule: 'retry_round_robin',
            provider_instance_ids: ['provider-2', 'provider-1'],
            excluded_provider_instance_ids: ['provider-2']
          }
        ]
      }
    );

    await act(async () => {
      await mutations.current.updateMainInstanceSettingsMutation.mutateAsync({
        providerCode: 'openai_compatible',
        auto_include_new_instances: true,
        expected_revision: 7,
        model_routing_policies: [
          {
            model_id: 'gpt-4o-mini',
            distribution_rule: 'retry_round_robin',
            provider_instance_ids: ['provider-2', 'provider-1'],
            excluded_provider_instance_ids: ['provider-2']
          }
        ]
      });
    });

    expect(
      modelProvidersApi.updateSettingsModelProviderMainInstance
    ).toHaveBeenCalledWith(
      'openai_compatible',
      {
        auto_include_new_instances: true,
        expected_revision: 7,
        model_routing_policies: [
          {
            model_id: 'gpt-4o-mini',
            distribution_rule: 'retry_round_robin',
            provider_instance_ids: ['provider-2', 'provider-1'],
            excluded_provider_instance_ids: ['provider-2']
          }
        ]
      },
      'csrf-123'
    );
  });
});
