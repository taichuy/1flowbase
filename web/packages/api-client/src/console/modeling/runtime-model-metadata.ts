import { apiFetch } from '../../transport';

export interface RuntimeDataModelField {
  id: string;
  code: string;
  title: string;
  description: string | null;
  field_kind: string;
  is_required: boolean;
  api_required: boolean;
  is_writable: boolean;
  default_value: unknown | null;
  display_interface: string | null;
  display_options: unknown;
  relation_target_model_id: string | null;
  relation_target_model_code: string | null;
  relation_options: unknown;
}

export interface RuntimeDataModelMetadata {
  model_id: string;
  model_code: string;
  scope_kind: 'workspace' | 'system';
  scope_id: string;
  record_id_field: string | null;
  fields: RuntimeDataModelField[];
  record_capabilities: {
    can_list: boolean;
    can_get: boolean;
    can_create: boolean;
    can_update: boolean;
    can_delete: boolean;
  };
}

export function fetchRuntimeDataModelMetadata(modelCode: string, baseUrl?: string) {
  return apiFetch<RuntimeDataModelMetadata>({
    path: `/api/runtime/models/${encodeURIComponent(modelCode)}/metadata`,
    baseUrl
  });
}
