import { useCallback, useMemo, useReducer, type SetStateAction } from 'react';

import { useQueryClient } from '@tanstack/react-query';
import { Alert, Layout } from 'antd';
import type { UploadFile } from 'antd/es/upload/interface';

import { useAuthStore } from '../../../../state/auth-store';
import { ModelProviderCatalogPanel } from '../../components/model-providers/ModelProviderCatalogPanel';
import { ModelProviderInstanceDrawer } from '../../components/model-providers/ModelProviderInstanceDrawer';
import { ModelProviderInstancesModal } from '../../components/model-providers/ModelProviderInstancesModal';
import { OfficialPluginInstallPanel } from '../../components/model-providers/OfficialPluginInstallPanel';
import { PluginUploadInstallModal } from '../../components/model-providers/PluginUploadInstallModal';
import {
  settingsModelProviderCatalogQueryKey,
  settingsModelProviderInstancesQueryKey,
  settingsModelProviderOptionsQueryKey
} from '../../api/model-providers';
import {
  settingsOfficialPluginsQueryKey,
  settingsPluginFamiliesQueryKey
} from '../../api/plugins';
import '../../components/model-providers/model-provider-panel.css';
import {
  getErrorMessage,
  MODEL_PROVIDER_MAIN_INSTANCE_QUERY_KEY_PREFIX,
  MODEL_PROVIDER_MODELS_QUERY_KEY_PREFIX,
  resetUploadState,
  type ModelProviderDrawerState,
  type ModelProviderInstanceModalState,
  type RecentVersionSwitchNotice,
  type UploadResultSummary
} from './model-providers/shared';
import { frontstageBlockCatalogQueryKeyPrefix } from '../../../frontstage/api/block-catalog';
import { useModelProviderData } from './model-providers/use-model-provider-data';
import { useModelProviderMutations } from './model-providers/use-model-provider-mutations';
import { useOfficialPluginTask } from './model-providers/use-official-plugin-task';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import { i18nText } from '../../../../shared/i18n/text';

interface ModelProviderSectionState {
  drawerState: ModelProviderDrawerState;
  instanceModalState: ModelProviderInstanceModalState;
  uploadModalOpen: boolean;
  uploadFileList: UploadFile[];
  uploadValidationMessage: string | null;
  uploadResultSummary: UploadResultSummary;
  recentVersionSwitchNotice: RecentVersionSwitchNotice;
  officialSearchQuery: string;
}

type ModelProviderSectionAction =
  | {
      type: 'setDrawerState';
      value: SetStateAction<ModelProviderDrawerState>;
    }
  | {
      type: 'setInstanceModalState';
      value: SetStateAction<ModelProviderInstanceModalState>;
    }
  | { type: 'setUploadModalOpen'; value: SetStateAction<boolean> }
  | { type: 'setUploadFileList'; value: SetStateAction<UploadFile[]> }
  | {
      type: 'setUploadValidationMessage';
      value: SetStateAction<string | null>;
    }
  | {
      type: 'setUploadResultSummary';
      value: SetStateAction<UploadResultSummary>;
    }
  | {
      type: 'setRecentVersionSwitchNotice';
      value: SetStateAction<RecentVersionSwitchNotice>;
    }
  | { type: 'setOfficialSearchQuery'; value: SetStateAction<string> };

const initialModelProviderSectionState: ModelProviderSectionState = {
  drawerState: null,
  instanceModalState: null,
  uploadModalOpen: false,
  uploadFileList: [],
  uploadValidationMessage: null,
  uploadResultSummary: null,
  recentVersionSwitchNotice: null,
  officialSearchQuery: ''
};

function resolveSetState<T>(value: SetStateAction<T>, current: T): T {
  return typeof value === 'function'
    ? (value as (previous: T) => T)(current)
    : value;
}

