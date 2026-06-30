export const FLOW_SCHEMA_VERSION = '1flowbase.flow/v2';
export const NODE_CONTRIBUTION_SCHEMA_VERSION =
  '1flowbase.node-contribution/v2';

export type BuiltinFlowNodeType =
  | 'start'
  | 'answer'
  | 'workflow_start'
  | 'workflow_end'
  | 'llm'
  | 'knowledge_retrieval'
  | 'question_classifier'
  | 'if_else'
  | 'code'
  | 'template_transform'
  | 'http_request'
  | 'tool'
  | 'tool_result'
  | 'data_model_list'
  | 'data_model_get'
  | 'data_model_create'
  | 'data_model_update'
  | 'data_model_delete'
  | 'variable_assigner'
  | 'parameter_extractor'
  | 'iteration'
  | 'loop'
  | 'human_input';

export type FlowNodeType =
  | BuiltinFlowNodeType
  | 'plugin_node'
  | 'unresolved_node';

export type FlowStartInputType =
  | 'text'
  | 'paragraph'
  | 'select'
  | 'number'
  | 'checkbox'
  | 'file'
  | 'file_list'
  | 'url';

export interface FlowStartInputField {
  key: string;
  label: string;
  inputType: FlowStartInputType;
  valueType: string;
  required: boolean;
  placeholder?: string;
  defaultValue?: string | number | boolean;
  maxLength?: number;
  hidden?: boolean;
  options?: string[];
}

export interface FlowStartModelDescriptor {
  id: string;
  name?: string;
  context_window?: number;
  max_context_window?: number;
  max_output_tokens?: number;
  auto_compact_token_limit?: number;
  capabilities?: FlowStartModelCapabilities;
  reasoning?: FlowStartModelReasoning;
}

export interface FlowStartModelCapabilities {
  reasoning?: boolean;
  tool_call?: boolean;
  multimodal?: boolean;
  structured_output?: boolean;
}

export interface FlowStartModelReasoning {
  default_effort?: string;
  supported_efforts?: string[];
}

export interface FlowNodeOutputDocument {
  key: string;
  title: string;
  valueType: string;
  description?: string;
  selector?: string[];
  jsonSchema?: Record<string, unknown>;
}

export type PublicOutputKeyValidationResult =
  | { ok: true }
  | { ok: false; reason: 'reserved_public_output_key' };

export const RESERVED_PUBLIC_OUTPUT_KEYS = [] as const;

export function validatePublicOutputKey(
  key: string
): PublicOutputKeyValidationResult {
  if (key.startsWith('__')) {
    return { ok: false, reason: 'reserved_public_output_key' };
  }

  return { ok: true };
}

export function isValidPublicOutputKey(key: string): boolean {
  return validatePublicOutputKey(key).ok;
}

export type NodeRuntimeSideEffectPolicy =
  | 'none'
  | 'external_read'
  | 'external_write'
  | 'durable_write';

export interface NodeRuntimeContractMeta {
  type: FlowNodeType;
  title: string;
  description?: string;
  schemaVersion?: typeof NODE_CONTRIBUTION_SCHEMA_VERSION;
  contributionRef?: FlowPluginContributionRef;
}

export interface NodeRuntimeContractDefaults {
  alias: string;
  description?: string;
  configVersion: number;
  config: Record<string, unknown>;
  bindings: Record<string, FlowBinding>;
  outputs: FlowNodeOutputDocument[];
}

export interface NodeRuntimePortDocument {
  key: string;
  title: string;
  description?: string;
}

export interface NodeRuntimeContractPorts {
  inputs: NodeRuntimePortDocument[];
  outputs: NodeRuntimePortDocument[];
}

export interface NodeRuntimeContractCard {
  title: string;
  description?: string;
  icon?: string;
  category?: string;
}

export interface NodeRuntimePanelFieldDocument {
  key: string;
  title: string;
  valueType: string;
  renderer: string;
  required?: boolean;
  description?: string;
  options?: unknown[];
  min?: number;
  max?: number;
  step?: number;
  numberFormat?: 'bytes_as_mib';
}

export interface NodeRuntimePanelSectionDocument {
  key?: string;
  title?: string;
  fields?: NodeRuntimePanelFieldDocument[];
  blocks?: Array<Record<string, unknown>>;
}

export interface NodeRuntimeContractPanel {
  sections: NodeRuntimePanelSectionDocument[];
}

export interface NodeRuntimeDisplaySchemaDocument {
  key: string;
  title: string;
  valueType: string;
  description?: string;
}

export interface NodeRuntimeContractRuntime {
  inputs?: NodeRuntimeDisplaySchemaDocument[];
  processData?: NodeRuntimeDisplaySchemaDocument[];
  outputs: FlowNodeOutputDocument[];
}

