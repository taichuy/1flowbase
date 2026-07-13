import {
  batchDeleteConsoleDataModels,
  createConsoleDataSource,
  createConsoleDataModel,
  createConsoleDataModelField,
  createConsoleDataModelScopeGrant,
  deleteConsoleDataModel,
  deleteConsoleDataModelField,
  discoverConsoleDataSourceResources,
  fetchConsoleDataModelAdvisorFindings,
  fetchConsoleDataModelOpenApiDocument,
  fetchConsoleDataModelRecordPreview,
  fetchConsoleDataModelScopeGrants,
  fetchConsoleDataModels,
  fetchConsoleDataSourceCatalog,
  fetchConsoleDataSources,
  fetchConsoleDataSourceResources,
  mapConsoleDataSourceResourceToModel,
  previewConsoleDataSourceResource,
  updateConsoleDataModel,
  updateConsoleDataModelField,
  updateConsoleDataModelScopeGrant,
  updateConsoleDataSourceDefaults,
  validateConsoleDataSource,
  type BatchDeleteConsoleDataModelsInput,
  type BatchDeleteConsoleDataModelsResult,
  type ConsoleDataModel,
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
  type UpdateConsoleDataModelFieldInput,
  type UpdateConsoleDataModelInput,
  type UpdateConsoleDataModelScopeGrantInput,
  type UpdateConsoleDataSourceDefaultsInput
} from '@1flowbase/api-client';

export type SettingsDataSource = ConsoleDataSource;
export type SettingsRuntimeExtensionDataSource =
  ConsoleRuntimeExtensionDataSource;
export type SettingsDataSourceCatalogEntry = ConsoleDataSourceCatalogEntry;
export type SettingsDataSourceRemoteResource = ConsoleDataSourceRemoteResource;
export type SettingsDataSourceResources = ConsoleDataSourceResources;
export type SettingsDataSourcePreview = ConsoleDataSourcePreview;
export type SettingsDataModel = ConsoleDataModel;
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

export function createSettingsDataSource(
  input: CreateSettingsDataSourceInput,
  csrfToken: string
) {
  return createConsoleDataSource(input, csrfToken);
}

export function validateSettingsDataSource(
  dataSourceId: string,
  csrfToken: string
) {
  return validateConsoleDataSource(dataSourceId, csrfToken);
}

export function updateSettingsDataSourceDefaults(
  dataSourceId: string,
  input: UpdateSettingsDataSourceDefaultsInput,
  csrfToken: string
) {
  return updateConsoleDataSourceDefaults(dataSourceId, input, csrfToken);
}

export function fetchSettingsDataSourceResources(dataSourceId: string) {
  return fetchConsoleDataSourceResources(dataSourceId);
}

export function discoverSettingsDataSourceResources(
  dataSourceId: string,
  csrfToken: string
) {
  return discoverConsoleDataSourceResources(dataSourceId, csrfToken);
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

export function mapSettingsDataSourceResourceToModel(
  dataSourceId: string,
  resourceKey: string,
  csrfToken: string
) {
  return mapConsoleDataSourceResourceToModel(
    dataSourceId,
    resourceKey,
    csrfToken
  );
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

export function createSettingsDataModel(
  input: CreateSettingsDataModelInput,
  csrfToken: string
) {
  return createConsoleDataModel(input, csrfToken);
}

export function updateSettingsDataModel(
  modelId: string,
  input: UpdateSettingsDataModelInput,
  csrfToken: string
) {
  return updateConsoleDataModel(modelId, input, csrfToken);
}

export function deleteSettingsDataModel(modelId: string, csrfToken: string) {
  return deleteConsoleDataModel(modelId, csrfToken);
}

export function batchDeleteSettingsDataModels(
  input: BatchDeleteSettingsDataModelsInput,
  csrfToken: string
) {
  return batchDeleteConsoleDataModels(input, csrfToken);
}

export function createSettingsDataModelField(
  modelId: string,
  input: CreateSettingsDataModelFieldInput,
  csrfToken: string
) {
  return createConsoleDataModelField(modelId, input, csrfToken);
}

export function updateSettingsDataModelField(
  modelId: string,
  fieldId: string,
  input: UpdateSettingsDataModelFieldInput,
  csrfToken: string
) {
  return updateConsoleDataModelField(modelId, fieldId, input, csrfToken);
}

export function deleteSettingsDataModelField(
  modelId: string,
  fieldId: string,
  csrfToken: string
) {
  return deleteConsoleDataModelField(modelId, fieldId, csrfToken);
}

export function fetchSettingsDataModelScopeGrants(modelId: string) {
  return fetchConsoleDataModelScopeGrants(modelId);
}

export function createSettingsDataModelScopeGrant(
  modelId: string,
  input: CreateSettingsDataModelScopeGrantInput,
  csrfToken: string
) {
  return createConsoleDataModelScopeGrant(modelId, input, csrfToken);
}

export function updateSettingsDataModelScopeGrant(
  modelId: string,
  grantId: string,
  input: UpdateSettingsDataModelScopeGrantInput,
  csrfToken: string
) {
  return updateConsoleDataModelScopeGrant(modelId, grantId, input, csrfToken);
}

export function fetchSettingsDataModelAdvisorFindings(modelId: string) {
  return fetchConsoleDataModelAdvisorFindings(modelId);
}

export function fetchSettingsDataModelRecordPreview(modelCode: string) {
  return fetchConsoleDataModelRecordPreview(modelCode);
}

export function fetchSettingsDataModelOpenApiDocument(modelId: string) {
  return fetchConsoleDataModelOpenApiDocument(modelId);
}