function modelProviderSectionReducer(
  state: ModelProviderSectionState,
  action: ModelProviderSectionAction
): ModelProviderSectionState {
  switch (action.type) {
    case 'setDrawerState':
      return {
        ...state,
        drawerState: resolveSetState(action.value, state.drawerState)
      };
    case 'setInstanceModalState':
      return {
        ...state,
        instanceModalState: resolveSetState(
          action.value,
          state.instanceModalState
        )
      };
    case 'setUploadModalOpen':
      return {
        ...state,
        uploadModalOpen: resolveSetState(action.value, state.uploadModalOpen)
      };
    case 'setUploadFileList':
      return {
        ...state,
        uploadFileList: resolveSetState(action.value, state.uploadFileList)
      };
    case 'setUploadValidationMessage':
      return {
        ...state,
        uploadValidationMessage: resolveSetState(
          action.value,
          state.uploadValidationMessage
        )
      };
    case 'setUploadResultSummary':
      return {
        ...state,
        uploadResultSummary: resolveSetState(
          action.value,
          state.uploadResultSummary
        )
      };
    case 'setRecentVersionSwitchNotice':
      return {
        ...state,
        recentVersionSwitchNotice: resolveSetState(
          action.value,
          state.recentVersionSwitchNotice
        )
      };
    case 'setOfficialSearchQuery':
      return {
        ...state,
        officialSearchQuery: resolveSetState(
          action.value,
          state.officialSearchQuery
        )
      };
  }

  return state;
}

