import { useEffect, useMemo, useRef, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  ArrowLeftOutlined,
  DatabaseOutlined,
  CloudServerOutlined,
  HomeOutlined
} from '@ant-design/icons';
import {
  Alert,
  Breadcrumb,
  Button,
  Flex,
  Tag,
  Typography,
  message
} from 'antd';

import { useAuthStore } from '../../../../state/auth-store';
import {
  createSettingsDataModel,
  createSettingsDataModelField,
  createSettingsDataSource,
  deleteSettingsDataModel,
  deleteSettingsDataModelField,
  fetchSettingsDataModelAdvisorFindings,
  fetchSettingsDataModelScopeGrants,
  fetchSettingsDataModels,
  fetchSettingsDataSourceCatalog,
  fetchSettingsDataSources,
  fetchSettingsDataSourceResources,
  discoverSettingsDataSourceResources,
  mapSettingsDataSourceResourceToModel,
  previewSettingsDataSourceResource,
  settingsDataModelAdvisorFindingsQueryKey,
  settingsDataModelsQueryKey,
  settingsDataModelScopeGrantsQueryKey,
  settingsDataSourceCatalogQueryKey,
  settingsDataSourcesQueryKey,
  settingsDataSourceResourcesQueryKey,
  validateSettingsDataSource,
  updateSettingsDataModel,
  updateSettingsDataModelField,
  updateSettingsDataModelScopeGrant,
  type CreateSettingsDataModelFieldInput,
  type CreateSettingsDataModelInput,
  type CreateSettingsDataSourceInput,
  type SettingsDataModel,
  type SettingsDataModelField,
  type SettingsDataModelScopeGrant,
  type SettingsDataSource,
  type SettingsRuntimeExtensionDataSource,
  type SettingsDataSourcePreview,
  type SettingsDataSourceRemoteResource,
  type UpdateSettingsDataModelFieldInput,
  type UpdateSettingsDataModelInput,
  type UpdateSettingsDataModelScopeGrantInput
} from '../../api/data-models';
import { DataModelDetail } from '../../components/data-models/DataModelDetail';
import { DataModelDetailDrawer } from '../../components/data-models/DataModelDetailDrawer';
import { DataModelTable } from '../../components/data-models/DataModelTable';
import { DataSourcePanel } from '../../components/data-models/DataSourcePanel';
import { DataSourceResourcePreviewDrawer } from '../../components/data-models/DataSourceResourcePreviewDrawer';
import { DataSourceResourcesPanel } from '../../components/data-models/DataSourceResourcesPanel';
import '../../components/data-models/data-model-panel.css';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import { i18nText } from '../../../../shared/i18n/text';

function getErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : null;
}

const emptyDataSources: SettingsDataSource[] = [];
const emptyModels: SettingsDataModel[] = [];

function isRuntimeExtensionDataSource(
  source: SettingsDataSource | null
): source is SettingsRuntimeExtensionDataSource {
  return source?.backend.kind === 'runtime_extension';
}

function readSourceIdFromLocation() {
  if (typeof window === 'undefined') {
    return null;
  }

  return new URLSearchParams(window.location.search).get('source');
}

function writeSourceIdToLocation(sourceId: string | null) {
  const url = new URL(window.location.href);
  if (sourceId) {
    url.searchParams.set('source', sourceId);
  } else {
    url.searchParams.delete('source');
  }

  window.history.pushState({}, '', `${url.pathname}${url.search}${url.hash}`);
}

