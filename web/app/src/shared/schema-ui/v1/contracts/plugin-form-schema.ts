export type PluginFormValue = string | number | boolean | null | PluginFormValue[] | {
  [key: string]: PluginFormValue;
};

export interface PluginFormOption {
  label: string;
  value: string | number | boolean;
  description?: string;
  disabled?: boolean;
}

export interface PluginFormCondition {
  field: string;
  operator: 'eq' | 'neq' | 'in' | 'truthy' | 'falsy' | string;
  value?: PluginFormValue;
  values?: PluginFormValue[];
}

export interface PluginFormFieldSchema {
  key: string;
  label: string;
  type: string;
  control?: string;
  group?: string;
  order?: number;
  advanced?: boolean;
  required?: boolean;
  read_only?: boolean;
  send_mode?: 'always' | 'optional' | string;
  enabled_by_default?: boolean;
  description?: string;
  placeholder?: string;
  default_value?: PluginFormValue;
  pattern?: string;
  min?: number;
  max?: number;
  step?: number;
  precision?: number;
  unit?: string;
  options?: PluginFormOption[];
  visible_when?: PluginFormCondition[];
  disabled_when?: PluginFormCondition[];
}

export interface PluginFormSchema {
  schema_version: '1.0.0' | string;
  title?: string;
  description?: string;
  fields: PluginFormFieldSchema[];
}
