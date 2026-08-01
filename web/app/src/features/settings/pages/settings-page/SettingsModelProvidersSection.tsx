import { useMemo, useState } from 'react';

import { Alert, Layout } from 'antd';
import { useQueryClient } from '@tanstack/react-query';

import { useAuthStore } from '../../../../state/auth-store';
import { ModelProviderCatalogPanel } from '../../components/model-providers/ModelProviderCatalogPanel';
import { ModelProviderInstanceDrawer } from '../../components/model-providers/ModelProviderInstanceDrawer';
import { ModelProviderInstancesModal } from '../../components/model-providers/ModelProviderInstancesModal';
import '../../components/model-providers/model-provider-panel.css';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import {
  getErrorMessage,
  type ModelProviderDrawerState,
  type ModelProviderInstanceModalState
} from './model-providers/shared';
import { useModelProviderData } from './model-providers/use-model-provider-data';
import { useModelProviderMutations } from './model-providers/use-model-provider-mutations';

export function SettingsModelProvidersSection({
  canManage
}: {
  canManage: boolean;
}) {
  const queryClient = useQueryClient();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const [drawerState, setDrawerState] =
    useState<ModelProviderDrawerState>(null);
  const [instanceModalState, setInstanceModalState] =
    useState<ModelProviderInstanceModalState>(null);
  const {
    catalogQuery,
    instancesQuery,
    optionsQuery,
    mainInstanceQuery,
    catalogEntries,
    editingInstance,
    editingModelCatalog,
    drawerCatalogEntry,
    drawerDefaultIncludedInMain,
    modalInstances,
    modalCatalogEntry,
    modalProviderOption,
    overviewRows
  } = useModelProviderData({ drawerState, instanceModalState });
  const {
    createMutation,
    updateMutation,
    updateInstanceInclusionMutation,
    updateMainInstanceSettingsMutation,
    previewMutation,
    validateMutation,
    refreshMutation,
    revealSecretMutation,
    deleteMutation
  } = useModelProviderMutations({
    csrfToken,
    queryClient,
    setDrawerState
  });

  const errorMessage =
    getErrorMessage(catalogQuery.error) ??
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
    getErrorMessage(deleteMutation.error);
  const sectionStatus = useMemo(
    () =>
      errorMessage ? (
        <Alert type="error" showIcon message={errorMessage} />
      ) : null,
    [errorMessage]
  );
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
                entries={catalogEntries}
                loading={catalogQuery.isLoading}
                canManage={canManage}
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
              />
            </Layout.Content>
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
        onClose={() => setInstanceModalState(null)}
        onEdit={(instance) => {
          setDrawerState({ mode: 'edit', instanceId: instance.id });
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
          if (!instanceModalState || !modalMainInstance) return;
          updateMainInstanceSettingsMutation.mutate({
            providerCode: instanceModalState.providerCode,
            auto_include_new_instances: checked,
            expected_revision: modalMainInstance.revision,
            model_routing_policies: modalMainInstance.model_routing_policies
          });
        }}
        onChangeDistributionRule={(modelId, distributionRule) => {
          if (!instanceModalState || !modalMainInstance) return;
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
          if (!instanceModalState || !modalMainInstance) return;
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
    </>
  );
}