export function SettingsDataModelsSection({
  canManage
}: {
  canManage: boolean;
}) {
  const queryClient = useQueryClient();
  const [messageApi, contextHolder] = message.useMessage();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const [selectedSourceId, setSelectedSourceId] = useState<string | null>(
    readSourceIdFromLocation
  );
  const [selectedModelId, setSelectedModelId] = useState<string | null>(null);
  const [editingModelId, setEditingModelId] = useState<string | null>(null);
  const [resourcePreview, setResourcePreview] = useState<{
    resource: SettingsDataSourceRemoteResource;
    preview: SettingsDataSourcePreview;
  } | null>(null);

  const dataSourcesQuery = useQuery({
    queryKey: settingsDataSourcesQueryKey,
    queryFn: fetchSettingsDataSources
  });
  const catalogQuery = useQuery({
    queryKey: settingsDataSourceCatalogQueryKey,
    queryFn: fetchSettingsDataSourceCatalog
  });

  const sources = dataSourcesQuery.data ?? emptyDataSources;
  const selectedSource = useMemo(
    () => sources.find((source) => source.id === selectedSourceId) ?? null,
    [selectedSourceId, sources]
  );
  const effectiveSourceId = selectedSource?.id ?? null;

  const modelsQuery = useQuery({
    queryKey: settingsDataModelsQueryKey(effectiveSourceId ?? ''),
    queryFn: () => fetchSettingsDataModels(effectiveSourceId ?? ''),
    enabled: Boolean(selectedSource)
  });

  const selectedRuntimeExtensionDataSource = isRuntimeExtensionDataSource(
    selectedSource
  )
    ? selectedSource
    : null;
  const resourcesQuery = useQuery({
    queryKey: settingsDataSourceResourcesQueryKey(
      selectedRuntimeExtensionDataSource?.id ?? ''
    ),
    queryFn: () =>
      fetchSettingsDataSourceResources(
        selectedRuntimeExtensionDataSource?.id ?? ''
      ),
    enabled:
      selectedRuntimeExtensionDataSource?.capabilities
        .can_discover_resources === true
  });

  const models = modelsQuery.data ?? emptyModels;
  const editingModel = useMemo(
    () => models.find((model) => model.id === editingModelId) ?? null,
    [editingModelId, models]
  );
  const previousEffectiveSourceIdRef = useRef(effectiveSourceId);

  useEffect(() => {
    const previousEffectiveSourceId = previousEffectiveSourceIdRef.current;
    previousEffectiveSourceIdRef.current = effectiveSourceId;

    if (
      previousEffectiveSourceId !== null &&
      previousEffectiveSourceId !== effectiveSourceId
    ) {
      setSelectedModelId(null);
      setEditingModelId(null);
      setResourcePreview(null);
    }
  }, [effectiveSourceId]);

  useEffect(() => {
    const handlePopState = () => {
      setSelectedSourceId(readSourceIdFromLocation());
    };

    window.addEventListener('popstate', handlePopState);
    return () => window.removeEventListener('popstate', handlePopState);
  }, []);

  useEffect(() => {
    if (
      selectedSourceId &&
      !dataSourcesQuery.isLoading &&
      !sources.some((source) => source.id === selectedSourceId)
    ) {
      setSelectedSourceId(null);
      writeSourceIdToLocation(null);
    }
  }, [
    dataSourcesQuery.isLoading,
    selectedSourceId,
    sources
  ]);

  const openSourceManager = (sourceId: string) => {
    setSelectedSourceId(sourceId);
    writeSourceIdToLocation(sourceId);
  };

  const closeSourceManager = () => {
    setSelectedSourceId(null);
    setSelectedModelId(null);
    setEditingModelId(null);
    setResourcePreview(null);
    writeSourceIdToLocation(null);
  };

  const createDataSourceMutation = useMutation({
    mutationFn: (input: CreateSettingsDataSourceInput) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }
      return createSettingsDataSource(input, csrfToken);
    },
    onSuccess: async (dataSource) => {
      messageApi.success(i18nText('settings', 'auto.data_source_created'));
      await queryClient.invalidateQueries({
        queryKey: settingsDataSourcesQueryKey
      });
      openSourceManager(dataSource.id);
    }
  });

  const validateDataSourceMutation = useMutation({
    mutationFn: (dataSource: SettingsRuntimeExtensionDataSource) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }
      return validateSettingsDataSource(dataSource.id, csrfToken);
    },
    onSuccess: async () => {
      messageApi.success(i18nText('settings', 'auto.data_source_ready'));
      await queryClient.invalidateQueries({
        queryKey: settingsDataSourcesQueryKey
      });
    }
  });

  const discoverResourcesMutation = useMutation({
    mutationFn: (dataSource: SettingsRuntimeExtensionDataSource) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }
      return discoverSettingsDataSourceResources(dataSource.id, csrfToken);
    },
    onSuccess: async (_result, dataSource) => {
      await queryClient.invalidateQueries({
        queryKey: settingsDataSourceResourcesQueryKey(dataSource.id)
      });
    }
  });

  const previewResourceMutation = useMutation({
    onMutate: () => setResourcePreview(null),
    mutationFn: ({
      dataSource,
      resource
    }: {
      dataSource: SettingsRuntimeExtensionDataSource;
      resource: SettingsDataSourceRemoteResource;
    }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }
      return previewSettingsDataSourceResource(
        dataSource.id,
        resource.resource_key,
        csrfToken
      );
    },
    onSuccess: (preview, { resource }) => {
      setResourcePreview({ resource, preview });
    }
  });

  const mapResourceMutation = useMutation({
    mutationFn: ({
      dataSource,
      resource
    }: {
      dataSource: SettingsRuntimeExtensionDataSource;
      resource: SettingsDataSourceRemoteResource;
    }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }
      return mapSettingsDataSourceResourceToModel(
        dataSource.id,
        resource.resource_key,
        csrfToken
      );
    },
    onSuccess: async (model, { dataSource }) => {
      messageApi.success(i18nText('settings', 'auto.data_model_created'));
      setSelectedModelId(model.id);
      await queryClient.invalidateQueries({
        queryKey: settingsDataModelsQueryKey(dataSource.id)
      });
    }
  });

  const scopeGrantsQuery = useQuery({
    queryKey: settingsDataModelScopeGrantsQueryKey(editingModel?.id ?? ''),
    queryFn: () => fetchSettingsDataModelScopeGrants(editingModel?.id ?? ''),
    enabled: Boolean(editingModel)
  });

  const advisorQuery = useQuery({
    queryKey: settingsDataModelAdvisorFindingsQueryKey(editingModel?.id ?? ''),
    queryFn: () =>
      fetchSettingsDataModelAdvisorFindings(editingModel?.id ?? ''),
    enabled: Boolean(editingModel)
  });

  const updateModelMutation = useMutation({
    mutationFn: ({
      model,
      input
    }: {
      model: SettingsDataModel;
      input: UpdateSettingsDataModelInput;
    }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }
      return updateSettingsDataModel(model.id, input, csrfToken);
    },
    onSuccess: async () => {
      messageApi.success(i18nText("settings", "auto.data_model_saved"));
      if (effectiveSourceId) {
        await queryClient.invalidateQueries({
          queryKey: settingsDataModelsQueryKey(effectiveSourceId)
        });
      }
    }
  });

  const createModelMutation = useMutation({
    mutationFn: (input: CreateSettingsDataModelInput) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }
      return createSettingsDataModel(input, csrfToken);
    },
    onSuccess: async (model) => {
      messageApi.success(i18nText("settings", "auto.data_model_created"));
      setSelectedModelId(model.id);
      if (effectiveSourceId) {
        await queryClient.invalidateQueries({
          queryKey: settingsDataModelsQueryKey(effectiveSourceId)
        });
      }
    }
  });

  const deleteModelMutation = useMutation({
    mutationFn: (model: SettingsDataModel) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }
      return deleteSettingsDataModel(model.id, csrfToken);
    },
    onSuccess: async (_result, model) => {
      messageApi.success(i18nText("settings", "auto.data_model_deleted"));
      if (selectedModelId === model.id) {
        setSelectedModelId(null);
      }
      if (editingModelId === model.id) {
        setEditingModelId(null);
      }
      if (effectiveSourceId) {
        await queryClient.invalidateQueries({
          queryKey: settingsDataModelsQueryKey(effectiveSourceId)
        });
      }
    }
  });

  const createFieldMutation = useMutation({
    mutationFn: ({
      model,
      input
    }: {
      model: SettingsDataModel;
      input: CreateSettingsDataModelFieldInput;
    }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }
      return createSettingsDataModelField(model.id, input, csrfToken);
    },
    onSuccess: async () => {
      messageApi.success(i18nText("settings", "auto.field_created"));
      if (effectiveSourceId) {
        await queryClient.invalidateQueries({
          queryKey: settingsDataModelsQueryKey(effectiveSourceId)
        });
      }
    }
  });

  const updateFieldMutation = useMutation({
    mutationFn: ({
      model,
      field,
      input
    }: {
      model: SettingsDataModel;
      field: SettingsDataModelField;
      input: UpdateSettingsDataModelFieldInput;
    }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }
      return updateSettingsDataModelField(model.id, field.id, input, csrfToken);
    },
    onSuccess: async () => {
      messageApi.success(i18nText("settings", "auto.field_saved"));
      if (effectiveSourceId) {
        await queryClient.invalidateQueries({
          queryKey: settingsDataModelsQueryKey(effectiveSourceId)
        });
      }
    }
  });

  const deleteFieldMutation = useMutation({
    mutationFn: ({
      model,
      field
    }: {
      model: SettingsDataModel;
      field: SettingsDataModelField;
    }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }
      return deleteSettingsDataModelField(model.id, field.id, csrfToken);
    },
    onSuccess: async () => {
      messageApi.success(i18nText("settings", "auto.field_deleted"));
      if (effectiveSourceId) {
        await queryClient.invalidateQueries({
          queryKey: settingsDataModelsQueryKey(effectiveSourceId)
        });
      }
    }
  });

  const saveGrantMutation = useMutation({
    mutationFn: ({
      grant,
      input
    }: {
      grant: SettingsDataModelScopeGrant;
      input: UpdateSettingsDataModelScopeGrantInput;
    }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }
      return updateSettingsDataModelScopeGrant(
        grant.data_model_id,
        grant.id,
        input,
        csrfToken
      );
    },
    onSuccess: async (_result, variables) => {
      await queryClient.invalidateQueries({
        queryKey: settingsDataModelScopeGrantsQueryKey(
          variables.grant.data_model_id
        )
      });
    }
  });

  const errorMessage =
    getErrorMessage(dataSourcesQuery.error) ??
    getErrorMessage(catalogQuery.error) ??
    getErrorMessage(resourcesQuery.error) ??
    getErrorMessage(modelsQuery.error) ??
    getErrorMessage(scopeGrantsQuery.error) ??
    getErrorMessage(advisorQuery.error) ??
    getErrorMessage(updateModelMutation.error) ??
    getErrorMessage(createModelMutation.error) ??
    getErrorMessage(deleteModelMutation.error) ??
    getErrorMessage(createFieldMutation.error) ??
    getErrorMessage(updateFieldMutation.error) ??
    getErrorMessage(deleteFieldMutation.error) ??
    getErrorMessage(saveGrantMutation.error) ??
    getErrorMessage(validateDataSourceMutation.error) ??
    getErrorMessage(discoverResourcesMutation.error) ??
    getErrorMessage(previewResourceMutation.error) ??
    getErrorMessage(mapResourceMutation.error);

  return (
    <SettingsSectionSurface
      heightMode="fill"
      status={
        errorMessage ? (
          <Alert type="error" showIcon title={errorMessage} />
        ) : null
      }
    >
      {contextHolder}
      <div className="data-model-panel">
        {selectedSource ? (
          <Flex vertical gap={16} className="data-model-panel__models">
            <div className="data-model-panel__manager-head">
              <Breadcrumb
                items={[
                  {
                    title: (
                      <Button
                        type="link"
                        icon={<HomeOutlined aria-hidden="true" />}
                        className="data-model-panel__breadcrumb-link"
                        onClick={closeSourceManager}
                      >
                        {i18nText("settings", "auto.data_source_management")}</Button>
                    )
                  },
                  { title: selectedSource.display_name }
                ]}
              />

              <Flex
                align="center"
                className="data-model-panel__manager-title-row"
                gap={12}
                wrap="wrap"
              >
                <Button
                  aria-label={i18nText("settings", "auto.back")}
                  className="data-model-panel__back-button"
                  icon={<ArrowLeftOutlined aria-hidden="true" />}
                  onClick={closeSourceManager}
                  type="text"
                />
                <div
                  className={`data-model-panel__source-icon-wrapper ${selectedSource.backend.kind} small`}
                >
                  {selectedSource.backend.kind === 'core' ? (
                    <DatabaseOutlined aria-hidden="true" />
                  ) : (
                    <CloudServerOutlined aria-hidden="true" />
                  )}
                </div>
                <Typography.Title
                  level={4}
                  className="data-model-panel__section-title"
                  style={{ margin: 0, lineHeight: '24px' }}
                >
                  {selectedSource.display_name}
                </Typography.Title>
                <Tag
                  color={
                    selectedSource.status === 'ready' ? 'success' : 'default'
                  }
                  style={{ borderRadius: 12, margin: 0 }}
                >
                  {selectedSource.status === 'ready'
                    ? i18nText("settings", "auto.ready")
                    : selectedSource.status}
                </Tag>
                {selectedRuntimeExtensionDataSource ? (
                  <Typography.Text type="secondary" style={{ fontSize: 13 }}>
                    <code className="data-model-panel__code-badge">
                      {selectedRuntimeExtensionDataSource.backend.source_code}
                    </code>
                  </Typography.Text>
                ) : null}
              </Flex>

            </div>
            {selectedRuntimeExtensionDataSource ? (
              <DataSourceResourcesPanel
                dataSource={selectedRuntimeExtensionDataSource}
                resources={resourcesQuery.data?.entries ?? []}
                loading={resourcesQuery.isLoading}
                validating={validateDataSourceMutation.isPending}
                discovering={discoverResourcesMutation.isPending}
                previewingResourceKey={
                  previewResourceMutation.isPending
                    ? (previewResourceMutation.variables?.resource.resource_key ??
                      null)
                    : null
                }
                mappingResourceKey={
                  mapResourceMutation.isPending
                    ? (mapResourceMutation.variables?.resource.resource_key ??
                      null)
                    : null
                }
                canManage={canManage}
                onValidate={() =>
                  validateDataSourceMutation.mutate(
                    selectedRuntimeExtensionDataSource
                  )
                }
                onDiscover={() =>
                  discoverResourcesMutation.mutate(
                    selectedRuntimeExtensionDataSource
                  )
                }
                onPreview={(resource) =>
                  previewResourceMutation.mutate({
                    dataSource: selectedRuntimeExtensionDataSource,
                    resource
                  })
                }
                onMap={(resource) =>
                  mapResourceMutation.mutate({
                    dataSource: selectedRuntimeExtensionDataSource,
                    resource
                  })
                }
              />
            ) : null}
            {selectedRuntimeExtensionDataSource && resourcePreview ? (
              <DataSourceResourcePreviewDrawer
                dataSource={selectedRuntimeExtensionDataSource}
                resource={resourcePreview.resource}
                preview={resourcePreview.preview}
                onClose={() => setResourcePreview(null)}
              />
            ) : null}
            <DataModelTable
              models={models}
              selectedSource={selectedSource}
              selectedModelId={selectedModelId}
              loading={modelsQuery.isLoading}
              saving={
                createModelMutation.isPending ||
                updateModelMutation.isPending ||
                deleteModelMutation.isPending
              }
              canManage={canManage}
              onSelectModel={(model) => setSelectedModelId(model.id)}
              onEditModel={(model) => {
                setSelectedModelId(model.id);
                setEditingModelId(model.id);
              }}
              onDeleteModel={(model) => deleteModelMutation.mutate(model)}
              onCreateModel={(input) => createModelMutation.mutate(input)}
              onUpdateModel={(model, input) =>
                updateModelMutation.mutate({ model, input })
              }
            />

            <DataModelDetailDrawer
              title={
                editingModel ? i18nText("settings", "auto.edit_item", { value1: editingModel.title }) : i18nText("settings", "auto.edit_data_model")
              }
              open={Boolean(editingModel)}
              onClose={() => setEditingModelId(null)}
            >
              {editingModel ? (
                <DataModelDetail
                  model={editingModel}
                  allModels={models}
                  canManage={canManage}
                  grants={scopeGrantsQuery.data ?? []}
                  grantsLoading={scopeGrantsQuery.isLoading}
                  grantsSaving={saveGrantMutation.isPending}
                  advisorFindings={advisorQuery.data ?? []}
                  advisorLoading={advisorQuery.isLoading}
                  modelSaving={updateModelMutation.isPending}
                  fieldSaving={
                    createFieldMutation.isPending ||
                    updateFieldMutation.isPending ||
                    deleteFieldMutation.isPending
                  }
                  onUpdateModel={(input) =>
                    updateModelMutation.mutate({ model: editingModel, input })
                  }
                  onCreateField={(input) =>
                    createFieldMutation.mutate({ model: editingModel, input })
                  }
                  onUpdateField={(field, input) =>
                    updateFieldMutation.mutate({
                      model: editingModel,
                      field,
                      input
                    })
                  }
                  onDeleteField={(field) =>
                    deleteFieldMutation.mutate({ model: editingModel, field })
                  }
                  onSaveGrant={(grant, input) =>
                    saveGrantMutation.mutate({ grant, input })
                  }
                />
              ) : null}
            </DataModelDetailDrawer>
          </Flex>
        ) : (
          <DataSourcePanel
            dataSources={sources}
            catalog={catalogQuery.data?.entries ?? []}
            loading={dataSourcesQuery.isLoading || catalogQuery.isLoading}
            creating={createDataSourceMutation.isPending}
            creationErrorMessage={getErrorMessage(
              createDataSourceMutation.error
            )}
            canManage={canManage}
            onRefresh={async () => {
              await Promise.all([
                dataSourcesQuery.refetch(),
                catalogQuery.refetch()
              ]);
            }}
            onOpenDataSource={openSourceManager}
            onCreateDataSource={(input) =>
              createDataSourceMutation.mutateAsync(input).then(() => undefined)
            }
          />
        )}
      </div>
    </SettingsSectionSurface>
  );
}
