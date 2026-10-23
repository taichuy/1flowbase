import {
  fetchConsoleCompatibleDataModelTemplates,
  fetchConsoleDataModels,
  fetchConsoleDataSourceCatalog,
  fetchConsoleDataSources,
  previewConsoleDataSourceResource,
  type BatchDeleteConsoleDataModelsInput,
  type BatchDeleteConsoleDataModelsResult,
  type ConsoleDataModel,
  type ConsoleCompatibleDataModelTemplate,
  type ConsoleDataModelAdvisorFinding,
  type ConsoleDataModelField,
  type ConsoleDataModelScopeGrant,
  type ConsoleDataModelOpenApiDocument,
  type ConsoleDataSourceCatalogEntry,
  type ConsoleDataSource,
  type ConsoleDataSourceRemoteResource,
  type ConsoleDataSourceResources,
  type ConsoleRuntimeExtensionDataSource,
  type ConsoleDataSourcePreview,
  type ConsoleRuntimeRecordPreview,
  type CreateConsoleDataModelFieldInput,
  type CreateConsoleDataModelInput,
  type CreateConsoleDataSourceInput,
  type CreateConsoleDataModelScopeGrantInput,
  type MapConsoleDataSourceResourceToModelInput,
  type UpdateConsoleDataModelFieldInput,
  type UpdateConsoleDataModelInput,
  type UpdateConsoleDataModelScopeGrantInput,
  type UpdateConsoleDataSourceDefaultsInput
} from '@1flowbase/api-client';

export {
  createConsoleDataSource as createSettingsDataSource,
  validateConsoleDataSource as validateSettingsDataSource,
  updateConsoleDataSourceDefaults as updateSettingsDataSourceDefaults,
  fetchConsoleDataSourceResources as fetchSettingsDataSourceResources,
  discoverConsoleDataSourceResources as discoverSettingsDataSourceResources,
  mapConsoleDataSourceResourceToModel as mapSettingsDataSourceResourceToModel,
  createConsoleDataModel as createSettingsDataModel,
  updateConsoleDataModel as updateSettingsDataModel,
  deleteConsoleDataModel as deleteSettingsDataModel,
  batchDeleteConsoleDataModels as batchDeleteSettingsDataModels,
  createConsoleDataModelField as createSettingsDataModelField,
  updateConsoleDataModelField as updateSettingsDataModelField,
  deleteConsoleDataModelField as deleteSettingsDataModelField,
  fetchConsoleDataModelScopeGrants as fetchSettingsDataModelScopeGrants,
  createConsoleDataModelScopeGrant as createSettingsDataModelScopeGrant,
  updateConsoleDataModelScopeGrant as updateSettingsDataModelScopeGrant,
  fetchConsoleDataModelAdvisorFindings as fetchSettingsDataModelAdvisorFindings,
  fetchConsoleDataModelRecordPreview as fetchSettingsDataModelRecordPreview,
  fetchConsoleDataModelOpenApiDocument as fetchSettingsDataModelOpenApiDocument
} from '@1flowbase/api-client';

export type SettingsDataSource = ConsoleDataSource;
export type SettingsRuntimeExtensionDataSource =
  ConsoleRuntimeExtensionDataSource;
export type SettingsDataSourceCatalogEntry = ConsoleDataSourceCatalogEntry;
export type SettingsDataSourceRemoteResource = ConsoleDataSourceRemoteResource;
export type SettingsDataSourceResources = ConsoleDataSourceResources;
export type SettingsDataSourcePreview = ConsoleDataSourcePreview;
export type SettingsDataModel = ConsoleDataModel;
export type SettingsCompatibleDataModelTemplate =
  ConsoleCompatibleDataModelTemplate;
export type SettingsDataModelField = ConsoleDataModelField;
export type SettingsDataModelScopeGrant = ConsoleDataModelScopeGrant;
export type SettingsDataModelAdvisorFinding = ConsoleDataModelAdvisorFinding;
export type SettingsRuntimeRecordPreview = ConsoleRuntimeRecordPreview;
export type SettingsDataModelOpenApiDocument = ConsoleDataModelOpenApiDocument;
export type BatchDeleteSettingsDataModelsInput =
  BatchDeleteConsoleDataModelsInput;