export interface NodeRuntimeContractPolicies {
  sideEffect: NodeRuntimeSideEffectPolicy;
  timeoutMs?: number;
  retry?: {
    maxAttempts: number;
  };
  errorHandling?: {
    mode: 'fail' | 'continue';
  };
  singleRunForm?: {
    enabled: boolean;
  };
}

export interface NodeRuntimeUiContract {
  meta: NodeRuntimeContractMeta;
  defaults: NodeRuntimeContractDefaults;
  ports: NodeRuntimeContractPorts;
  card: NodeRuntimeContractCard;
  panel: NodeRuntimeContractPanel;
  runtime: NodeRuntimeContractRuntime;
  policies: NodeRuntimeContractPolicies;
}

export const DEFAULT_LLM_NODE_OUTPUTS = [
  { key: 'text', title: '模型输出', valueType: 'string' },
  { key: 'usage', title: '用量', valueType: 'json' }
] satisfies FlowNodeOutputDocument[];

export const DEFAULT_START_NODE_CONFIG = {
  input_fields: [] as unknown[],
  model_list: [] as unknown[]
} satisfies Record<string, unknown>;

export const DEFAULT_WORKFLOW_SYNC_TIMEOUT_MS = 30000;

export const DEFAULT_WORKFLOW_START_NODE_CONFIG = {
  input_fields: [] as unknown[],
  sync_timeout_ms: DEFAULT_WORKFLOW_SYNC_TIMEOUT_MS
} satisfies Record<string, unknown>;

export const DEFAULT_ANSWER_NODE_OUTPUTS = [
  { key: 'answer', title: '对话输出', valueType: 'string' }
] satisfies FlowNodeOutputDocument[];

export const LLM_STRUCTURED_OUTPUT = {
  key: 'structured_output',
  title: '结构化输出',
  valueType: 'json'
} satisfies FlowNodeOutputDocument;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function getLlmNodeOutputs(
  config?: Record<string, unknown>
): FlowNodeOutputDocument[] {
  const responseFormat = config?.response_format;
  const mode = isRecord(responseFormat) ? responseFormat.mode : undefined;
  const outputs = DEFAULT_LLM_NODE_OUTPUTS.map((output) => ({ ...output }));

  if (mode === 'json_object' || mode === 'json_schema') {
    outputs.push({ ...LLM_STRUCTURED_OUTPUT });
  }

  return outputs;
}

export interface FlowPluginContributionRef {
  plugin_id: string;
  plugin_version: string;
  contribution_code: string;
  node_shell: string;
  schema_version: string;
  plugin_unique_identifier: string;
  package_id: string;
  contribution_checksum: string;
  compiled_contribution_hash: string;
  output_schema_snapshot: FlowPluginContributionOutputSchemaSnapshot;
}

export interface FlowPluginContributionOutputSchemaSnapshot {
  outputs?: Array<Record<string, unknown>>;
  [key: string]: unknown;
}

export type LlmPromptMessageRole = 'system' | 'user' | 'assistant';

export interface LlmPromptMessage {
  id: string;
  role: LlmPromptMessageRole;
  content: {
    kind: 'templated_text';
    value: string;
  };
}

export interface FlowLlmVisibleInternalToolDocument {
  type: 'visible_internal_llm_tool';
  tool_name: string;
  connector_id?: string;
  target_node_id: string;
  description?: string;
  input_schema?: Record<string, unknown>;
  preconditions?: Array<Record<string, unknown>>;
  tool_mode?: 'agent' | 'fusion';
  internal_llm_node_policy?: 'forbidden' | 'allowed';
  external_tool_policy?: 'forbidden' | 'inherited';
}

export type DataModelQueryOperator = 'eq' | 'ne' | 'gt' | 'gte' | 'lt' | 'lte';

export type DataModelQueryValue =
  | { kind: 'constant'; value: unknown }
  | { kind: 'selector'; selector: string[] };

export interface DataModelQueryFilter {
  field_code: string;
  operator: DataModelQueryOperator;
  value: DataModelQueryValue;
}

export interface DataModelQuerySort {
  field_code: string;
  direction: 'asc' | 'desc';
}

export interface DataModelQueryBindingValue {
  filters: DataModelQueryFilter[];
  sorts: DataModelQuerySort[];
  expand_relations: string[];
  page: DataModelQueryValue;
  page_size: DataModelQueryValue;
}

export type NamedBindingExpression =
  | { kind: 'selector'; selector: string[] }
  | { kind: 'constant'; value: unknown }
  | { kind: 'templated_text'; value: string };

export type StateWriteValueExpression = NamedBindingExpression;

export interface NamedBindingEntry {
  name: string;
  valueType?: string;
  value?: NamedBindingExpression;
  selector?: string[];
  content?: { kind: 'templated_text'; value: string };
}

