import {
  batchDeleteConsoleDataModels,
  createConsoleDataModel,
  createConsoleDataModelField,
  createConsoleDataModelScopeGrant,
  deleteConsoleDataModel,
  deleteConsoleDataModelField,
  fetchConsoleDataModelAdvisorFindings,
  fetchConsoleDataModelOpenApiDocument,
  fetchConsoleDataModelRecordPreview,
  fetchConsoleDataModelScopeGrants,
  fetchConsoleDataModels,
  fetchConsoleDataSourceInstances,
  updateConsoleDataModel,
  updateConsoleDataModelField,
  updateConsoleDataModelScopeGrant,
  updateConsoleDataSourceDefaults,
  type BatchDeleteConsoleDataModelsInput,
  type BatchDeleteConsoleDataModelsResult,
  type ConsoleDataModel,
  type ConsoleDataModelAdvisorFinding,
  type ConsoleDataModelField,
  type ConsoleDataModelScopeGrant,
  type ConsoleDataModelOpenApiDocument,
  type ConsoleDataSourceInstance,
  type ConsoleRuntimeRecordPreview,
  type CreateConsoleDataModelFieldInput,
  type CreateConsoleDataModelInput,
  type CreateConsoleDataModelScopeGrantInput,
  type UpdateConsoleDataModelFieldInput,
  type UpdateConsoleDataModelInput,
  type UpdateConsoleDataModelScopeGrantInput,
  type UpdateConsoleDataSourceDefaultsInput
} from '@1flowbase/api-client';

export type SettingsDataSourceInstance = ConsoleDataSourceInstance;
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
  'sources'
] as const;

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

export function fetchSettingsDataSourceInstances() {
  return fetchConsoleDataSourceInstances();
}

export function updateSettingsDataSourceDefaults(
  instanceId: string,
  input: UpdateSettingsDataSourceDefaultsInput,
  csrfToken: string
) {
  return updateConsoleDataSourceDefaults(instanceId, input, csrfToken);
}

export function fetchSettingsDataModels(
  dataSourceInstanceId: string,
  filter?: Record<string, unknown>
) {
  return fetchConsoleDataModels(
    filter === undefined
      ? { data_source_instance_id: dataSourceInstanceId }
      : { data_source_instance_id: dataSourceInstanceId, filter }
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
