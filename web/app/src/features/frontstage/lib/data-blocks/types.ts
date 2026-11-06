import type { BlockContext } from '@1flowbase/page-protocol';
import type { RuntimeDataModelMetadata, RuntimeDataModelField } from '@1flowbase/api-client';

export type DataBlockMetadata = RuntimeDataModelMetadata;
export type DataBlockField = RuntimeDataModelField;
export interface DataBlockComponentProps { ctx: BlockContext; }

export interface DataBlockBinding {
  model_id: string;
  model_code: string;
  data_source_id: string | null;
}

export interface DataBlockRelation {
  field_code: string;
  parent_model_code: string;
  parent_record_input: string;
}

export type DataBlockPresentation = 'drawer' | 'modal' | 'page';
export type DataBlockFormMode = 'create' | 'edit' | 'view';
export interface DataBlockAction {
  id: string;
  kind: 'filter' | 'create' | 'delete' | 'refresh' | 'view' | 'edit' | 'open';
  enabled: boolean;
  title?: string;
  target_block_id?: string;
  presentation?: DataBlockPresentation;
  size?: 'small' | 'middle' | 'large';
}

export interface DataBlockColumn {
  field_code: string;
  title?: string;
  tooltip?: string;
  width?: number;
  fixed?: 'left' | 'right';
  sortable?: boolean;
  ellipsis?: boolean;
  hidden?: boolean;
  action?: DataBlockAction;
}

export type DataFilterOperator = '$eq' | '$ne' | '$includes' | '$notIncludes' | '$gt' | '$gte' | '$lt' | '$lte' | '$in';
export interface DataFilterCondition {
  id: string;
  field_code: string;
  operator: DataFilterOperator;
  value?: unknown;
}
export interface DataFilterGroup {
  id: string;
  conjunction: 'and' | 'or';
  conditions: Array<DataFilterCondition | DataFilterGroup>;
}

export interface DataBlockLinkage {
  source_block_id: string;
  source_output: 'record_id' | 'selected_record_ids';
  field_code: string;
}

interface DataBlockCommon {
  version: 1;
  binding: DataBlockBinding;
  title: string;
  description: string;
  relation?: DataBlockRelation;
}

export interface TableBlockConfig extends DataBlockCommon {
  kind: 'table';
  columns: DataBlockColumn[];
  actions: DataBlockAction[];
  row_actions: DataBlockAction[];
  page_size: number;
  density: 'small' | 'middle' | 'large';
  show_index: boolean;
  height: number | null;
  filter: DataFilterGroup;
  sort: { field: string; direction: 'asc' | 'desc' } | null;
  linkage?: DataBlockLinkage;
}

export interface DataFormItem {
  id: string;
  kind: 'field' | 'group' | 'markdown';
  field_code?: string;
  title?: string;
  description?: string;
  tooltip?: string;
  required?: boolean;
  read_only?: boolean;
  default_value?: unknown;
  row: string;
  span: number;
  text?: string;
}

export interface DataFormRule {
  id: string;
  condition: DataFilterGroup;
  target_field: string;
  effect: 'show' | 'hide' | 'required' | 'optional' | 'readonly' | 'editable';
}

export interface FormBlockConfig extends DataBlockCommon {
  kind: 'form';
  mode: DataBlockFormMode;
  items: DataFormItem[];
  rules: DataFormRule[];
  label_layout: 'vertical' | 'horizontal';
  label_align: 'left' | 'right';
  label_width: number;
  label_wrap: boolean;
  colon: boolean;
  submit_enabled: boolean;
  submit_title: string;
  close_after_submit: boolean;
  source_block_id?: string;
  source_model_code?: string;
  surface_size?: 'small' | 'middle' | 'large';
}

export type DataBlockConfig = TableBlockConfig | FormBlockConfig;

export const TABLE_BLOCK_CONTRIBUTION = 'frontstage.table-block';
export const FORM_BLOCK_CONTRIBUTION = 'frontstage.form-block';

export function emptyDataFilter(): DataFilterGroup {
  return { id: 'root', conjunction: 'and', conditions: [] };
}

export function createTableBlockConfig(binding: DataBlockBinding): TableBlockConfig {
  return { version: 1, kind: 'table', binding, title: '', description: '', columns: [], actions: [], row_actions: [], page_size: 20, density: 'small', show_index: true, height: null, filter: emptyDataFilter(), sort: null };
}

export function createFormBlockConfig(binding: DataBlockBinding, mode: DataBlockFormMode = 'create'): FormBlockConfig {
  return { version: 1, kind: 'form', binding, title: '', description: '', mode, items: [], rules: [], label_layout: 'vertical', label_align: 'left', label_width: 120, label_wrap: true, colon: true, submit_enabled: false, submit_title: '', close_after_submit: true };
}

export function readDataBlockConfig(props: Readonly<Record<string, unknown>>): DataBlockConfig | null {
  const value = props.data_block;
  if (!value || typeof value !== 'object') return null;
  const candidate = value as Partial<DataBlockConfig>;
  if (candidate.version !== 1 || !candidate.binding?.model_id || !candidate.binding.model_code) return null;
  if (candidate.kind === 'table' && Array.isArray(candidate.columns) && Array.isArray(candidate.actions) && Array.isArray(candidate.row_actions)) return candidate as TableBlockConfig;
  if (candidate.kind === 'form' && Array.isArray(candidate.items) && Array.isArray(candidate.rules)) return candidate as FormBlockConfig;
  return null;
}
