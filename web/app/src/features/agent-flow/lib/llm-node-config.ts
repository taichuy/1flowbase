import type {
  ConsolePluginFormFieldSchema,
  ConsolePluginFormSchema
} from '@1flowbase/api-client';
import {
  DEFAULT_LLM_PROTOCOL_CONTEXT_REFERENCE,
  type FlowVariableReference
} from '@1flowbase/flow-schema';

export interface LlmNodeModelProvider {
  provider_code: string;
  model_id: string;
  protocol?: string;
  provider_label?: string;
  model_label?: string;
  schema_fetched_at?: string;
}

export interface LlmParameterItem {
  enabled: boolean;
  value: unknown;
}

export interface LlmNodeParameters {
  schema_version: string;
  items: Record<string, LlmParameterItem>;
}

export interface LlmNodeResponseFormat {
  mode: 'text' | 'json_object' | 'json_schema';
  schema?: Record<string, unknown>;
}

export interface LlmNodeContextPolicy {
  integration_context: 'enabled' | 'disabled';
  context_selector?: string[];
}

export interface LlmNodeExternalReasoningPolicy {
  follow_external_reasoning: boolean;
}

export interface LlmNodeExternalModelParameterPolicy {
  follow_external_max_output_tokens: boolean;
}

export type LlmInternalLlmNodePolicy = 'forbidden' | 'allowed';

export type LlmExternalToolPolicy = 'forbidden' | 'inherited';

export type LlmToolMode = 'agent' | 'fusion';

export interface LlmVisibleInternalTool {
  type: 'visible_internal_llm_tool';
  tool_name: string;
  connector_id?: string;
  target_node_id: string;
  description?: string;
  input_schema?: Record<string, unknown>;
  preconditions?: Array<Record<string, unknown>>;
  tool_mode?: LlmToolMode;
  internal_llm_node_policy?: LlmInternalLlmNodePolicy;
  external_tool_policy?: LlmExternalToolPolicy;
}

export const DEFAULT_LLM_CONTEXT_POLICY: LlmNodeContextPolicy = {
  integration_context: 'enabled',
  context_selector: ['node-start', 'history']
};

export const DEFAULT_LLM_EXTERNAL_REASONING_POLICY: LlmNodeExternalReasoningPolicy =
  {
    follow_external_reasoning: false
  };

export const DEFAULT_LLM_EXTERNAL_MODEL_PARAMETER_POLICY: LlmNodeExternalModelParameterPolicy =
  {
    follow_external_max_output_tokens: true
  };

export const DEFAULT_LLM_PARAMETERS: LlmNodeParameters = {
  schema_version: '1.0.0',
  items: {}
};

export const DEFAULT_LLM_RESPONSE_FORMAT: LlmNodeResponseFormat = {
  mode: 'text'
};

export const DEFAULT_LLM_VISIBLE_INTERNAL_TOOLS_ENABLED = false;
export const DEFAULT_LLM_VISIBLE_INTERNAL_TOOLS: LlmVisibleInternalTool[] = [];
export const DEFAULT_LLM_MCP_INSTANCE_IDS: string[] = [];
export const DEFAULT_LLM_INTERNAL_LLM_NODE_POLICY: LlmInternalLlmNodePolicy =
  'forbidden';
export const DEFAULT_LLM_EXTERNAL_TOOL_POLICY: LlmExternalToolPolicy =
  'forbidden';
export const DEFAULT_LLM_TOOL_MODE: LlmToolMode = 'agent';
export const LLM_TOOL_IDENTIFIER_MAX_LENGTH = 64;
const LLM_TOOL_SOURCE_HANDLE_PREFIX = 'visible_internal_llm_tool:';

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function asString(value: unknown) {
  return typeof value === 'string' ? value : '';
}

function recordArray(value: unknown): Array<Record<string, unknown>> {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.filter(isRecord).map((item) => ({ ...item }));
}

export function createLlmToolSourceHandleId(connectorId: string) {
  return `${LLM_TOOL_SOURCE_HANDLE_PREFIX}${connectorId}`;
}

export function parseLlmToolSourceHandleId(
  handleId: string | null | undefined
) {
  if (!handleId?.startsWith(LLM_TOOL_SOURCE_HANDLE_PREFIX)) {
    return null;
  }

  return handleId.slice(LLM_TOOL_SOURCE_HANDLE_PREFIX.length);
}

export function isLlmToolSourceHandle(handleId: string | null | undefined) {
  return parseLlmToolSourceHandleId(handleId) !== null;
}

export function isLlmToolIdentifier(value: string) {
  return (
    value.length > 0 &&
    value.length <= LLM_TOOL_IDENTIFIER_MAX_LENGTH &&
    /^[A-Za-z0-9_]+$/.test(value)
  );
}

