import type { ConsoleModelProviderOptions } from '@1flowbase/api-client';

import {
  modelProviderCatalogEntries,
  primaryContractProviderModels
} from '../../../test/model-provider-contract-fixtures';

export function buildSettingsModelProviderInstances() {
  return [
    {
      id: 'provider-1',
      installation_id: modelProviderCatalogEntries[0].installation_id,
      provider_code: modelProviderCatalogEntries[0].provider_code,
      protocol: modelProviderCatalogEntries[0].protocol,
      display_name: 'OpenAI Production',
      status: 'ready',
      config_json: {
        base_url: 'https://api.openai.com/v1',
        api_key: 'supe****cret'
      },
      included_in_main: true,
      configured_models: [
        {
          model_id: 'gpt-4o-mini',
          enabled: true,
          context_window_override_tokens: null
        },
        {
          model_id: 'gpt-4o',
          enabled: true,
          context_window_override_tokens: null
        }
      ],
      enabled_model_ids: ['gpt-4o-mini', 'gpt-4o'],
      catalog_refresh_status: 'ready',
      catalog_last_error_message: null,
      catalog_refreshed_at: '2026-04-18T10:01:00Z',
      model_count: 1
    },
    {
      id: 'provider-2',
      installation_id: modelProviderCatalogEntries[0].installation_id,
      provider_code: modelProviderCatalogEntries[0].provider_code,
      protocol: modelProviderCatalogEntries[0].protocol,
      display_name: 'OpenAI Backup',
      status: 'ready',
      config_json: {
        base_url: 'https://backup.openai.example/v1',
        api_key: 'back****cret'
      },
      included_in_main: false,
      configured_models: [
        {
          model_id: 'gpt-4.1-mini',
          enabled: true,
          context_window_override_tokens: null
        }
      ],
      enabled_model_ids: ['gpt-4.1-mini'],
      catalog_refresh_status: 'ready',
      catalog_last_error_message: null,
      catalog_refreshed_at: '2026-04-18T09:58:00Z',
      model_count: 1
    }
  ];
}

export function buildSettingsModelProviderOptions(): ConsoleModelProviderOptions {
  return {
    locale_meta: {
      requested_locale: 'zh_Hans',
      resolved_locale: 'zh_Hans',
      user_preferred_locale: 'zh_Hans',
      accept_language: 'zh-Hans-CN,zh;q=0.9,en;q=0.8',
      fallback_locale: 'en_US',
      supported_locales: ['zh_Hans', 'en_US']
    },
    i18n_catalog: {},
    providers: [
      {
        provider_code: modelProviderCatalogEntries[0].provider_code,
        plugin_type: 'model_provider',
        namespace: modelProviderCatalogEntries[0].namespace,
        label_key: modelProviderCatalogEntries[0].label_key,
        description_key: modelProviderCatalogEntries[0].description_key,
        protocol: modelProviderCatalogEntries[0].protocol,
        display_name: modelProviderCatalogEntries[0].display_name,
        main_instance: {
          provider_code: modelProviderCatalogEntries[0].provider_code,
          auto_include_new_instances: true,
          group_count: primaryContractProviderModels.length,
          model_count: primaryContractProviderModels.length
        },
        parameter_form: null,
        model_groups: [
          {
            model_id: primaryContractProviderModels[0].model_id,
            distribution_rule: 'none',
            model: primaryContractProviderModels[0],
            targets: [
              {
                source_instance_id: 'provider-1',
                source_instance_display_name: 'OpenAI Production',
                model: primaryContractProviderModels[0]
              }
            ]
          },
          {
            model_id: primaryContractProviderModels[1].model_id,
            distribution_rule: 'none',
            model: primaryContractProviderModels[1],
            targets: [
              {
                source_instance_id: 'provider-1',
                source_instance_display_name: 'OpenAI Production',
                model: primaryContractProviderModels[1]
              }
            ]
          }
        ]
      }
    ]
  };
}

export function buildMainInstanceSettings(
  autoIncludeNewInstances = true,
  distributionRule: 'none' | 'round_robin' | 'retry_round_robin' = 'none'
) {
  return {
    provider_code: modelProviderCatalogEntries[0].provider_code,
    auto_include_new_instances: autoIncludeNewInstances,
    revision: 1,
    model_routing_policies: primaryContractProviderModels.map((model) => ({
      model_id: model.model_id,
      distribution_rule: distributionRule,
      provider_instance_ids: ['provider-1']
    }))
  };
}
