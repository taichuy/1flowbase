import { apiFetch } from '../../transport';
import type { ConsolePolicyCatalogResource } from './roles';

export interface RecordPermissionKey {
  plugin_code: string;
  resource_code: string;
  record_id: string;
}

export type RecordPermissionMode = 'inherit' | 'restricted';

export interface RecordPermissionGrant {
  role_id: string;
  actions: string[];
}

export interface RecordPermissionRole {
  role_id: string;
  code: string;
  name: string;
}

export interface RecordPermissionPolicy {
  mode: RecordPermissionMode;
  grants: RecordPermissionGrant[];
  revision: number;
}

export interface RecordPermissionEditorData extends RecordPermissionPolicy {
  resource: ConsolePolicyCatalogResource;
  roles: RecordPermissionRole[];
}

export interface ReplaceRecordPermissionsInput {
  mode: RecordPermissionMode;
  grants: RecordPermissionGrant[];
  expected_revision?: number;
}

export interface RecordPermissionCapabilities {
  record_id: string;
  allowed_actions: string[];
  can_manage_permissions: boolean;
}

export interface RecordCapabilitiesInput {
  plugin_code: string;
  resource_code: string;
  record_ids: string[];
}

export interface RecordCapabilitiesResponse {
  items: RecordPermissionCapabilities[];
}

function recordPermissionsPath(key: RecordPermissionKey): string {
  const query = new URLSearchParams({ ...key });
  return `/api/console/settings/roles/record-permissions?${query.toString()}`;
}

export function fetchRecordPermissions(
  key: RecordPermissionKey,
  baseUrl?: string,
  signal?: AbortSignal
): Promise<RecordPermissionEditorData> {
  return apiFetch({ path: recordPermissionsPath(key), baseUrl, signal });
}

export function replaceRecordPermissions(
  key: RecordPermissionKey,
  input: ReplaceRecordPermissionsInput,
  csrfToken: string | null,
  baseUrl?: string
): Promise<RecordPermissionPolicy> {
  return apiFetch({
    path: recordPermissionsPath(key),
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function fetchRecordCapabilities(
  input: RecordCapabilitiesInput,
  csrfToken: string | null,
  baseUrl?: string,
  signal?: AbortSignal
): Promise<RecordCapabilitiesResponse> {
  return apiFetch({
    path: '/api/console/settings/roles/record-capabilities',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl,
    signal
  });
}
