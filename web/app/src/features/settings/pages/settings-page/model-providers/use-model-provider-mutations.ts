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
  deleteSettingsPluginFamily,
  installSettingsPluginCurrentNodeArtifact,
  installSettingsOfficialPlugin,
  refreshSettingsPluginCurrentNodeArtifact,
  settingsOfficialPluginsQueryKey,
  settingsPluginFamiliesQueryKey,
  switchSettingsPluginFamilyVersion,
  type SettingsPluginCompatibilityOverride,
  upgradeSettingsPluginFamilyLatest,
  uploadSettingsPluginPackage
} from '../../../api/plugins';
import { formatPluginAvailabilityStatus } from '../../../components/model-providers/plugin-installation-status';
import {
  formatTrustLabel,
  isTaskSucceeded,
  isTaskTerminal,
  MODEL_PROVIDER_MAIN_INSTANCE_QUERY_KEY_PREFIX,
  MODEL_PROVIDER_MODELS_QUERY_KEY_PREFIX,
  type ModelProviderDrawerState,
  type ModelProviderInstanceModalState,
  type OfficialInstallState,
  type RecentVersionSwitchNotice,
  type UploadResultSummary
} from './shared';
import { i18nText } from '../../../../../shared/i18n/text';

export function useModelProviderMutations({
  csrfToken,
  queryClient,
  setDrawerState,
  setInstanceModalState,
  setOfficialInstallState,
  setUploadValidationMessage,
  setUploadResultSummary,
  setRecentVersionSwitchNotice
}: {
  csrfToken: string | null;
  queryClient: QueryClient;
  setDrawerState: Dispatch<SetStateAction<ModelProviderDrawerState>>;
  setInstanceModalState: Dispatch<
    SetStateAction<ModelProviderInstanceModalState>
  >;
  setOfficialInstallState: Dispatch<SetStateAction<OfficialInstallState>>;
  setUploadValidationMessage: Dispatch<SetStateAction<string | null>>;
  setUploadResultSummary: Dispatch<SetStateAction<UploadResultSummary>>;
  setRecentVersionSwitchNotice: Dispatch<
    SetStateAction<RecentVersionSwitchNotice>
  >;
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
        queryKey: settingsPluginFamiliesQueryKey
      }),
      queryClient.invalidateQueries({
        queryKey: settingsModelProviderOptionsQueryKey
      }),
      queryClient.invalidateQueries({
        queryKey: MODEL_PROVIDER_MAIN_INSTANCE_QUERY_KEY_PREFIX
      }),
      queryClient.invalidateQueries({
        queryKey: MODEL_PROVIDER_MODELS_QUERY_KEY_PREFIX
      }),
      queryClient.invalidateQueries({
        queryKey: settingsOfficialPluginsQueryKey
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
      model_distribution_rules: SettingsModelProviderMainInstance['model_distribution_rules'];
    }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return updateSettingsModelProviderMainInstance(
        input.providerCode,
        {
          auto_include_new_instances: input.auto_include_new_instances,
          model_distribution_rules: input.model_distribution_rules
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

  const familyDeleteMutation = useMutation({
    mutationFn: async (providerCode: string) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return deleteSettingsPluginFamily(providerCode, csrfToken);
    },
    onSuccess: async () => {
      setDrawerState(null);
      setInstanceModalState(null);
      await invalidateModelProviderQueries();
    }
  });

  const officialInstallMutation = useMutation({
    mutationFn: async (input: {
      pluginId: string;
      compatibilityOverride?: SettingsPluginCompatibilityOverride;
    }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      if (input.compatibilityOverride) {
        return installSettingsOfficialPlugin(
          input.pluginId,
          csrfToken,
          input.compatibilityOverride
        );
      }

      return installSettingsOfficialPlugin(input.pluginId, csrfToken);
    },
    onMutate: (input) => {
      setOfficialInstallState({
        pluginId: input.pluginId,
        taskId: null,
        status: 'installing'
      });
    },
    onSuccess: async (result, input) => {
      if (result.task.finished_at || isTaskTerminal(result.task.status)) {
        const status = isTaskSucceeded(result.task.status)
          ? 'success'
          : 'failed';
        setOfficialInstallState({
          pluginId: input.pluginId,
          taskId: null,
          status
        });
        if (status === 'success') {
          await invalidateModelProviderQueries();
        }
        return;
      }

      setOfficialInstallState({
        pluginId: input.pluginId,
        taskId: result.task.id,
        status: 'installing'
      });
    },
    onError: (_error, input) => {
      setOfficialInstallState({
        pluginId: input.pluginId,
        taskId: null,
        status: 'failed'
      });
    }
  });

  const uploadMutation = useMutation({
    mutationFn: async (file: File) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return uploadSettingsPluginPackage(file, csrfToken);
    },
    onSuccess: async (result) => {
      setUploadValidationMessage(null);
      setUploadResultSummary({
        displayName: result.installation.display_name,
        version: result.installation.plugin_version,
        trustLabel: formatTrustLabel(result.installation.trust_level),
        availabilityLabel: formatPluginAvailabilityStatus(
          result.installation.availability_status
        ).label
      });
      await invalidateModelProviderQueries();
    }
  });

  const refreshCurrentNodeArtifactMutation = useMutation({
    mutationFn: async (installationId: string) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return refreshSettingsPluginCurrentNodeArtifact(
        installationId,
        csrfToken
      );
    },
    onSuccess: invalidateModelProviderQueries
  });

  const installCurrentNodeArtifactMutation = useMutation({
    mutationFn: async (installationId: string) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return installSettingsPluginCurrentNodeArtifact(
        installationId,
        csrfToken
      );
    },
    onSuccess: invalidateModelProviderQueries
  });

  const versionMutation = useMutation({
    mutationFn: async (
      input:
        | {
            mode: 'upgrade';
            providerCode: string;
            compatibilityOverride?: SettingsPluginCompatibilityOverride;
          }
        | { mode: 'switch'; providerCode: string; installationId: string }
    ) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      const task =
        input.mode === 'upgrade'
          ? input.compatibilityOverride
            ? upgradeSettingsPluginFamilyLatest(
                input.providerCode,
                csrfToken,
                input.compatibilityOverride
              )
            : upgradeSettingsPluginFamilyLatest(input.providerCode, csrfToken)
          : switchSettingsPluginFamilyVersion(
              input.providerCode,
              input.installationId,
              csrfToken
            );

      const resolvedTask = await task;
      if (
        isTaskTerminal(resolvedTask.status) &&
        !isTaskSucceeded(resolvedTask.status)
      ) {
        throw new Error(
          resolvedTask.status_message ??
            i18nText('settings', 'auto.version_switching_failed')
        );
      }

      return resolvedTask;
    },
    onSuccess: async (task, variables) => {
      const detail = task.detail_json ?? {};

      setRecentVersionSwitchNotice({
        providerCode: variables.providerCode,
        targetVersion:
          typeof detail.target_version === 'string'
            ? detail.target_version
            : null,
        migratedInstanceCount:
          typeof detail.migrated_instance_count === 'number'
            ? detail.migrated_instance_count
            : null
      });
      await invalidateModelProviderQueries();
    }
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
    deleteMutation,
    familyDeleteMutation,
    officialInstallMutation,
    uploadMutation,
    refreshCurrentNodeArtifactMutation,
    installCurrentNodeArtifactMutation,
    versionMutation
  };
}
