import type { FlowNodeType } from '@1flowbase/flow-schema';
import type {
  SchemaFieldOption,
  SchemaRule
} from '../../../../shared/schema-ui/contracts/canvas-node-schema';

export type InspectorSectionKey =
  | 'basics'
  | 'inputs'
  | 'outputs'
  | 'policy'
  | 'advanced';

export type NodeEditorKind =
  | 'text'
  | 'static_select'
  | 'data_model'
  | 'data_model_query'
  | 'llm_model'
  | 'llm_context_policy'
  | 'llm_external_reasoning_policy'
  | 'llm_tool_registrations'
  | 'llm_prompt_messages'
  | 'llm_response_format'
  | 'code_source'
  | 'number'
  | 'selector'
  | 'selector_list'
  | 'templated_text'
  | 'named_bindings'
  | 'templated_named_bindings'
  | 'condition_group'
  | 'if_else_branches'
  | 'state_write'
  | 'variable_assignment'
  | 'output_contract_definition'
  | 'start_input_fields'
  | 'start_model_list';

export interface NodeDefinitionField {
  key: string;
  label: string;
  editor: NodeEditorKind;
  required?: boolean;
  options?: SchemaFieldOption[];
  visibleWhen?: SchemaRule;
}

export interface NodeDefinitionSection {
  key: InspectorSectionKey;
  title: string;
  fields: NodeDefinitionField[];
}

export interface NodeDefinition {
  label: string;
  summary?: string;
  helpHref?: string | null;
  canEnterContainer?: boolean;
  sections: NodeDefinitionSection[];
}

export interface NodeDefinitionMeta {
  summary: string;
  helpHref: string | null;
  canEnterContainer?: boolean;
}

export type NodeDefinitionMap = Partial<Record<FlowNodeType, NodeDefinition>>;
export type NodeDefinitionMetaMap = Record<
  Exclude<FlowNodeType, 'workflow_start' | 'workflow_end'>,
  NodeDefinitionMeta
>;