export function SettingsModelProvidersSection({
  canManage
}: {
  canManage: boolean;
}) {
  const queryClient = useQueryClient();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const [sectionState, dispatchSectionState] = useReducer(
    modelProviderSectionReducer,
    initialModelProviderSectionState
  );
  const {
    drawerState,
    instanceModalState,
    uploadModalOpen,
    uploadFileList,
    uploadValidationMessage,
    uploadResultSummary,
    recentVersionSwitchNotice,
    officialSearchQuery
  } = sectionState;
  const setDrawerState = useCallback(
    (value: SetStateAction<ModelProviderDrawerState>) =>
      dispatchSectionState({ type: 'setDrawerState', value }),
    []
  );
  const setInstanceModalState = useCallback(
    (value: SetStateAction<ModelProviderInstanceModalState>) =>
      dispatchSectionState({ type: 'setInstanceModalState', value }),
    []
  );
  const setUploadModalOpen = useCallback(
    (value: SetStateAction<boolean>) =>
      dispatchSectionState({ type: 'setUploadModalOpen', value }),
    []
  );
  const setUploadFileList = useCallback(
    (value: SetStateAction<UploadFile[]>) =>
      dispatchSectionState({ type: 'setUploadFileList', value }),
    []
  );
  const setUploadValidationMessage = useCallback(
    (value: SetStateAction<string | null>) =>
      dispatchSectionState({ type: 'setUploadValidationMessage', value }),
    []
  );
  const setUploadResultSummary = useCallback(
    (value: SetStateAction<UploadResultSummary>) =>
      dispatchSectionState({ type: 'setUploadResultSummary', value }),
    []
  );
  const setRecentVersionSwitchNotice = useCallback(
    (value: SetStateAction<RecentVersionSwitchNotice>) =>
      dispatchSectionState({ type: 'setRecentVersionSwitchNotice', value }),
    []
  );
  const setOfficialSearchQuery = useCallback(
    (value: SetStateAction<string>) =>
      dispatchSectionState({ type: 'setOfficialSearchQuery', value }),
    []
  );
  const clearUploadState = () => {
    resetUploadState(
      setUploadFileList,
      setUploadValidationMessage,
      setUploadResultSummary
    );
  };

  const handleOfficialInstallSettled = async (status: 'success' | 'failed') => {
    if (status !== 'success') {
      return;
    }

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
      }),
      queryClient.invalidateQueries({
        queryKey: frontstageBlockCatalogQueryKeyPrefix
      })
    ]);
    queryClient.removeQueries({
      queryKey: frontstageBlockCatalogQueryKeyPrefix,
      type: 'inactive'
    });
  };
  const { officialInstallState, setOfficialInstallState, pluginTaskQuery } =
    useOfficialPluginTask({
      onSettled: handleOfficialInstallSettled
    });
  const {
    catalogQuery,
    familiesQuery,
    officialCatalogQuery,
    instancesQuery,
    optionsQuery,
    mainInstanceQuery,
    families,
    officialCatalogEntries,
    officialSourceMeta,
    currentCatalogEntriesByProviderCode,
    familiesByProviderCode,
    editingInstance,
    editingModelCatalog,
    drawerCatalogEntry,
    drawerDefaultIncludedInMain,
    modalInstances,
    modalCatalogEntry,
    modalProviderOption,
    overviewRows
  } = useModelProviderData({
    drawerState,
    instanceModalState,
    officialSearchQuery
  });
  const {
    createMutation,
    updateMutation,
    updateInstanceInclusionMutation,
    updateMainInstanceSettingsMutation,
    previewMutation,
    validateMutation,
    refreshMutation,
    revealSecretMutation,
    deleteMutation,
    officialInstallMutation,
    uploadMutation,
    versionMutation
  } = useModelProviderMutations({
    csrfToken,
    queryClient,
    setDrawerState,
    setInstanceModalState,
    setOfficialInstallState,
    setUploadValidationMessage,
    setUploadResultSummary,
    setRecentVersionSwitchNotice
  });

  const errorMessage =
    getErrorMessage(catalogQuery.error) ??
    getErrorMessage(familiesQuery.error) ??
    getErrorMessage(instancesQuery.error) ??
    getErrorMessage(optionsQuery.error) ??
    getErrorMessage(mainInstanceQuery.error) ??
    getErrorMessage(createMutation.error) ??
    getErrorMessage(updateMutation.error) ??
    getErrorMessage(updateInstanceInclusionMutation.error) ??
    getErrorMessage(updateMainInstanceSettingsMutation.error) ??
    getErrorMessage(previewMutation.error) ??
    getErrorMessage(revealSecretMutation.error) ??
    getErrorMessage(validateMutation.error) ??
    getErrorMessage(refreshMutation.error) ??
    getErrorMessage(deleteMutation.error) ??
    getErrorMessage(officialInstallMutation.error) ??
    getErrorMessage(versionMutation.error) ??
    getErrorMessage(pluginTaskQuery.error);
  const uploadErrorMessage =
    uploadValidationMessage ?? getErrorMessage(uploadMutation.error);
  const sectionStatus = useMemo(
    () =>
      errorMessage ? (
        <Alert type="error" showIcon message={errorMessage} />
      ) : null,
    [errorMessage]
  );
  const officialCatalogErrorMessage = officialCatalogQuery.error
    ? i18nText('settings', 'auto.official_supplier_catalog_unavailable')
    : null;
  const modalMainInstance =
    mainInstanceQuery.data ??
    (modalProviderOption
      ? {
          provider_code: modalProviderOption.provider_code,
          auto_include_new_instances:
            modalProviderOption.main_instance.auto_include_new_instances,
          revision: 0,
          model_routing_policies: modalProviderOption.model_groups.map(
            (group) => ({
              model_id: group.model_id,
              distribution_rule: group.distribution_rule,
              provider_instance_ids: group.targets.map(
                (target) => target.source_instance_id
              ),
              excluded_provider_instance_ids: group.targets
                .filter((target) => !target.routing_enabled)
                .map((target) => target.source_instance_id)
            })
          )
        }
      : null);

  return (
    <>
      <SettingsSectionSurface heightMode="fill" status={sectionStatus}>
        <div className="model-provider-panel">
          <Layout className="model-provider-panel__main">
            <Layout.Content className="model-provider-panel__left">
              <ModelProviderCatalogPanel
                overviewRows={overviewRows}
                entries={families}
                currentCatalogEntries={currentCatalogEntriesByProviderCode}
                loading={familiesQuery.isLoading}
                canManage={canManage}
                switchingProviderCode={
                  versionMutation.isPending &&
                  versionMutation.variables.mode === 'switch'
                    ? versionMutation.variables.providerCode
                    : null
                }
                onViewInstances={(entry) => {
                  setInstanceModalState({
                    providerCode: entry.provider_code,
                    displayName: entry.display_name
                  });
                }}
                onCreate={(entry) => {
                  setDrawerState({
                    mode: 'create',
                    providerCode: entry.provider_code
                  });
                }}
                onSwitchVersion={(entry, installationId) => {
                  versionMutation.mutate({
                    mode: 'switch',
                    providerCode: entry.provider_code,
                    installationId
                  });
                }}
              />
            </Layout.Content>

            <Layout.Sider
              width={360}
              theme="light"
              className="model-provider-panel__sidebar"
            >
              <OfficialPluginInstallPanel
                sourceMeta={officialSourceMeta}
                entries={officialCatalogEntries}
                catalogErrorMessage={officialCatalogErrorMessage}
                familiesByProviderCode={familiesByProviderCode}
                searchQuery={officialSearchQuery}
                loading={officialCatalogQuery.isLoading}
                canManage={canManage}
                activePluginId={officialInstallState.pluginId}
                installState={officialInstallState.status}
                upgradingProviderCode={
                  versionMutation.isPending &&
                  versionMutation.variables?.mode === 'upgrade'
                    ? (versionMutation.variables.providerCode ?? null)
                    : null
                }
                onInstall={(entry, compatibilityOverride) => {
                  officialInstallMutation.mutate({
                    pluginId: entry.plugin_id,
                    compatibilityOverride
                  });
                }}
                onOpenUpload={() => {
                  setUploadModalOpen(true);
                  clearUploadState();
                }}
                onSearchQueryChange={setOfficialSearchQuery}
                onRetryCatalog={() => {
                  void officialCatalogQuery.refetch();
                }}
                onUpgradeLatest={(entry, compatibilityOverride) => {
                  versionMutation.mutate({
                    mode: 'upgrade',
                    providerCode: entry.provider_code,
                    compatibilityOverride
                  });
                }}
              />
            </Layout.Sider>
          </Layout>
        </div>
      </SettingsSectionSurface>

      <ModelProviderInstanceDrawer
        open={drawerState !== null}
        mode={drawerState?.mode ?? 'create'}
        catalogEntry={drawerCatalogEntry}
        instance={editingInstance}
        cachedModelCatalog={editingModelCatalog}
        defaultIncludedInMain={drawerDefaultIncludedInMain}
        submitting={createMutation.isPending || updateMutation.isPending}
        onClose={() => setDrawerState(null)}
        onRevealSecret={async (fieldKey) => {
          if (!editingInstance) {
            throw new Error('missing provider instance');
          }

          const result = await revealSecretMutation.mutateAsync({
            instanceId: editingInstance.id,
            key: fieldKey
          });

          return typeof result.value === 'string'
            ? result.value
            : JSON.stringify(result.value ?? '');
        }}
        onSubmit={async (values) => {
          if (drawerState?.mode === 'edit' && editingInstance) {
            await updateMutation.mutateAsync({
              instanceId: editingInstance.id,
              display_name: values.display_name,
              included_in_main: values.included_in_main,
              configured_models: values.configured_models,
              preview_token: values.preview_token,
              config: values.config
            });
            return;
          }

          if (!drawerCatalogEntry) {
            throw new Error('missing provider catalog entry');
          }

          await createMutation.mutateAsync({
            installationId: drawerCatalogEntry.installation_id,
            display_name: values.display_name,
            included_in_main: values.included_in_main,
            configured_models: values.configured_models,
            preview_token: values.preview_token,
            config: values.config
          });
        }}
        onPreviewModels={async (config) => {
          if (drawerState?.mode === 'edit' && editingInstance) {
            return previewMutation.mutateAsync({
              instanceId: editingInstance.id,
              config
            });
          }

          if (!drawerCatalogEntry) {
            throw new Error('missing provider catalog entry');
          }

          return previewMutation.mutateAsync({
            installationId: drawerCatalogEntry.installation_id,
            config
          });
        }}
      />

      <ModelProviderInstancesModal
        open={instanceModalState !== null}
        catalogEntry={modalCatalogEntry}
        providerDisplayName={instanceModalState?.displayName ?? null}
        mainInstance={modalMainInstance}
        modelGroups={modalProviderOption?.model_groups ?? []}
        instances={modalInstances}
        updatingMainInstance={
          updateMainInstanceSettingsMutation.isPending ||
          mainInstanceQuery.isFetching
        }
        updatingInstanceId={
          updateInstanceInclusionMutation.isPending
            ? (updateInstanceInclusionMutation.variables?.instance.id ?? null)
            : null
        }
        refreshingCandidates={validateMutation.isPending}
        refreshing={refreshMutation.isPending}
        deleting={deleteMutation.isPending}
        canManage={canManage}
        versionSwitchNotice={
          instanceModalState &&
          recentVersionSwitchNotice?.providerCode ===
            instanceModalState.providerCode
            ? {
                targetVersion: recentVersionSwitchNotice.targetVersion,
                migratedInstanceCount:
                  recentVersionSwitchNotice.migratedInstanceCount
              }
            : null
        }
        onClose={() => {
          setInstanceModalState(null);
          setRecentVersionSwitchNotice((current) =>
            current && current.providerCode === instanceModalState?.providerCode
              ? null
              : current
          );
        }}
        onEdit={(instance) => {
          setDrawerState({
            mode: 'edit',
            instanceId: instance.id
          });
        }}
        onRefreshCandidates={(instance) => {
          validateMutation.mutate(instance.id);
        }}
        onRefreshModels={(instance) => {
          refreshMutation.mutate(instance.id);
        }}
        onDelete={(instance) => {
          deleteMutation.mutate(instance.id);
        }}
        onToggleAutoIncludeNewInstances={(checked) => {
          if (!instanceModalState || !modalMainInstance) {
            return;
          }

          updateMainInstanceSettingsMutation.mutate({
            providerCode: instanceModalState.providerCode,
            auto_include_new_instances: checked,
            expected_revision: modalMainInstance.revision,
            model_routing_policies: modalMainInstance.model_routing_policies
          });
        }}
        onChangeDistributionRule={(modelId, distributionRule) => {
          if (!instanceModalState || !modalMainInstance) {
            return;
          }

          const existingPolicy = modalMainInstance.model_routing_policies.find(
            (policy) => policy.model_id === modelId
          );
          const nextPolicy = existingPolicy
            ? { ...existingPolicy, distribution_rule: distributionRule }
            : {
                model_id: modelId,
                distribution_rule: distributionRule,
                provider_instance_ids:
                  modalProviderOption?.model_groups
                    .find((group) => group.model_id === modelId)
                    ?.targets.map((target) => target.source_instance_id) ?? [],
                excluded_provider_instance_ids:
                  modalProviderOption?.model_groups
                    .find((group) => group.model_id === modelId)
                    ?.targets.filter((target) => !target.routing_enabled)
                    .map((target) => target.source_instance_id) ?? []
              };

          updateMainInstanceSettingsMutation.mutate({
            providerCode: instanceModalState.providerCode,
            auto_include_new_instances:
              modalMainInstance.auto_include_new_instances,
            expected_revision: modalMainInstance.revision,
            model_routing_policies: existingPolicy
              ? modalMainInstance.model_routing_policies.map((policy) =>
                  policy.model_id === modelId ? nextPolicy : policy
                )
              : [...modalMainInstance.model_routing_policies, nextPolicy]
          });
        }}
        onSaveRoutingPolicy={(
          modelId,
          distributionRule,
          providerInstanceIds,
          excludedProviderInstanceIds,
          onSuccess
        ) => {
          if (!instanceModalState || !modalMainInstance) {
            return;
          }
          const existingPolicy = modalMainInstance.model_routing_policies.find(
            (policy) => policy.model_id === modelId
          );
          const nextPolicy = {
            model_id: modelId,
            distribution_rule: distributionRule,
            provider_instance_ids: providerInstanceIds,
            excluded_provider_instance_ids: excludedProviderInstanceIds
          };
          updateMainInstanceSettingsMutation.mutate(
            {
              providerCode: instanceModalState.providerCode,
              auto_include_new_instances:
                modalMainInstance.auto_include_new_instances,
              expected_revision: modalMainInstance.revision,
              model_routing_policies: existingPolicy
                ? modalMainInstance.model_routing_policies.map((policy) =>
                    policy.model_id === modelId ? nextPolicy : policy
                  )
                : [...modalMainInstance.model_routing_policies, nextPolicy]
            },
            { onSuccess }
          );
        }}
        onToggleIncludedInMain={(instance, checked) => {
          updateInstanceInclusionMutation.mutate({
            instance,
            included_in_main: checked
          });
        }}
      />

      <PluginUploadInstallModal
        open={uploadModalOpen}
        submitting={uploadMutation.isPending}
        resultSummary={uploadResultSummary}
        errorMessage={uploadErrorMessage}
        fileList={uploadFileList}
        onClose={() => {
          setUploadModalOpen(false);
          clearUploadState();
        }}
        onChange={(nextFiles) => {
          clearUploadState();
          setUploadFileList(nextFiles.slice(-1));
        }}
        onSubmit={() => {
          const file = uploadFileList[0]?.originFileObj;
          if (!(file instanceof File)) {
            setUploadValidationMessage(
              i18nText('settings', 'auto.select_plug_package_first')
            );
            return;
          }

          uploadMutation.mutate(file);
        }}
      />
    </>
  );
}
