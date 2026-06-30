import { apiFetch } from './transport';

export type ConsoleDataModelStatus =
  | 'draft'
  | 'published'
  | 'disabled'
  | 'broken';

export type ConsoleApiExposureStatus =
  | 'draft'
  | 'published_not_exposed'
  | 'api_exposed_no_permission'
  | 'api_exposed_ready'
  | 'unsafe_external_source';

export type ConsoleDataSourceKind = 'main_source' | 'external_source';
export type ConsoleDataModelScopeKind = 'workspace' | 'system';
export type ConsoleDataModelSourceKind = 'main_source' | 'external_source';
export type ConsoleDataModelPermissionProfile =
  | 'owner'
  | 'scope_all'
  | 'system_all';

export type ConsoleDataModelBuiltinKind = 'core' | 'runtime_read';

export type ConsoleDataModelFieldOwnership = 'system_owned' | 'user_added';

export interface ConsoleDataModelRecordCapabilities {
  can_list: boolean;
  can_get: boolean;
  can_create: boolean;
  can_update: boolean;
  can_delete: boolean;
}

export interface ConsoleDataModelCapabilities {
  can_delete: boolean;
  can_add_user_field: boolean;
  can_update_lifecycle_status: boolean;
  record: ConsoleDataModelRecordCapabilities;
}

export interface ConsoleDataModelFieldCapabilities {
  ownership: ConsoleDataModelFieldOwnership;
  can_update_presentation_metadata: boolean;
  can_update_physical_metadata: boolean;
  can_delete: boolean;
}

export interface ConsoleDataSourceInstance {
  id: string;
  source_kind: ConsoleDataSourceKind;
  installation_id: string;
  source_code: string;
  display_name: string;
  status: string;
  default_data_model_status: ConsoleDataModelStatus;
  default_api_exposure_status: ConsoleApiExposureStatus;
  config_json: Record<string, unknown>;
  secret_ref: string | null;
  secret_version: number | null;
  catalog_refresh_status: string | null;
  catalog_last_error_message: string | null;
  catalog_refreshed_at: string | null;
}

export interface ConsoleDataModelField {
  id: string;
  code: string;
  title: string;
  description: string | null;
  physical_column_name: string;
  external_field_key: string | null;
  field_kind: string;
  is_system: boolean;
  is_writable: boolean;
  is_required: boolean;
  api_required: boolean;
  is_unique: boolean;
  default_value: unknown | null;
  display_interface: string | null;
  display_options: Record<string, unknown>;
  relation_target_model_id: string | null;
  relation_options: Record<string, unknown>;
  sort_order: number;
  capabilities: ConsoleDataModelFieldCapabilities;
}

export interface ConsoleDataModel {
  id: string;
  scope_kind: ConsoleDataModelScopeKind;
  scope_id: string;
  code: string;
  title: string;
  status: ConsoleDataModelStatus;
  api_exposure_status: ConsoleApiExposureStatus;
  runtime_availability: string;
  data_source_instance_id: string | null;
  source_kind: ConsoleDataModelSourceKind;
  external_resource_key: string | null;
  external_table_id: string | null;
  physical_table_name: string;
  acl_namespace: string;
  audit_namespace: string;
  builtin_kind: ConsoleDataModelBuiltinKind | null;
  capabilities: ConsoleDataModelCapabilities;
  fields: ConsoleDataModelField[];
}

export type ConsoleAgentFlowDataModelOptionState =
  | 'enabled'
  | 'unpublished'
  | 'disabled'
  | 'broken';

export interface ConsoleAgentFlowDataModelFieldOption {
  code: string;
  title: string;
  valueType: string;
  required: boolean;
  writable: boolean;
}

export interface ConsoleAgentFlowDataModelOption {
  value: string;
  label: string;
  state: ConsoleAgentFlowDataModelOptionState;
  disabled: boolean;
  disabledReason: string | null;
  modelId: string;
  modelCode: string;
  fields: ConsoleAgentFlowDataModelFieldOption[];
}

export interface ConsoleDataModelScopeGrant {
  id: string;
  scope_kind: ConsoleDataModelScopeKind;
  scope_id: string;
  data_model_id: string;
  enabled: boolean;
  permission_profile: ConsoleDataModelPermissionProfile;
}

export interface ConsoleDataModelAdvisorFinding {
  id: string;
  data_model_id: string;
  severity: 'blocking' | 'high' | 'info' | string;
  code: string;
  message: string;
  recommended_action: string;
  can_acknowledge: boolean;
}

export type ConsoleRuntimeModelRecord = Record<string, unknown>;

export interface ConsoleRuntimeRecordPreview {
  items: ConsoleRuntimeModelRecord[];
  total: number;
}

export interface ConsoleDataModelOpenApiDocument {
  openapi: string;
  info?: Record<string, unknown>;
  paths?: Record<string, unknown>;
  components?: Record<string, unknown>;
}