export function getLlmParameterDefaultValue(
  field: ConsolePluginFormFieldSchema
): unknown {
  if (field.default_value !== undefined) {
    return field.default_value;
  }

  switch (field.type) {
    case 'boolean':
      return false;
    case 'integer':
    case 'number':
      return 0;
    case 'string_list':
      return [];
    case 'json':
      return {};
    default:
      return '';
  }
}

export function buildLlmParameterState(
  schema?: ConsolePluginFormSchema | null
): LlmNodeParameters {
  if (!schema) {
    return DEFAULT_LLM_PARAMETERS;
  }

  return {
    schema_version: schema.schema_version,
    items: Object.fromEntries(
      schema.fields.map((field) => {
        const enabled =
          field.send_mode === 'always'
            ? true
            : Boolean(field.enabled_by_default);

        return [
          field.key,
          {
            enabled,
            value: getLlmParameterDefaultValue(field)
          } satisfies LlmParameterItem
        ];
      })
    )
  };
}

export function resolveLlmParameterStateOnModelChange({
  currentProviderCode,
  nextProviderCode,
  currentParameters,
  nextSchema
}: {
  currentProviderCode: string;
  nextProviderCode: string;
  currentParameters: LlmNodeParameters;
  nextSchema?: ConsolePluginFormSchema | null;
}) {
  if (
    currentProviderCode.trim().length > 0 &&
    currentProviderCode.trim() === nextProviderCode.trim()
  ) {
    return currentParameters;
  }

  return buildLlmParameterState(nextSchema);
}

export function getLlmModelProvider(
  config: Record<string, unknown>
): LlmNodeModelProvider {
  const provider = config.model_provider;

  if (!isRecord(provider)) {
    return {
      provider_code: '',
      model_id: '',
      protocol: undefined,
      provider_label: undefined,
      model_label: undefined,
      schema_fetched_at: undefined
    };
  }

  return {
    provider_code: asString(provider.provider_code),
    model_id: asString(provider.model_id),
    protocol: asString(provider.protocol) || undefined,
    provider_label: asString(provider.provider_label) || undefined,
    model_label: asString(provider.model_label) || undefined,
    schema_fetched_at: asString(provider.schema_fetched_at) || undefined
  };
}

export function getLlmParameters(
  config: Record<string, unknown>
): LlmNodeParameters {
  const llmParameters = config.llm_parameters;

  if (isRecord(llmParameters)) {
    const items = isRecord(llmParameters.items) ? llmParameters.items : {};

    return {
      schema_version: asString(llmParameters.schema_version) || '1.0.0',
      items: Object.fromEntries(
        Object.entries(items).map(([key, item]) => {
          if (!isRecord(item)) {
            return [
              key,
              { enabled: false, value: item } satisfies LlmParameterItem
            ];
          }

          return [
            key,
            {
              enabled: Boolean(item.enabled),
              value: item.value
            } satisfies LlmParameterItem
          ];
        })
      )
    };
  }

  return DEFAULT_LLM_PARAMETERS;
}

export function getLlmContextPolicy(
  config: Record<string, unknown>
): LlmNodeContextPolicy {
  const contextPolicy = config.context_policy;

  if (!isRecord(contextPolicy)) {
    return DEFAULT_LLM_CONTEXT_POLICY;
  }

  return {
    integration_context:
      contextPolicy.integration_context === 'disabled' ? 'disabled' : 'enabled',
    context_selector: Array.isArray(contextPolicy.context_selector)
      ? contextPolicy.context_selector.filter(
          (segment): segment is string => typeof segment === 'string'
        )
      : DEFAULT_LLM_CONTEXT_POLICY.context_selector
  };
}

export function getLlmProtocolContextReference(
  config: Record<string, unknown>
): FlowVariableReference | null {
  const reference = config.protocol_context;

  if (reference === undefined) {
    return {
      kind: DEFAULT_LLM_PROTOCOL_CONTEXT_REFERENCE.kind,
      value: [...DEFAULT_LLM_PROTOCOL_CONTEXT_REFERENCE.value]
    };
  }

  if (
    !isRecord(reference) ||
    Object.keys(reference).length !== 2 ||
    reference.kind !== 'selector' ||
    !Array.isArray(reference.value) ||
    reference.value.length < 2 ||
    reference.value.some(
      (segment) => typeof segment !== 'string' || segment.trim().length === 0
    )
  ) {
    return null;
  }

  return {
    kind: 'selector',
    value: [...(reference.value as string[])]
  };
}

