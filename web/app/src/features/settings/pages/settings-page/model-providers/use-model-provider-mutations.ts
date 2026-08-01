import { useMutation, type QueryClient } from '@tanstack/react-query';
import type { Dispatch, SetStateAction } from 'react';

import {
  createSettingsModelProviderInstance,
  deleteSettingsModelProviderInstance,
  previewSettingsModelProviderModels,
  refreshSettingsModelProviderModels,
  revealSettingsModelProviderSecret,
  settingsModelProviderCatalogQueryKey,
  settingsModelProviderInstancesQueryKey,
  settingsModelProviderOptionsQueryKey,
  updateSettingsModelProviderInstance,
  updateSettingsModelProviderMainInstance,
  validateSettingsModelProviderInstance,
  type SettingsModelProviderInstance,
  type SettingsModelProviderMainInstance
} from '../../../api/model-providers';
import {
  MODEL_PROVIDER_MAIN_INSTANCE_QUERY_KEY_PREFIX,
  MODEL_PROVIDER_MODELS_QUERY_KEY_PREFIX,
  type ModelProviderDrawerState
} from './shared';

export function useModelProviderMutations({
  csrfToken,
  queryClient,
  setDrawerState
}: {
  csrfToken: string | null;
  queryClient: QueryClient;
  setDrawerState: Dispatch<SetStateAction<ModelProviderDrawerState>>;
}) {
  async function invalidateModelProviderQueries() {
    await Promise.all([
      queryClient.invalidateQueries({
        queryKey: settingsModelProviderCatalogQueryKey
      }),
      queryClient.invalidateQueries({
        queryKey: settingsModelProviderInstancesQueryKey
      }),
      queryClient.invalidateQueries({
        queryKey: settingsModelProviderOptionsQueryKey
      }),
      queryClient.invalidateQueries({
        queryKey: MODEL_PROVIDER_MAIN_INSTANCE_QUERY_KEY_PREFIX
      }),
      queryClient.invalidateQueries({
        queryKey: MODEL_PROVIDER_MODELS_QUERY_KEY_PREFIX
      })
    ]);
  }

  const createMutation = useMutation({
    mutationFn: async (input: {
      installationId: string;
      display_name: string;
      included_in_main: boolean;
      configured_models: Array<{
        model_id: string;
        enabled: boolean;
        context_window_override_tokens: number | null;
        supports_multimodal: boolean;
      }>;
      preview_token?: string;
      config: Record<string, unknown>;
    }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return createSettingsModelProviderInstance(
        {
          installation_id: input.installationId,
          display_name: input.display_name,
          included_in_main: input.included_in_main,
          configured_models: input.configured_models,
          preview_token: input.preview_token,
          config: input.config
        },
        csrfToken
      );
    },
    onSuccess: async () => {
      setDrawerState(null);
      await invalidateModelProviderQueries();
    }
  });

  const updateMutation = useMutation({
    mutationFn: async (input: {
      instanceId: string;
      display_name: string;
      included_in_main: boolean;
      configured_models: Array<{
        model_id: string;
        enabled: boolean;
        context_window_override_tokens: number | null;
        supports_multimodal: boolean;
      }>;
      preview_token?: string;
      config: Record<string, unknown>;
    }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return updateSettingsModelProviderInstance(
        input.instanceId,
        {
          display_name: input.display_name,
          included_in_main: input.included_in_main,
          configured_models: input.configured_models,
          preview_token: input.preview_token,
          config: input.config
        },
        csrfToken
      );
    },
    onSuccess: async () => {
      setDrawerState(null);
      await invalidateModelProviderQueries();
    }
  });

  const updateInstanceInclusionMutation = useMutation({
    mutationFn: async (input: {
      instance: SettingsModelProviderInstance;
      included_in_main: boolean;
    }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return updateSettingsModelProviderInstance(
        input.instance.id,
        {
          display_name: input.instance.display_name,
          included_in_main: input.included_in_main,
          configured_models: input.instance.configured_models,
          config: {}
        },
        csrfToken
      );
    },
    onSuccess: invalidateModelProviderQueries
  });

  const updateMainInstanceSettingsMutation = useMutation({
    mutationFn: async (input: {
      providerCode: string;
      auto_include_new_instances: boolean;
      expected_revision: number;
      model_routing_policies: SettingsModelProviderMainInstance['model_routing_policies'];
    }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return updateSettingsModelProviderMainInstance(
        input.providerCode,
        {
          auto_include_new_instances: input.auto_include_new_instances,
          expected_revision: input.expected_revision,
          model_routing_policies: input.model_routing_policies
        },
        csrfToken
      );
    },
    onSuccess: invalidateModelProviderQueries
  });

  const previewMutation = useMutation({
    mutationFn: async (input: {
      installationId?: string;
      instanceId?: string;
      config: Record<string, unknown>;
    }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return previewSettingsModelProviderModels(
        {
          installation_id: input.installationId,
          instance_id: input.instanceId,
          config: input.config
        },
        csrfToken
      );
    }
  });

  const validateMutation = useMutation({
    mutationFn: async (instanceId: string) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return validateSettingsModelProviderInstance(instanceId, csrfToken);
    },
    onSuccess: invalidateModelProviderQueries
  });

  const refreshMutation = useMutation({
    mutationFn: async (instanceId: string) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return refreshSettingsModelProviderModels(instanceId, csrfToken);
    },
    onSuccess: async () => {
      await invalidateModelProviderQueries();
    }
  });

  const revealSecretMutation = useMutation({
    mutationFn: async (input: { instanceId: string; key: string }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return revealSettingsModelProviderSecret(
        input.instanceId,
        input.key,
        csrfToken
      );
    }
  });

  const deleteMutation = useMutation({
    mutationFn: async (instanceId: string) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return deleteSettingsModelProviderInstance(instanceId, csrfToken);
    },
    onSuccess: invalidateModelProviderQueries
  });

  return {
    createMutation,
    updateMutation,
    updateInstanceInclusionMutation,
    updateMainInstanceSettingsMutation,
    previewMutation,
    validateMutation,
    refreshMutation,
    revealSecretMutation,
    deleteMutation
  };
}