export interface UpdateConsoleDataSourceDefaultsInput {
  default_data_model_status: ConsoleDataModelStatus;
  default_api_exposure_status: Exclude<
    ConsoleApiExposureStatus,
    'api_exposed_ready'
  >;
}

export interface FetchConsoleDataModelsInput {
  data_source_instance_id?: string;
  filter?: Record<string, unknown>;
}

export interface CreateConsoleDataModelInput {
  scope_kind: ConsoleDataModelScopeKind;
  data_source_instance_id?: string | null;
  external_resource_key?: string | null;
  external_table_id?: string | null;
  code: string;
  title: string;
  status?: ConsoleDataModelStatus;
}

export interface UpdateConsoleDataModelInput {
  title?: string;
  status?: ConsoleDataModelStatus;
  external_table_id?: string | null;
}

export interface BatchDeleteConsoleDataModelsInput {
  filterByTk?: string | string[];
  filter?: Record<string, unknown>;
  confirmed: boolean;
}

export interface BatchDeleteConsoleDataModelsResult {
  deleted: boolean;
  deleted_count: number;
  deleted_ids: string[];
}

export interface CreateConsoleDataModelFieldInput {
  code: string;
  title: string;
  description?: string | null;
  external_field_key?: string | null;
  field_kind: string;
  is_required: boolean;
  api_required?: boolean;
  is_unique: boolean;
  default_value: unknown | null;
  display_interface: string | null;
  display_options: Record<string, unknown>;
  relation_target_model_id: string | null;
  relation_options: Record<string, unknown>;
}

export interface UpdateConsoleDataModelFieldInput {
  title: string;
  description?: string | null;
  is_required: boolean;
  api_required?: boolean;
  is_unique: boolean;
  default_value: unknown | null;
  display_interface: string | null;
  display_options: Record<string, unknown>;
  relation_options: Record<string, unknown>;
}

export interface CreateConsoleDataModelScopeGrantInput {
  scope_kind: ConsoleDataModelScopeKind;
  scope_id: string;
  enabled: boolean;
  permission_profile: ConsoleDataModelPermissionProfile;
  confirm_unsafe_external_source_system_all: boolean;
}

export interface UpdateConsoleDataModelScopeGrantInput {
  enabled?: boolean;
  permission_profile?: ConsoleDataModelPermissionProfile;
  confirm_unsafe_external_source_system_all: boolean;
}

export type ConsoleRuntimeModelRecordFilterInput = Record<string, unknown>;

export interface ConsoleRuntimeModelRecordSortInput {
  field: string;
  direction: string;
}

export interface FetchConsoleRuntimeModelRecordsInput {
  page?: number;
  page_size?: number;
  filter?: ConsoleRuntimeModelRecordFilterInput;
  sort?: ConsoleRuntimeModelRecordSortInput | string;
  expand?: string | string[];
}

export type ConsoleRuntimeModelRecordInput = Record<string, unknown>;

function appendQuery(path: string, params: Record<string, string | undefined>) {
  const searchParams = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value) {
      searchParams.set(key, value);
    }
  }
  const query = searchParams.toString();
  return query ? `${path}?${query}` : path;
}

function encodedPathSegment(value: string) {
  return encodeURIComponent(value);
}

function runtimeModelActionPath(modelCode: string, action: string) {
  return `/api/runtime/models/${encodedPathSegment(modelCode)}/${action}`;
}

function runtimeModelRecordActionPath(
  modelCode: string,
  action: string,
  recordId: string
) {
  return `${runtimeModelActionPath(modelCode, action)}/${encodedPathSegment(recordId)}`;
}

function serializeRuntimeRecordFilter(
  filter: FetchConsoleRuntimeModelRecordsInput['filter']
) {
  if (filter === undefined || typeof filter === 'string') {
    return filter;
  }
  return JSON.stringify(filter);
}

function serializeRuntimeRecordSort(
  sort: FetchConsoleRuntimeModelRecordsInput['sort']
) {
  if (sort === undefined || typeof sort === 'string') {
    return sort;
  }
  return `${sort.field}:${sort.direction}`;
}

function serializeRuntimeRecordExpand(
  expand: FetchConsoleRuntimeModelRecordsInput['expand']
) {
  if (Array.isArray(expand)) {
    return expand.join(',');
  }
  return expand;
}

function appendRuntimeRecordsQuery(
  path: string,
  input: FetchConsoleRuntimeModelRecordsInput
) {
  const searchParams = new URLSearchParams();
  if (input.page !== undefined) {
    searchParams.set('page', String(input.page));
  }
  if (input.page_size !== undefined) {
    searchParams.set('page_size', String(input.page_size));
  }

  const filter = serializeRuntimeRecordFilter(input.filter);
  if (filter) {
    searchParams.set('filter', filter);
  }

  const sort = serializeRuntimeRecordSort(input.sort);
  if (sort) {
    searchParams.set('sort', sort);
  }

  const expand = serializeRuntimeRecordExpand(input.expand);
  if (expand) {
    searchParams.set('expand', expand);
  }

  const query = searchParams.toString();
  return query ? `${path}?${query}` : path;
}