export function getLlmExternalReasoningPolicy(
  config: Record<string, unknown>
): LlmNodeExternalReasoningPolicy {
  const externalReasoningPolicy = config.external_reasoning_policy;

  if (!isRecord(externalReasoningPolicy)) {
    return DEFAULT_LLM_EXTERNAL_REASONING_POLICY;
  }

  return {
    follow_external_reasoning:
      externalReasoningPolicy.follow_external_reasoning === true
  };
}

export function getLlmExternalModelParameterPolicy(
  config: Record<string, unknown>
): LlmNodeExternalModelParameterPolicy {
  const externalModelParameterPolicy = config.external_model_parameter_policy;

  if (!isRecord(externalModelParameterPolicy)) {
    return DEFAULT_LLM_EXTERNAL_MODEL_PARAMETER_POLICY;
  }

  return {
    follow_external_max_output_tokens:
      externalModelParameterPolicy.follow_external_max_output_tokens !== false
  };
}

export function getLlmVisibleInternalToolsEnabled(
  config: Record<string, unknown>
): boolean {
  return config.visible_internal_llm_tools_enabled === true;
}

export function getLlmMcpInstanceIds(
  config: Record<string, unknown>
): string[] {
  if (!Array.isArray(config.mcp_instance_ids)) {
    return DEFAULT_LLM_MCP_INSTANCE_IDS;
  }

  return config.mcp_instance_ids.filter(
    (value): value is string => typeof value === 'string' && value.length > 0
  );
}

export function getLlmToolInternalLlmNodePolicy(
  tool: Pick<LlmVisibleInternalTool, 'internal_llm_node_policy'>
): LlmInternalLlmNodePolicy {
  return tool.internal_llm_node_policy === 'allowed'
    ? 'allowed'
    : DEFAULT_LLM_INTERNAL_LLM_NODE_POLICY;
}

export function getLlmToolExternalToolPolicy(
  tool: Pick<LlmVisibleInternalTool, 'external_tool_policy'>
): LlmExternalToolPolicy {
  return tool.external_tool_policy === 'inherited'
    ? 'inherited'
    : DEFAULT_LLM_EXTERNAL_TOOL_POLICY;
}

export function getLlmToolMode(
  tool: Pick<LlmVisibleInternalTool, 'tool_mode'>
): LlmToolMode {
  return tool.tool_mode === 'fusion' ? 'fusion' : DEFAULT_LLM_TOOL_MODE;
}

export function getLlmVisibleInternalTools(
  config: Record<string, unknown>
): LlmVisibleInternalTool[] {
  const tools = config.visible_internal_llm_tools;

  if (!Array.isArray(tools)) {
    return DEFAULT_LLM_VISIBLE_INTERNAL_TOOLS;
  }

  return tools.flatMap((tool): LlmVisibleInternalTool[] => {
    if (!isRecord(tool) || tool.type !== 'visible_internal_llm_tool') {
      return [];
    }

    const toolName = asString(tool.tool_name).trim();
    const targetNodeId = asString(tool.target_node_id).trim();

    if (!toolName) {
      return [];
    }

    const registration: LlmVisibleInternalTool = {
      type: 'visible_internal_llm_tool',
      tool_name: toolName,
      connector_id: asString(tool.connector_id).trim() || toolName,
      target_node_id: targetNodeId
    };
    const description = asString(tool.description).trim();
    if (description) {
      registration.description = description;
    }
    if (isRecord(tool.input_schema)) {
      registration.input_schema = tool.input_schema as Record<string, unknown>;
    }
    const preconditions = recordArray(tool.preconditions);
    if (preconditions.length > 0) {
      registration.preconditions = preconditions;
    }
    if (tool.tool_mode === 'fusion') {
      registration.tool_mode = 'fusion';
    } else if (tool.tool_mode === 'agent') {
      registration.tool_mode = 'agent';
    }
    if (tool.internal_llm_node_policy === 'allowed') {
      registration.internal_llm_node_policy = 'allowed';
    }
    if (tool.external_tool_policy === 'inherited') {
      registration.external_tool_policy = 'inherited';
    }

    return [registration];
  });
}

export function getLlmResponseFormat(
  config: Record<string, unknown>
): LlmNodeResponseFormat {
  const responseFormat = config.response_format;

  if (!isRecord(responseFormat)) {
    return DEFAULT_LLM_RESPONSE_FORMAT;
  }

  const mode = asString(responseFormat.mode);
  if (mode !== 'json_object' && mode !== 'json_schema') {
    return DEFAULT_LLM_RESPONSE_FORMAT;
  }

  const schema = isRecord(responseFormat.schema)
    ? (responseFormat.schema as Record<string, unknown>)
    : undefined;

  return {
    mode,
    schema
  };
}