export type BatchDeleteSettingsDataModelsResult =
  BatchDeleteConsoleDataModelsResult;
export type CreateSettingsDataModelInput = CreateConsoleDataModelInput;
export type MapSettingsDataSourceResourceToModelInput =
  MapConsoleDataSourceResourceToModelInput;
export type CreateSettingsDataSourceInput = CreateConsoleDataSourceInput;
export type UpdateSettingsDataModelInput = UpdateConsoleDataModelInput;
export type CreateSettingsDataModelFieldInput =
  CreateConsoleDataModelFieldInput;
export type UpdateSettingsDataModelFieldInput =
  UpdateConsoleDataModelFieldInput;
export type CreateSettingsDataModelScopeGrantInput =
  CreateConsoleDataModelScopeGrantInput;
export type UpdateSettingsDataModelScopeGrantInput =
  UpdateConsoleDataModelScopeGrantInput;
export type UpdateSettingsDataSourceDefaultsInput =
  UpdateConsoleDataSourceDefaultsInput;

export const settingsDataSourcesQueryKey = [
  'settings',
  'data-models',
  'data-sources'
] as const;

export const settingsDataSourceCatalogQueryKey = [
  'settings',
  'data-models',
  'data-source-catalog'
] as const;

export function settingsDataSourceResourcesQueryKey(dataSourceId: string) {
  return [
    'settings',
    'data-models',
    'data-sources',
    dataSourceId,
    'resources'
  ] as const;
}

export function settingsCompatibleDataModelTemplatesQueryKey(
  dataSourceId: string,
  resourceKey?: string
) {
  return [
    'settings',
    'data-models',
    'compatible-templates',
    dataSourceId,
    resourceKey ?? null
  ] as const;
}

export function settingsDataModelsQueryKey(
  sourceId: string,
  filter: Record<string, unknown> = {}
) {
  return [
    'settings',
    'data-models',
    'models',
    sourceId,
    JSON.stringify(filter)
  ] as const;
}

export const settingsAllDataModelsQueryKey = [
  'settings',
  'data-models',
  'models',
  'all'
] as const;

export function settingsDataModelScopeGrantsQueryKey(modelId: string) {
  return ['settings', 'data-models', 'scope-grants', modelId] as const;
}

export function settingsDataModelAdvisorFindingsQueryKey(modelId: string) {
  return ['settings', 'data-models', 'advisor', modelId] as const;
}

export function settingsDataModelRecordPreviewQueryKey(modelCode: string) {
  return ['settings', 'data-models', 'record-preview', modelCode] as const;
}

export function settingsDataModelOpenApiQueryKey(modelId: string) {
  return ['settings', 'data-models', 'openapi', modelId] as const;
}

export function fetchSettingsDataSources() {
  return fetchConsoleDataSources();
}

export function fetchSettingsDataSourceCatalog() {
  return fetchConsoleDataSourceCatalog();
}

export function previewSettingsDataSourceResource(
  dataSourceId: string,
  resourceKey: string,
  csrfToken: string
) {
  return previewConsoleDataSourceResource(
    dataSourceId,
    { resource_key: resourceKey, limit: 20, options_json: {} },
    csrfToken
  );
}

export function fetchSettingsCompatibleDataModelTemplates(
  dataSourceId: string,
  resourceKey?: string
) {
  return fetchConsoleCompatibleDataModelTemplates({
    data_source_id: dataSourceId,
    resource_key: resourceKey
  });
}

export function fetchSettingsDataModels(
  dataSourceId: string,
  filter?: Record<string, unknown>
) {
  return fetchConsoleDataModels(
    filter === undefined
      ? { data_source_id: dataSourceId }
      : { data_source_id: dataSourceId, filter }
  );
}

export function fetchSettingsAllDataModels() {
  return fetchConsoleDataModels();
}
