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
  createSettingsDataSourceConnection,
  deleteSettingsDataModel,
  deleteSettingsDataModelField,
  fetchSettingsDataModelAdvisorFindings,
  fetchSettingsDataModelScopeGrants,
  fetchSettingsDataModels,
  fetchSettingsDataSourceCatalog,
  fetchSettingsDataSourceConnections,
  fetchSettingsDataSourceResources,
  fetchSettingsMainDataSource,
  discoverSettingsDataSourceResources,
  mapSettingsDataSourceResourceToModel,
  previewSettingsDataSourceResource,
  settingsDataModelAdvisorFindingsQueryKey,
  settingsDataModelsQueryKey,
  settingsDataModelScopeGrantsQueryKey,
  settingsDataSourceCatalogQueryKey,
  settingsDataSourceConnectionsQueryKey,
  settingsDataSourceResourcesQueryKey,
  settingsMainDataSourceQueryKey,
  validateSettingsDataSourceConnection,
  updateSettingsDataModel,
  updateSettingsDataModelField,
  updateSettingsDataModelScopeGrant,
  type CreateSettingsDataModelFieldInput,
  type CreateSettingsDataModelInput,
  type CreateSettingsDataSourceConnectionInput,
  type SettingsDataModel,
  type SettingsDataModelField,
  type SettingsDataModelScopeGrant,
  type SettingsDataSource,
  type SettingsDataSourceConnection,
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