export type FlowConditionComparator =
  | 'exists'
  | 'empty'
  | 'equals'
  | 'not_equals'
  | 'greater_than'
  | 'greater_than_or_equals'
  | 'less_than'
  | 'less_than_or_equals'
  | 'contains'
  | 'starts_with'
  | 'ends_with'
  | 'matches_regex';

export type FlowConditionValue =
  | { kind: 'constant'; value: unknown }
  | { kind: 'selector'; selector: string[] };

export interface FlowConditionRuleDocument {
  kind?: 'rule';
  left: string[];
  comparator: FlowConditionComparator;
  right?: FlowConditionValue;
}

export interface FlowConditionGroupDocument {
  kind?: 'group';
  operator: 'and' | 'or';
  conditions: FlowConditionExpressionDocument[];
}

export type FlowConditionExpressionDocument =
  | FlowConditionRuleDocument
  | FlowConditionGroupDocument;

export type IfElseBranchKind = 'if' | 'else_if' | 'else';

export interface IfElseBranchDocument {
  id: string;
  kind: IfElseBranchKind;
  title: string;
  sourceHandle: string;
  condition?: FlowConditionGroupDocument;
}

export type FlowBinding =
  | { kind: 'templated_text'; value: string }
  | { kind: 'selector'; value: string[] }
  | { kind: 'selector_list'; value: string[][] }
  | {
      kind: 'data_model_query';
      value: DataModelQueryBindingValue;
    }
  | {
      kind: 'prompt_messages';
      value: LlmPromptMessage[];
    }
  | {
      kind: 'named_bindings';
      value: NamedBindingEntry[];
    }
  | {
      kind: 'condition_group';
      value: FlowConditionGroupDocument;
    }
  | {
      kind: 'if_else_branches';
      value: {
        branches: IfElseBranchDocument[];
      };
    }
  | {
      kind: 'state_write';
      value: Array<{
        path: string[];
        operator: 'set' | 'append' | 'clear' | 'increment';
        source?: string[] | null;
        value?: StateWriteValueExpression | null;
      }>;
    };

export interface FlowNodeDocument {
  id: string;
  type: FlowNodeType;
  plugin_id?: string;
  plugin_version?: string;
  contribution_code?: string;
  node_shell?: string;
  schema_version?: string;
  plugin_unique_identifier?: string;
  package_id?: string;
  contribution_checksum?: string;
  compiled_contribution_hash?: string;
  output_schema_snapshot?: FlowPluginContributionOutputSchemaSnapshot;
  alias: string;
  description?: string;
  containerId: string | null;
  position: { x: number; y: number };
  configVersion: number;
  config: Record<string, unknown>;
  bindings: Record<string, FlowBinding>;
  outputs: FlowNodeOutputDocument[];
}

export interface FlowEdgeDocument {
  id: string;
  source: string;
  target: string;
  sourceHandle: string | null;
  targetHandle: string | null;
  containerId: string | null;
  points: Array<{ x: number; y: number }>;
}

export interface FlowAnnotationDocument {
  id: string;
  kind: 'note';
  text: string;
  position: { x: number; y: number };
}

export interface FlowConversationVariableDocument {
  name: string;
  valueType: string;
  description?: string;
}

export interface FlowVariableRegistryDocument {
  conversation: FlowConversationVariableDocument[];
}

export interface FlowAuthoringDocument {
  schemaVersion: typeof FLOW_SCHEMA_VERSION;
  meta: {
    flowId: string;
    name: string;
    description: string;
    tags: string[];
  };
  graph: {
    nodes: FlowNodeDocument[];
    edges: FlowEdgeDocument[];
  };
  variables?: FlowVariableRegistryDocument;
  editor: {
    viewport: { x: number; y: number; zoom: number };
    annotations: FlowAnnotationDocument[];
    activeContainerPath: string[];
  };
}

