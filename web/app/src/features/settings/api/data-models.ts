import {
  batchDeleteConsoleDataModels,
  createConsoleDataSourceConnection,
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
  fetchConsoleDataSourceConnections,
  fetchConsoleDataSourceResources,
  fetchConsoleMainDataSource,
  mapConsoleDataSourceResourceToModel,
  previewConsoleDataSourceResource,
  updateConsoleDataModel,
  updateConsoleDataModelField,
  updateConsoleDataModelScopeGrant,
  updateConsoleDataSourceConnectionDefaults,
  updateConsoleMainDataSourceDefaults,
  validateConsoleDataSourceConnection,
  type BatchDeleteConsoleDataModelsInput,
  type BatchDeleteConsoleDataModelsResult,
  type ConsoleDataModel,
  type ConsoleDataModelAdvisorFinding,
  type ConsoleDataModelField,
  type ConsoleDataModelScopeGrant,
  type ConsoleDataModelOpenApiDocument,
  type ConsoleDataSourceCatalogEntry,
  type ConsoleDataSourceConnection,
  type ConsoleDataSourceRemoteResource,
  type ConsoleDataSourceResources,
  type ConsoleMainDataSource,
  type ConsoleDataSourcePreview,
  type ConsoleRuntimeRecordPreview,
  type CreateConsoleDataModelFieldInput,
  type CreateConsoleDataModelInput,
  type CreateConsoleDataSourceConnectionInput,
  type CreateConsoleDataModelScopeGrantInput,
  type UpdateConsoleDataModelFieldInput,
  type UpdateConsoleDataModelInput,
  type UpdateConsoleDataModelScopeGrantInput,
  type UpdateConsoleDataSourceDefaultsInput
} from '@1flowbase/api-client';

export type SettingsMainDataSource = ConsoleMainDataSource;
export type SettingsDataSourceConnection = ConsoleDataSourceConnection;
export type SettingsDataSource =
  | SettingsMainDataSource
  | SettingsDataSourceConnection;
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
export type CreateSettingsDataSourceConnectionInput =
  CreateConsoleDataSourceConnectionInput;
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

export const settingsMainDataSourceQueryKey = [
  'settings',
  'data-models',
  'main-source'
] as const;

export const settingsDataSourceConnectionsQueryKey = [
  'settings',
  'data-models',
  'connections'
] as const;

export const settingsDataSourceCatalogQueryKey = [
  'settings',
  'data-models',
  'connection-catalog'
] as const;

export function settingsDataSourceResourcesQueryKey(connectionId: string) {
  return ['settings', 'data-models', 'connections', connectionId, 'resources'] as const;
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

export function fetchSettingsMainDataSource() {
  return fetchConsoleMainDataSource();
}

export function fetchSettingsDataSourceConnections() {
  return fetchConsoleDataSourceConnections();
}

export function fetchSettingsDataSourceCatalog() {
  return fetchConsoleDataSourceCatalog();
}

export function createSettingsDataSourceConnection(
  input: CreateSettingsDataSourceConnectionInput,
  csrfToken: string
) {
  return createConsoleDataSourceConnection(input, csrfToken);
}

export function validateSettingsDataSourceConnection(
  connectionId: string,
  csrfToken: string
) {
  return validateConsoleDataSourceConnection(connectionId, csrfToken);
}

export function updateSettingsDataSourceConnectionDefaults(
  connectionId: string,
  input: UpdateSettingsDataSourceDefaultsInput,
  csrfToken: string
) {
  return updateConsoleDataSourceConnectionDefaults(
    connectionId,
    input,
    csrfToken
  );
}

export function updateSettingsMainDataSourceDefaults(
  input: UpdateSettingsDataSourceDefaultsInput,
  csrfToken: string
) {
  return updateConsoleMainDataSourceDefaults(input, csrfToken);
}

export function fetchSettingsDataSourceResources(connectionId: string) {
  return fetchConsoleDataSourceResources(connectionId);
}

export function discoverSettingsDataSourceResources(
  connectionId: string,
  csrfToken: string
) {
  return discoverConsoleDataSourceResources(connectionId, csrfToken);
}

export function previewSettingsDataSourceResource(
  connectionId: string,
  resourceKey: string,
  csrfToken: string
) {
  return previewConsoleDataSourceResource(
    connectionId,
    { resource_key: resourceKey, limit: 20, options_json: {} },
    csrfToken
  );
}

export function mapSettingsDataSourceResourceToModel(
  connectionId: string,
  resourceKey: string,
  csrfToken: string
) {
  return mapConsoleDataSourceResourceToModel(
    connectionId,
    resourceKey,
    csrfToken
  );
}

export function fetchSettingsDataModels(
  source: SettingsDataSource,
  filter?: Record<string, unknown>
) {
  const sourceFilter =
    source.source_kind === 'main_source'
      ? { source_kind: 'main_source' as const }
      : { data_source_instance_id: source.id };
  return fetchConsoleDataModels(
    filter === undefined ? sourceFilter : { ...sourceFilter, filter }
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