const emptyConnections: SettingsDataSourceConnection[] = [];
const emptyModels: SettingsDataModel[] = [];

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

  const mainSourceQuery = useQuery({
    queryKey: settingsMainDataSourceQueryKey,
    queryFn: fetchSettingsMainDataSource
  });
  const connectionsQuery = useQuery({
    queryKey: settingsDataSourceConnectionsQueryKey,
    queryFn: fetchSettingsDataSourceConnections
  });
  const catalogQuery = useQuery({
    queryKey: settingsDataSourceCatalogQueryKey,
    queryFn: fetchSettingsDataSourceCatalog
  });

  const connections = connectionsQuery.data ?? emptyConnections;
  const sources = useMemo<SettingsDataSource[]>(
    () =>
      mainSourceQuery.data
        ? [mainSourceQuery.data, ...connections]
        : connections,
    [connections, mainSourceQuery.data]
  );
  const selectedSource = useMemo(
    () => sources.find((source) => source.id === selectedSourceId) ?? null,
    [selectedSourceId, sources]
  );
  const effectiveSourceId = selectedSource?.id ?? null;

  const modelsQuery = useQuery({
    queryKey: settingsDataModelsQueryKey(effectiveSourceId ?? ''),
    queryFn: () => fetchSettingsDataModels(selectedSource as SettingsDataSource),
    enabled: Boolean(selectedSource)
  });

  const selectedConnection =
    selectedSource?.source_kind === 'external_source' ? selectedSource : null;
  const resourcesQuery = useQuery({
    queryKey: settingsDataSourceResourcesQueryKey(selectedConnection?.id ?? ''),
    queryFn: () => fetchSettingsDataSourceResources(selectedConnection?.id ?? ''),
    enabled: selectedConnection?.status === 'ready'
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
      !mainSourceQuery.isLoading &&
      !connectionsQuery.isLoading &&
      !sources.some((source) => source.id === selectedSourceId)
    ) {
      setSelectedSourceId(null);
      writeSourceIdToLocation(null);
    }
  }, [
    connectionsQuery.isLoading,
    mainSourceQuery.isLoading,
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
    writeSourceIdToLocation(null);
  };

  const createConnectionMutation = useMutation({
    mutationFn: (input: CreateSettingsDataSourceConnectionInput) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }
      return createSettingsDataSourceConnection(input, csrfToken);
    },
    onSuccess: async (connection) => {
      messageApi.success(i18nText('settings', 'auto.connection_created'));
      await queryClient.invalidateQueries({
        queryKey: settingsDataSourceConnectionsQueryKey
      });
      openSourceManager(connection.id);
    }
  });

  const validateConnectionMutation = useMutation({
    mutationFn: (connection: SettingsDataSourceConnection) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }
      return validateSettingsDataSourceConnection(connection.id, csrfToken);
    },
    onSuccess: async () => {
      messageApi.success(i18nText('settings', 'auto.connection_ready'));
      await queryClient.invalidateQueries({
        queryKey: settingsDataSourceConnectionsQueryKey
      });
    }
  });

  const discoverResourcesMutation = useMutation({
    mutationFn: (connection: SettingsDataSourceConnection) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }
      return discoverSettingsDataSourceResources(connection.id, csrfToken);
    },
    onSuccess: async (_result, connection) => {
      await queryClient.invalidateQueries({
        queryKey: settingsDataSourceResourcesQueryKey(connection.id)
      });
    }
  });

  const previewResourceMutation = useMutation({
    onMutate: () => setResourcePreview(null),
    mutationFn: ({
      connection,
      resource
    }: {
      connection: SettingsDataSourceConnection;
      resource: SettingsDataSourceRemoteResource;
    }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }
      return previewSettingsDataSourceResource(
        connection.id,
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
      connection,
      resource
    }: {
      connection: SettingsDataSourceConnection;
      resource: SettingsDataSourceRemoteResource;
    }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }
      return mapSettingsDataSourceResourceToModel(
        connection.id,
        resource.resource_key,
        csrfToken
      );
    },
    onSuccess: async (model, { connection }) => {
      messageApi.success(i18nText('settings', 'auto.data_model_created'));
      setSelectedModelId(model.id);
      await queryClient.invalidateQueries({
        queryKey: settingsDataModelsQueryKey(connection.id)
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
    getErrorMessage(mainSourceQuery.error) ??
    getErrorMessage(connectionsQuery.error) ??
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
    getErrorMessage(validateConnectionMutation.error) ??
    getErrorMessage(discoverResourcesMutation.error) ??
    getErrorMessage(previewResourceMutation.error) ??
    getErrorMessage(mapResourceMutation.error);

  return (
    <SettingsSectionSurface
      title={i18nText("settings", "auto.data_source")}
      description={i18nText("settings", "auto.data_source_description")}
      hideHeader={true}
      heightMode="fill"
      status={
        errorMessage ? (
          <Alert type="error" showIcon message={errorMessage} />
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
                        icon={<HomeOutlined />}
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
                  icon={<ArrowLeftOutlined />}
                  onClick={closeSourceManager}
                  type="text"
                />
                <div
                  className={`data-model-panel__source-icon-wrapper ${selectedSource.source_kind} small`}
                >
                  {selectedSource.source_kind === 'main_source' ? (
                    <DatabaseOutlined />
                  ) : (
                    <CloudServerOutlined />
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
                {selectedConnection ? (
                  <Typography.Text type="secondary" style={{ fontSize: 13 }}>
                    <code className="data-model-panel__code-badge">
                      {selectedConnection.source_code}
                    </code>
                  </Typography.Text>
                ) : null}
              </Flex>

            </div>
            {selectedConnection ? (
              <DataSourceResourcesPanel
                connection={selectedConnection}
                resources={resourcesQuery.data?.entries ?? []}
                loading={resourcesQuery.isLoading}
                validating={validateConnectionMutation.isPending}
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
                  validateConnectionMutation.mutate(selectedConnection)
                }
                onDiscover={() =>
                  discoverResourcesMutation.mutate(selectedConnection)
                }
                onPreview={(resource) =>
                  previewResourceMutation.mutate({
                    connection: selectedConnection,
                    resource
                  })
                }
                onMap={(resource) =>
                  mapResourceMutation.mutate({
                    connection: selectedConnection,
                    resource
                  })
                }
              />
            ) : null}
            {selectedConnection && resourcePreview ? (
              <DataSourceResourcePreviewDrawer
                connection={selectedConnection}
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
            mainSource={mainSourceQuery.data ?? null}
            connections={connections}
            catalog={catalogQuery.data?.entries ?? []}
            loading={
              mainSourceQuery.isLoading ||
              connectionsQuery.isLoading ||
              catalogQuery.isLoading
            }
            creating={createConnectionMutation.isPending}
            creationErrorMessage={getErrorMessage(
              createConnectionMutation.error
            )}
            canManage={canManage}
            onOpenMainSource={() => openSourceManager('main_source')}
            onOpenConnection={openSourceManager}
            onCreateConnection={(input) =>
              createConnectionMutation.mutateAsync(input).then(() => undefined)
            }
          />
        )}
      </div>
    </SettingsSectionSurface>
  );
}
