import { apiFetch } from '../../transport';

export interface ConsoleFileStorage {
  id: string;
  code: string;
  title: string;
  driver_type: string;
  enabled: boolean;
  is_default: boolean;
  health_status: string;
  last_health_error: string | null;
  config_json: Record<string, unknown>;
  rule_json: Record<string, unknown>;
}

export interface CreateConsoleFileStorageInput {
  code: string;
  title: string;
  driver_type: string;
  enabled: boolean;
  is_default: boolean;
  config_json: Record<string, unknown>;
  rule_json: Record<string, unknown>;
}

export interface ConsoleFileTable {
  id: string;
  code: string;
  title: string;
  scope_kind: 'system' | 'workspace';
  scope_id: string;
  model_definition_id: string;
  bound_storage_id: string | null;
  bound_storage_title: string | null;
  is_builtin: boolean;
  is_default: boolean;
  status: string;
}

export interface CreateConsoleFileTableInput {
  code: string;
  title: string;
  bound_storage_id?: string | null;
}

export interface UpdateConsoleFileTableBindingInput {
  bound_storage_id: string | null;
}

export interface UploadedConsoleFile {
  file_table_id: string;
  storage_id: string;
  record: Record<string, unknown>;
}

export interface ConsoleFileRecord extends Record<string, unknown> {
  id: string;
  title: string | null;
  filename: string;
  size: number;
  mimetype: string;
  created_at: string;
  storage_id: string;
  url: string;
  preview_url: string | null;
}

export interface FileShareLink {
  url: string;
  preview_url: string | null;
  expires_at: string;
}

export function createConsoleFileShareLink(
  fileTableId: string,
  recordId: string,
  input: { expires_in_seconds: number },
  csrfToken: string,
  signal?: AbortSignal
) {
  return apiFetch<FileShareLink>({
    path: `/api/console/files/${encodeURIComponent(fileTableId)}/records/${encodeURIComponent(recordId)}/access-links`,
    method: 'POST',
    body: input,
    csrfToken,
    signal
  });
}

export function fetchConsoleFileRecords(
  modelCode: string,
  page: number,
  pageSize: number,
  signal?: AbortSignal,
  options: { filename?: string; sort?: string } = {}
) {
  const query = new URLSearchParams({
    page: String(page),
    page_size: String(pageSize),
    sort: options.sort ?? 'created_at:desc'
  });
  if (options.filename) {
    query.set(
      'filter',
      JSON.stringify({ filename: { $includes: options.filename } })
    );
  }
  return apiFetch<{ items: ConsoleFileRecord[]; total: number }>({
    path: `/api/runtime/models/${encodeURIComponent(modelCode)}/list?${query}`,
    signal
  });
}

export function consoleFileContentPath(fileTableId: string, recordId: string) {
  return `/api/console/files/${encodeURIComponent(fileTableId)}/records/${encodeURIComponent(recordId)}/content`;
}

export function uploadConsoleFile(
  file: File,
  fileTableId: string,
  csrfToken: string,
  signal?: AbortSignal
) {
  const formData = new FormData();
  formData.append('file_table_id', fileTableId);
  formData.append('file', file);
  return apiFetch<UploadedConsoleFile>({
    path: '/api/console/files/upload',
    method: 'POST',
    rawBody: formData,
    csrfToken,
    signal
  });
}

export interface UpdateConsoleFileStorageInput {
  title?: string;
  enabled?: boolean;
  is_default?: boolean;
  config_json?: Record<string, unknown>;
  rule_json?: Record<string, unknown>;
}

export interface UpdateConsoleFileTableInput {
  title?: string;
}

export function fetchConsoleFileStorages(baseUrl?: string) {
  return apiFetch<ConsoleFileStorage[]>({
    path: '/api/console/settings/files/storages',
    baseUrl
  });
}

export function createConsoleFileStorage(
  input: CreateConsoleFileStorageInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleFileStorage>({
    path: '/api/console/settings/files/storages',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function fetchConsoleFileTables(baseUrl?: string) {
  return apiFetch<ConsoleFileTable[]>({
    path: '/api/console/settings/files/tables',
    baseUrl
  });
}

export function createConsoleFileTable(
  input: CreateConsoleFileTableInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleFileTable>({
    path: '/api/console/settings/files/tables',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleFileTableBinding(
  fileTableId: string,
  input: UpdateConsoleFileTableBindingInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleFileTable>({
    path: `/api/console/settings/files/tables/${fileTableId}/binding`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleFileStorage(
  fileStorageId: string,
  input: UpdateConsoleFileStorageInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleFileStorage>({
    path: `/api/console/settings/files/storages/${fileStorageId}`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleFileStorage(
  fileStorageId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<void>({
    path: `/api/console/settings/files/storages/${fileStorageId}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleFileTable(
  fileTableId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<void>({
    path: `/api/console/settings/files/tables/${fileTableId}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}