export function createDefaultAgentFlowDocument({
  flowId
}: {
  flowId: string;
}): FlowAuthoringDocument {
  return {
    schemaVersion: FLOW_SCHEMA_VERSION,
    meta: {
      flowId,
      name: 'Untitled agentFlow',
      description: '',
      tags: []
    },
    graph: {
      nodes: [
        {
          id: 'node-start',
          type: 'start',
          alias: 'Start',
          description: '',
          containerId: null,
          position: { x: 80, y: 220 },
          configVersion: 1,
          config: {
            input_fields: [...DEFAULT_START_NODE_CONFIG.input_fields],
            model_list: [...DEFAULT_START_NODE_CONFIG.model_list]
          },
          bindings: {},
          outputs: []
        },
        {
          id: 'node-llm',
          type: 'llm',
          alias: 'LLM',
          description: '',
          containerId: null,
          position: { x: 360, y: 220 },
          configVersion: 1,
          config: {
            model_provider: {
              provider_code: '',
              model_id: ''
            },
            llm_parameters: {
              schema_version: '1.0.0',
              items: {}
            },
            context_policy: {
              integration_context: 'enabled',
              context_selector: ['node-start', 'history']
            },
            external_reasoning_policy: {
              follow_external_reasoning: false
            },
            visible_internal_llm_tools_enabled: false,
            visible_internal_llm_tools: [],
            response_format: {
              mode: 'text'
            }
          },
          bindings: {
            prompt_messages: {
              kind: 'prompt_messages',
              value: [
                {
                  id: 'system-1',
                  role: 'system',
                  content: { kind: 'templated_text', value: '' }
                },
                {
                  id: 'user-1',
                  role: 'user',
                  content: {
                    kind: 'templated_text',
                    value: '{{node-start.query}}'
                  }
                }
              ]
            }
          },
          outputs: getLlmNodeOutputs({
            response_format: {
              mode: 'text'
            }
          })
        },
        {
          id: 'node-answer',
          type: 'answer',
          alias: 'Answer',
          description: '',
          containerId: null,
          position: { x: 640, y: 220 },
          configVersion: 1,
          config: {},
          bindings: {
            answer_template: {
              kind: 'templated_text',
              value: '{{node-llm.text}}'
            }
          },
          outputs: DEFAULT_ANSWER_NODE_OUTPUTS.map((output) => ({ ...output }))
        }
      ],
      edges: [
        {
          id: 'edge-start-llm',
          source: 'node-start',
          target: 'node-llm',
          sourceHandle: null,
          targetHandle: null,
          containerId: null,
          points: []
        },
        {
          id: 'edge-llm-answer',
          source: 'node-llm',
          target: 'node-answer',
          sourceHandle: null,
          targetHandle: null,
          containerId: null,
          points: []
        }
      ]
    },
    variables: {
      conversation: []
    },
    editor: {
      viewport: { x: 0, y: 0, zoom: 1 },
      annotations: [],
      activeContainerPath: []
    }
  };
}

export function createDefaultWorkflowDocument({
  flowId
}: {
  flowId: string;
}): FlowAuthoringDocument {
  return {
    schemaVersion: FLOW_SCHEMA_VERSION,
    meta: {
      flowId,
      name: 'Untitled workflow',
      description: '',
      tags: []
    },
    graph: {
      nodes: [
        {
          id: 'node-workflow-start',
          type: 'workflow_start',
          alias: 'Workflow Start',
          description: '',
          containerId: null,
          position: { x: 120, y: 220 },
          configVersion: 1,
          config: {
            input_fields: [...DEFAULT_WORKFLOW_START_NODE_CONFIG.input_fields],
            sync_timeout_ms: DEFAULT_WORKFLOW_START_NODE_CONFIG.sync_timeout_ms
          },
          bindings: {},
          outputs: []
        },
        {
          id: 'node-workflow-end',
          type: 'workflow_end',
          alias: 'Workflow End',
          description: '',
          containerId: null,
          position: { x: 440, y: 220 },
          configVersion: 1,
          config: {},
          bindings: {},
          outputs: []
        }
      ],
      edges: [
        {
          id: 'edge-workflow-start-end',
          source: 'node-workflow-start',
          target: 'node-workflow-end',
          sourceHandle: null,
          targetHandle: null,
          containerId: null,
          points: []
        }
      ]
    },
    variables: {
      conversation: []
    },
    editor: {
      viewport: { x: 0, y: 0, zoom: 1 },
      annotations: [],
      activeContainerPath: []
    }
  };
}

function omitKey<T extends object, K extends keyof T>(
  value: T,
  key: K
): Omit<T, K> {
  return Object.fromEntries(
    Object.entries(value).filter(([entryKey]) => entryKey !== String(key))
  ) as Omit<T, K>;
}

function stripLayout(document: FlowAuthoringDocument) {
  return {
    ...document,
    graph: {
      nodes: document.graph.nodes.map((node) => omitKey(node, 'position')),
      edges: document.graph.edges.map((edge) => omitKey(edge, 'points'))
    },
    editor: {
      ...document.editor,
      viewport: { x: 0, y: 0, zoom: 1 },
      annotations: document.editor.annotations.map((annotation) =>
        omitKey(annotation, 'position')
      )
    }
  };
}

export function classifyDocumentChange(
  before: FlowAuthoringDocument,
  after: FlowAuthoringDocument
): 'layout' | 'logical' {
  return JSON.stringify(stripLayout(before)) ===
    JSON.stringify(stripLayout(after))
    ? 'layout'
    : 'logical';
}