export function fetchConsoleDataSourceInstances(baseUrl?: string) {
  return apiFetch<ConsoleDataSourceInstance[]>({
    path: '/api/console/data-sources/instances',
    baseUrl
  });
}

export function updateConsoleDataSourceDefaults(
  instanceId: string,
  input: UpdateConsoleDataSourceDefaultsInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleDataSourceInstance>({
    path: `/api/console/data-sources/instances/${instanceId}/defaults`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function fetchConsoleDataModels(
  input: FetchConsoleDataModelsInput = {},
  baseUrl?: string
) {
  return apiFetch<ConsoleDataModel[]>({
    path: appendQuery('/api/console/models', {
      data_source_instance_id: input.data_source_instance_id,
      filter: input.filter ? JSON.stringify(input.filter) : undefined
    }),
    baseUrl
  });
}

export function fetchConsoleAgentFlowDataModelOptions(baseUrl?: string) {
  return apiFetch<ConsoleAgentFlowDataModelOption[]>({
    path: '/api/console/models/agent-flow-options',
    baseUrl
  });
}

export function createConsoleDataModel(
  input: CreateConsoleDataModelInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleDataModel>({
    path: '/api/console/models',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleDataModel(
  modelId: string,
  input: UpdateConsoleDataModelInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleDataModel>({
    path: `/api/console/models/${modelId}`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleDataModel(
  modelId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<{ deleted: boolean }>({
    path: `/api/console/models/${modelId}?confirmed=true`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function batchDeleteConsoleDataModels(
  input: BatchDeleteConsoleDataModelsInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<BatchDeleteConsoleDataModelsResult>({
    path: '/api/console/models:batchDelete',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function createConsoleDataModelField(
  modelId: string,
  input: CreateConsoleDataModelFieldInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleDataModelField>({
    path: `/api/console/models/${modelId}/fields`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleDataModelField(
  modelId: string,
  fieldId: string,
  input: UpdateConsoleDataModelFieldInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleDataModelField>({
    path: `/api/console/models/${modelId}/fields/${fieldId}`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleDataModelField(
  modelId: string,
  fieldId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<{ deleted: boolean }>({
    path: `/api/console/models/${modelId}/fields/${fieldId}?confirmed=true`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function fetchConsoleDataModelScopeGrants(
  modelId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleDataModelScopeGrant[]>({
    path: `/api/console/models/${modelId}/scope-grants`,
    baseUrl
  });
}

export function createConsoleDataModelScopeGrant(
  modelId: string,
  input: CreateConsoleDataModelScopeGrantInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleDataModelScopeGrant>({
    path: `/api/console/models/${modelId}/scope-grants`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleDataModelScopeGrant(
  modelId: string,
  grantId: string,
  input: UpdateConsoleDataModelScopeGrantInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleDataModelScopeGrant>({
    path: `/api/console/models/${modelId}/scope-grants/${grantId}`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function fetchConsoleDataModelAdvisorFindings(
  modelId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleDataModelAdvisorFinding[]>({
    path: `/api/console/models/${modelId}/advisor-findings`,
    baseUrl
  });
}

export function fetchConsoleDataModelRecordPreview(
  modelCode: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleRuntimeRecordPreview>({
    path: appendRuntimeRecordsQuery(runtimeModelActionPath(modelCode, 'list'), {
      page: 1,
      page_size: 20
    }),
    baseUrl
  });
}

export function fetchConsoleRuntimeModelRecords(
  modelCode: string,
  input: FetchConsoleRuntimeModelRecordsInput = {},
  baseUrl?: string
) {
  return apiFetch<ConsoleRuntimeRecordPreview>({
    path: appendRuntimeRecordsQuery(
      runtimeModelActionPath(modelCode, 'list'),
      input
    ),
    baseUrl
  });
}

export function fetchConsoleRuntimeModelRecord(
  modelCode: string,
  recordId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleRuntimeModelRecord>({
    path: runtimeModelRecordActionPath(modelCode, 'get', recordId),
    baseUrl
  });
}

export function createConsoleRuntimeModelRecord(
  modelCode: string,
  input: ConsoleRuntimeModelRecordInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleRuntimeModelRecord>({
    path: runtimeModelActionPath(modelCode, 'create'),
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleRuntimeModelRecord(
  modelCode: string,
  recordId: string,
  input: ConsoleRuntimeModelRecordInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleRuntimeModelRecord>({
    path: runtimeModelRecordActionPath(modelCode, 'update', recordId),
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleRuntimeModelRecord(
  modelCode: string,
  recordId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<{ deleted: true }>({
    path: runtimeModelRecordActionPath(modelCode, 'delete', recordId),
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function fetchConsoleDataModelOpenApiDocument(
  modelId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleDataModelOpenApiDocument>({
    path: `/api/console/docs/data-models/${modelId}/openapi.json`,
    baseUrl
  });
}
