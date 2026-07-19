export type SchemaRuleOperator = 'eq' | 'neq' | 'in' | 'truthy' | 'falsy' | 'hasCapability';

export interface SchemaRule {
  operator: SchemaRuleOperator;
  path?: string;
  value?: unknown;
  values?: readonly unknown[];
  capability?: string;
}

export interface SchemaBlockBase {
  visibleWhen?: SchemaRule;
}

export interface SchemaFieldBlock extends SchemaBlockBase {
  kind: 'field';
  renderer: string;
  path: string;
  label: string;
  options?: SchemaFieldOption[];
  min?: number;
  max?: number;
  step?: number;
  numberFormat?: 'bytes_as_mib';
  disabledWhen?: SchemaRule;
  requiredWhen?: SchemaRule;
}

export interface SchemaFieldOption {
  value: string;
  label: string;
  disabled?: boolean;
  disabledReason?: string | null;
}

export interface SchemaViewBlock extends SchemaBlockBase {
  kind: 'view';
  renderer: string;
  title?: string;
  key?: string;
}

export interface SchemaSectionBlock extends SchemaBlockBase {
  kind: 'section';
  title: string;
  blocks: SchemaBlock[];
}

export interface SchemaStackBlock extends SchemaBlockBase {
  kind: 'stack' | 'inline' | 'tabs';
  blocks: SchemaBlock[];
  title?: string;
}

export interface SchemaDynamicFormBlock extends SchemaBlockBase {
  kind: 'dynamic_form';
  form_key: 'provider_config' | 'llm_parameters' | string;
  title?: string;
  empty_text?: string;
}

export type SchemaBlock =
  | SchemaFieldBlock
  | SchemaViewBlock
  | SchemaSectionBlock
  | SchemaStackBlock
  | SchemaDynamicFormBlock;

export interface CanvasNodeSchema {
  schemaVersion: '1.0.0';
  nodeType: string;
  capabilities: string[];
  card: {
    blocks: SchemaBlock[];
  };
  detail: {
    header: {
      blocks: SchemaBlock[];
    };
    tabs: {
      config: {
        blocks: SchemaBlock[];
      };
      lastRun: {
        blocks: SchemaBlock[];
      };
    };
  };
  runtimeSlots: Record<string, string>;
}
