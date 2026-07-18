import type { FlowNodeDocument } from '@1flowbase/flow-schema';
import { DeleteOutlined, EditOutlined, PlusOutlined } from '@ant-design/icons';
import {
  Button,
  Checkbox,
  Input,
  List,
  Select,
  Switch,
  Typography
} from 'antd';
import { useReducer, useRef } from 'react';

import type { SchemaFieldRendererProps } from '../../../../../shared/schema-ui/v1/registry/create-renderer-registry';
import {
  DEFAULT_LLM_EXTERNAL_TOOL_POLICY,
  DEFAULT_LLM_INTERNAL_LLM_NODE_POLICY,
  DEFAULT_LLM_TOOL_MODE,
  getLlmToolMode,
  getLlmVisibleInternalTools,
  getLlmVisibleInternalToolsEnabled,
  isLlmToolIdentifier,
  type LlmExternalToolPolicy,
  type LlmInternalLlmNodePolicy,
  type LlmToolMode,
  type LlmVisibleInternalTool
} from '../../../lib/llm-node-config';
import { parseJsonSchemaInput } from '../../../lib/output-contract/schema';
import { i18nText } from '../../../../../shared/i18n/text';
import { FloatingSettingsPanel } from '../FloatingSettingsPanel';
import {
  JsonSchemaInlineEditor,
  type JsonSchemaEditorResult
} from './json-schema/JsonSchemaSettingsPanel';
import {
  JsonProtocolInlineEditor,
  type JsonProtocolEditorResult
} from './json-schema/JsonProtocolInlineEditor';
import { createDefaultJsonSchema } from './json-schema/json-schema-utils';

const TOOL_FORM_ROW_STYLE = {
  display: 'grid',
  gap: 6,
  color: '#31483a',
  fontSize: 13,
  fontWeight: 600
} as const;

const TOOL_FORM_ERROR_STYLE = {
  fontSize: 12,
  fontWeight: 400
} as const;

const TOOL_FORM_SWITCH_ROW_STYLE = {
  ...TOOL_FORM_ROW_STYLE,
  alignItems: 'center',
  gridTemplateColumns: 'minmax(0, 1fr) auto'
} as const;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function getNodeConfig(node: FlowNodeDocument) {
  return isRecord(node.config) ? node.config : {};
}

function getCurrentNode(adapter: SchemaFieldRendererProps['adapter']) {
  return adapter.getDerived('node') as FlowNodeDocument | null;
}

function createToolName(index: number) {
  return `tool_${index + 1}`;
}

function createNextToolName(tools: LlmVisibleInternalTool[]) {
  const usedIdentifiers = new Set(
    tools.flatMap((tool) => [tool.tool_name, tool.connector_id ?? ''])
  );
  let index = 0;

  while (usedIdentifiers.has(createToolName(index))) {
    index += 1;
  }

  return createToolName(index);
}

function buildNextTool(
  tools: LlmVisibleInternalTool[]
): LlmVisibleInternalTool {
  const toolName = createNextToolName(tools);

  return {
    type: 'visible_internal_llm_tool',
    tool_name: toolName,
    connector_id: toolName,
    target_node_id: '',
    tool_mode: DEFAULT_LLM_TOOL_MODE,
    internal_llm_node_policy: DEFAULT_LLM_INTERNAL_LLM_NODE_POLICY,
    external_tool_policy: DEFAULT_LLM_EXTERNAL_TOOL_POLICY,
    input_schema: { type: 'object' }
  };
}

interface LlmToolRegistrationDraft {
  tool_name: string;
  description: string;
  input_schema: Record<string, unknown>;
  preconditions: Array<Record<string, unknown>>;
  connector_id: string;
  tool_mode: LlmToolMode;
  internal_llm_node_policy: LlmInternalLlmNodePolicy;
  external_tool_policy: LlmExternalToolPolicy;
}

function recordArray(value: unknown): Array<Record<string, unknown>> {
  if (!Array.isArray(value)) {
    return [];
  }

  const records: Array<Record<string, unknown>> = [];

  for (const item of value) {
    if (isRecord(item)) {
      records.push({ ...item });
    }
  }

  return records;
}

function stringifyToolPreconditions(
  preconditions: Array<Record<string, unknown>>
) {
  return JSON.stringify(preconditions, null, 2);
}

function normalizeToolPrecondition(
  precondition: Record<string, unknown>
): Record<string, unknown> {
  const mediaKind = preconditionMediaKind(precondition);

  return {
    kind: preconditionKind(precondition),
    argument_path: preconditionArgumentPath(precondition),
    ...(mediaKind ? { media_kind: mediaKind } : {})
  };
}

function normalizeToolPreconditions(
  preconditions: Array<Record<string, unknown>>
) {
  return preconditions.map(normalizeToolPrecondition);
}

function createDefaultToolPrecondition(): Record<string, unknown> {
  return {
    kind: 'media_content_available',
    argument_path: ['media'],
    media_kind: 'image'
  };
}

function preconditionKind(value: Record<string, unknown>) {
  return typeof value.kind === 'string'
    ? value.kind
    : typeof value.type === 'string'
      ? value.type
      : 'media_content_available';
}

function preconditionMediaKind(value: Record<string, unknown>) {
  return typeof value.media_kind === 'string'
    ? value.media_kind
    : typeof value.mediaKind === 'string'
      ? value.mediaKind
      : '';
}

function argumentPathFromValue(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }

  const path: string[] = [];

  for (const entry of value) {
    if (typeof entry !== 'string') {
      continue;
    }

    const segment = entry.trim();
    if (segment) {
      path.push(segment);
    }
  }

  return path;
}

function preconditionArgumentPath(value: Record<string, unknown>) {
  const path = argumentPathFromValue(value.argument_path ?? value.argumentPath);

  return path.length > 0 ? path : ['media'];
}

function stringifyArgumentPath(value: Record<string, unknown>) {
  return preconditionArgumentPath(value).join('.');
}

function parseArgumentPathInput(value: string) {
  const trimmedValue = value.trim();

  if (!trimmedValue) {
    return [];
  }

  if (trimmedValue.startsWith('[')) {
    try {
      return argumentPathFromValue(JSON.parse(trimmedValue));
    } catch {
      return [trimmedValue];
    }
  }

  const path: string[] = [];

  for (const segment of trimmedValue.split('.')) {
    const trimmedSegment = segment.trim();
    if (trimmedSegment) {
      path.push(trimmedSegment);
    }
  }

  return path;
}

function embeddedPreconditions(value: Record<string, unknown>) {
  if (Array.isArray(value.preconditions)) {
    return recordArray(value.preconditions);
  }

  if (Array.isArray(value.preConditions)) {
    return recordArray(value.preConditions);
  }

  return null;
}

function mediaSchemaPreconditions(value: Record<string, unknown>) {
  const schema = isRecord(value.input_schema)
    ? value.input_schema
    : isRecord(value.inputSchema)
      ? value.inputSchema
      : value;
  const properties = isRecord(schema.properties) ? schema.properties : {};

  if (!isRecord(properties.media)) {
    return null;
  }

  return [createDefaultToolPrecondition()];
}

function parseToolPreconditionsInput(value: string):
  | { ok: true; preconditions: Array<Record<string, unknown>> }
  | {
      ok: false;
      message: string;
    } {
  const trimmedValue = value.trim();

  if (!trimmedValue) {
    return { ok: true, preconditions: [] };
  }

  let parsed: unknown;

  try {
    parsed = JSON.parse(trimmedValue);
  } catch (error) {
    return {
      ok: false,
      message:
        error instanceof Error
          ? error.message
          : i18nText('agentFlow', 'auto.invalid_json')
    };
  }

  if (Array.isArray(parsed)) {
    if (parsed.some((item) => !isRecord(item))) {
      return {
        ok: false,
        message: i18nText('agentFlow', 'auto.preconditions_json_array_required')
      };
    }

    return { ok: true, preconditions: recordArray(parsed) };
  }

  if (isRecord(parsed)) {
    const preconditions = embeddedPreconditions(parsed);

    if (preconditions) {
      return { ok: true, preconditions };
    }

    const schemaPreconditions = mediaSchemaPreconditions(parsed);

    if (schemaPreconditions) {
      return { ok: true, preconditions: schemaPreconditions };
    }
  }

  return {
    ok: false,
    message: i18nText('agentFlow', 'auto.preconditions_json_array_required')
  };
}

function parseToolPreconditionsProtocolInput(
  value: string
): JsonProtocolEditorResult<Array<Record<string, unknown>>> {
  const parsed = parseToolPreconditionsInput(value);

  return parsed.ok
    ? { ok: true, value: parsed.preconditions }
    : { ok: false, message: parsed.message };
}

function splitToolRegistrationSchema(value: Record<string, unknown>): {
  input_schema: Record<string, unknown>;
  preconditions: Array<Record<string, unknown>> | null;
} | null {
  const inputSchema = isRecord(value.input_schema)
    ? value.input_schema
    : isRecord(value.inputSchema)
      ? value.inputSchema
      : null;

  if (!inputSchema) {
    return null;
  }

  return {
    input_schema: inputSchema,
    preconditions: embeddedPreconditions(value)
  };
}

function parseToolRegistrationSchemaInput(
  value: string
): JsonSchemaEditorResult {
  const trimmedValue = value.trim();

  if (!trimmedValue) {
    return parseJsonSchemaInput(value);
  }

  try {
    const parsed: unknown = JSON.parse(trimmedValue);
    const embeddedToolConfig = isRecord(parsed)
      ? splitToolRegistrationSchema(parsed)
      : null;

    if (embeddedToolConfig) {
      return {
        ok: true,
        schema: embeddedToolConfig
      };
    }
  } catch {
    return parseJsonSchemaInput(value);
  }

  return parseJsonSchemaInput(value);
}

function draftFromTool(tool: LlmVisibleInternalTool): LlmToolRegistrationDraft {
  const inputSchema = isRecord(tool.input_schema)
    ? tool.input_schema
    : createDefaultJsonSchema();
  const embeddedToolConfig = splitToolRegistrationSchema(inputSchema);
  const toolMode = getLlmToolMode(tool);

  return {
    tool_name: tool.tool_name,
    description: tool.description ?? '',
    input_schema: embeddedToolConfig?.input_schema ?? inputSchema,
    preconditions:
      recordArray(tool.preconditions).length > 0
        ? recordArray(tool.preconditions)
        : (embeddedToolConfig?.preconditions ?? []),
    connector_id: tool.connector_id ?? tool.tool_name,
    tool_mode: toolMode,
    internal_llm_node_policy:
      toolMode === 'fusion'
        ? 'allowed'
        : (tool.internal_llm_node_policy ??
          DEFAULT_LLM_INTERNAL_LLM_NODE_POLICY),
    external_tool_policy:
      toolMode === 'fusion'
        ? 'forbidden'
        : (tool.external_tool_policy ?? DEFAULT_LLM_EXTERNAL_TOOL_POLICY)
  };
}

function toolFromDraft(draft: LlmToolRegistrationDraft, targetNodeId: string) {
  const toolName = draft.tool_name.trim();
  const connectorId = draft.connector_id.trim();

  if (!isLlmToolIdentifier(toolName) || !isLlmToolIdentifier(connectorId)) {
    return null;
  }

  return {
    type: 'visible_internal_llm_tool' as const,
    tool_name: toolName,
    connector_id: connectorId,
    target_node_id: targetNodeId,
    description: draft.description.trim() || undefined,
    tool_mode: draft.tool_mode,
    internal_llm_node_policy:
      draft.tool_mode === 'fusion' ? 'allowed' : draft.internal_llm_node_policy,
    external_tool_policy:
      draft.tool_mode === 'fusion' ? 'forbidden' : draft.external_tool_policy,
    input_schema: draft.input_schema,
    preconditions:
      draft.preconditions.length > 0
        ? normalizeToolPreconditions(draft.preconditions)
        : undefined
  };
}

function identifierError(
  value: string,
  existingIdentifiers: Set<string>
): string | null {
  const trimmedValue = value.trim();

  if (!isLlmToolIdentifier(trimmedValue)) {
    return i18nText('agentFlow', 'auto.tool_identifier_rule');
  }

  if (existingIdentifiers.has(trimmedValue)) {
    return i18nText('agentFlow', 'auto.tool_identifier_duplicate');
  }

  return null;
}

interface LlmToolRegistrationsEditorState {
  editingIndex: number | null;
  draft: LlmToolRegistrationDraft | null;
  schemaEditorValid: boolean;
  schemaEditorRevision: number;
  preconditionsEditorValid: boolean;
}

const CLOSED_LLM_TOOL_REGISTRATION_EDITOR_STATE: LlmToolRegistrationsEditorState =
  {
    editingIndex: null,
    draft: null,
    schemaEditorValid: true,
    schemaEditorRevision: 0,
    preconditionsEditorValid: true
  };

type LlmToolRegistrationsEditorAction =
  | {
      type: 'open';
      editingIndex: number | null;
      draft: LlmToolRegistrationDraft;
    }
  | { type: 'close' }
  | { type: 'update-draft'; patch: Partial<LlmToolRegistrationDraft> }
  | {
      type: 'apply-input-schema';
      inputSchema: Record<string, unknown>;
      preconditions: Array<Record<string, unknown>> | null;
      bumpSchemaRevision: boolean;
    }
  | {
      type: 'update-preconditions';
      preconditions: Array<Record<string, unknown>>;
    }
  | { type: 'set-schema-valid'; valid: boolean }
  | { type: 'set-preconditions-valid'; valid: boolean };

function llmToolRegistrationsEditorReducer(
  state: LlmToolRegistrationsEditorState,
  action: LlmToolRegistrationsEditorAction
): LlmToolRegistrationsEditorState {
  switch (action.type) {
    case 'open':
      return {
        editingIndex: action.editingIndex,
        draft: action.draft,
        schemaEditorValid: true,
        schemaEditorRevision: 0,
        preconditionsEditorValid: true
      };
    case 'close':
      return CLOSED_LLM_TOOL_REGISTRATION_EDITOR_STATE;
    case 'update-draft':
      return state.draft
        ? {
            ...state,
            draft: {
              ...state.draft,
              ...action.patch,
              ...(action.patch.tool_mode === 'fusion'
                ? {
                    internal_llm_node_policy: 'allowed' as const,
                    external_tool_policy: 'forbidden' as const
                  }
                : {})
            }
          }
        : state;
    case 'apply-input-schema':
      return state.draft
        ? {
            ...state,
            schemaEditorRevision: action.bumpSchemaRevision
              ? state.schemaEditorRevision + 1
              : state.schemaEditorRevision,
            preconditionsEditorValid:
              action.preconditions === null
                ? state.preconditionsEditorValid
                : true,
            draft: {
              ...state.draft,
              input_schema: action.inputSchema,
              preconditions:
                action.preconditions === null
                  ? state.draft.preconditions
                  : action.preconditions
            }
          }
        : state;
    case 'update-preconditions':
      return state.draft
        ? {
            ...state,
            preconditionsEditorValid: true,
            draft: {
              ...state.draft,
              preconditions: action.preconditions
            }
          }
        : state;
    case 'set-schema-valid':
      return {
        ...state,
        schemaEditorValid: action.valid
      };
    case 'set-preconditions-valid':
      return {
        ...state,
        preconditionsEditorValid: action.valid
      };
    default:
      return state;
  }
}

function patchPrecondition(
  preconditions: Array<Record<string, unknown>>,
  index: number,
  onChange: (nextPreconditions: Array<Record<string, unknown>>) => void,
  patch: Partial<Record<string, unknown>>
) {
  onChange(
    preconditions.map((precondition, preconditionIndex) =>
      preconditionIndex === index
        ? {
            ...precondition,
            ...patch
          }
        : precondition
    )
  );
}

function removePrecondition(
  preconditions: Array<Record<string, unknown>>,
  index: number,
  onChange: (nextPreconditions: Array<Record<string, unknown>>) => void
) {
  onChange(
    preconditions.filter((_, preconditionIndex) => preconditionIndex !== index)
  );
}

function addPrecondition(
  preconditions: Array<Record<string, unknown>>,
  onChange: (nextPreconditions: Array<Record<string, unknown>>) => void
) {
  onChange([...preconditions, createDefaultToolPrecondition()]);
}

const preconditionRowKeys = new WeakMap<Record<string, unknown>, string>();
let preconditionRowKeySequence = 0;

function preconditionRowKey(precondition: Record<string, unknown>) {
  const existingKey = preconditionRowKeys.get(precondition);
  if (existingKey) {
    return existingKey;
  }

  preconditionRowKeySequence += 1;
  const key = `precondition:${preconditionRowKeySequence}`;
  preconditionRowKeys.set(precondition, key);
  return key;
}

function renderPreconditionRows({
  value,
  onChange
}: {
  value: Array<Record<string, unknown>>;
  onChange: (nextPreconditions: Array<Record<string, unknown>>) => void;
}) {
  return (
    <div className="agent-flow-json-schema-settings__fields">
      <div className="agent-flow-json-schema-settings__field-head">
        <span>{i18nText('agentFlow', 'auto.field_name')}</span>
        <span>{i18nText('agentFlow', 'auto.schema_value_or_description')}</span>
        <span>{i18nText('agentFlow', 'auto.type')}</span>
        <span>{i18nText('agentFlow', 'auto.required')}</span>
        <span>{i18nText('agentFlow', 'auto.operation')}</span>
      </div>
      <div className="agent-flow-json-schema-settings__field-rows">
        {value.map((precondition, index) => {
          const indexLabel = String(index + 1);

          return (
            <div
              className="agent-flow-json-schema-settings__field-node"
              key={preconditionRowKey(precondition)}
            >
              <div className="agent-flow-json-schema-settings__field-row">
                <Input
                  aria-label={i18nText(
                    'agentFlow',
                    'auto.precondition_field_name',
                    { value1: indexLabel, value2: 'kind' }
                  )}
                  disabled
                  value="kind"
                />
                <Input
                  aria-label={i18nText(
                    'agentFlow',
                    'auto.precondition_field_value',
                    { value1: indexLabel, value2: 'kind' }
                  )}
                  value={preconditionKind(precondition)}
                  onChange={(event) =>
                    patchPrecondition(value, index, onChange, {
                      kind: event.target.value
                    })
                  }
                />
                <Input disabled value="String" />
                <Checkbox disabled checked />
                <Button
                  aria-label={i18nText(
                    'agentFlow',
                    'auto.delete_tool_precondition',
                    { value1: indexLabel }
                  )}
                  danger
                  icon={<DeleteOutlined />}
                  size="small"
                  type="text"
                  onClick={() => removePrecondition(value, index, onChange)}
                />
              </div>
              <div
                className="agent-flow-json-schema-settings__field-row"
                style={{ paddingLeft: 18 }}
              >
                <Input
                  aria-label={i18nText(
                    'agentFlow',
                    'auto.precondition_field_name',
                    { value1: indexLabel, value2: 'argument_path' }
                  )}
                  disabled
                  value="argument_path"
                />
                <Input
                  aria-label={i18nText(
                    'agentFlow',
                    'auto.precondition_field_value',
                    { value1: indexLabel, value2: 'argument_path' }
                  )}
                  value={stringifyArgumentPath(precondition)}
                  onChange={(event) =>
                    patchPrecondition(value, index, onChange, {
                      argument_path: parseArgumentPathInput(event.target.value)
                    })
                  }
                />
                <Input disabled value="Array<String>" />
                <Checkbox disabled checked />
                <div className="agent-flow-json-schema-settings__field-actions" />
              </div>
              <div
                className="agent-flow-json-schema-settings__field-row"
                style={{ paddingLeft: 18 }}
              >
                <Input
                  aria-label={i18nText(
                    'agentFlow',
                    'auto.precondition_field_name',
                    { value1: indexLabel, value2: 'media_kind' }
                  )}
                  disabled
                  value="media_kind"
                />
                <Input
                  aria-label={i18nText(
                    'agentFlow',
                    'auto.precondition_field_value',
                    { value1: indexLabel, value2: 'media_kind' }
                  )}
                  value={preconditionMediaKind(precondition)}
                  onChange={(event) =>
                    patchPrecondition(value, index, onChange, {
                      media_kind: event.target.value || undefined
                    })
                  }
                />
                <Input disabled value="String" />
                <Checkbox disabled checked={false} />
                <div className="agent-flow-json-schema-settings__field-actions" />
              </div>
            </div>
          );
        })}
        <Button
          icon={<PlusOutlined />}
          type="dashed"
          onClick={() => addPrecondition(value, onChange)}
        >
          {i18nText('agentFlow', 'auto.add_tool_precondition')}
        </Button>
      </div>
    </div>
  );
}

function LlmToolRegistrationForm({
  connectorIdError,
  draft,
  editingIndex,
  schemaEditorRevision,
  toolNameError,
  onPreconditionsChange,
  onPreconditionsValidityChange,
  onSchemaChange,
  onSchemaValidityChange,
  onUpdateDraft
}: {
  connectorIdError: string | null;
  draft: LlmToolRegistrationDraft;
  editingIndex: number | null;
  schemaEditorRevision: number;
  toolNameError: string | null;
  onPreconditionsChange: (
    nextPreconditions: Array<Record<string, unknown>>
  ) => void;
  onPreconditionsValidityChange: (valid: boolean) => void;
  onSchemaChange: (schema: Record<string, unknown>) => void;
  onSchemaValidityChange: (valid: boolean) => void;
  onUpdateDraft: (patch: Partial<LlmToolRegistrationDraft>) => void;
}) {
  return (
    <form className="agent-flow-llm-tool-registration-form">
      <label style={TOOL_FORM_ROW_STYLE}>
        <span>{i18nText('agentFlow', 'auto.tool_name')}</span>
        <Input
          aria-label={i18nText('agentFlow', 'auto.tool_name')}
          status={toolNameError ? 'error' : undefined}
          value={draft.tool_name}
          onChange={(event) =>
            onUpdateDraft({
              tool_name: event.target.value,
              connector_id: draft.connector_id || event.target.value
            })
          }
        />
        {toolNameError ? (
          <Typography.Text type="danger" style={TOOL_FORM_ERROR_STYLE}>
            {toolNameError}
          </Typography.Text>
        ) : null}
      </label>
      <label style={TOOL_FORM_ROW_STYLE}>
        <span>{i18nText('agentFlow', 'auto.tool_identifier')}</span>
        <Input
          aria-label={i18nText('agentFlow', 'auto.tool_identifier')}
          status={connectorIdError ? 'error' : undefined}
          value={draft.connector_id}
          onChange={(event) =>
            onUpdateDraft({ connector_id: event.target.value })
          }
        />
        {connectorIdError ? (
          <Typography.Text type="danger" style={TOOL_FORM_ERROR_STYLE}>
            {connectorIdError}
          </Typography.Text>
        ) : null}
      </label>
      <label style={TOOL_FORM_ROW_STYLE}>
        <span>{i18nText('agentFlow', 'auto.description')}</span>
        <Input
          aria-label={i18nText('agentFlow', 'auto.description')}
          value={draft.description}
          onChange={(event) =>
            onUpdateDraft({ description: event.target.value })
          }
        />
      </label>
      <div style={TOOL_FORM_SWITCH_ROW_STYLE}>
        <span>{i18nText('agentFlow', 'auto.internal_llm_node_policy')}</span>
        <Switch
          aria-label={i18nText('agentFlow', 'auto.internal_llm_node_policy')}
          checked={draft.internal_llm_node_policy === 'allowed'}
          disabled={draft.tool_mode === 'fusion'}
          onChange={(checked) =>
            onUpdateDraft({
              internal_llm_node_policy: checked ? 'allowed' : 'forbidden',
              ...(checked ? {} : { tool_mode: 'agent' as const })
            })
          }
        />
      </div>
      {draft.internal_llm_node_policy === 'allowed' ? (
        <label style={TOOL_FORM_ROW_STYLE}>
          <span>{i18nText('agentFlow', 'auto.tool_mode')}</span>
          <Select
            aria-label={i18nText('agentFlow', 'auto.tool_mode')}
            options={[
              {
                label: i18nText('agentFlow', 'auto.tool_mode_agent'),
                value: 'agent'
              },
              {
                label: i18nText('agentFlow', 'auto.tool_mode_fusion'),
                value: 'fusion'
              }
            ]}
            value={draft.tool_mode}
            onChange={(nextMode: LlmToolMode) =>
              onUpdateDraft({ tool_mode: nextMode })
            }
          />
        </label>
      ) : null}
      {draft.tool_mode === 'agent' ? (
        <div style={TOOL_FORM_SWITCH_ROW_STYLE}>
          <span>{i18nText('agentFlow', 'auto.external_tool_policy')}</span>
          <Switch
            aria-label={i18nText('agentFlow', 'auto.external_tool_policy')}
            checked={draft.external_tool_policy === 'inherited'}
            onChange={(checked) =>
              onUpdateDraft({
                external_tool_policy: checked ? 'inherited' : 'forbidden'
              })
            }
          />
        </div>
      ) : null}
      <div style={TOOL_FORM_ROW_STYLE}>
        <span>{i18nText('agentFlow', 'auto.tool_preconditions')}</span>
        <JsonProtocolInlineEditor
          ariaLabel={i18nText('agentFlow', 'auto.tool_preconditions_json')}
          className="agent-flow-llm-tool-registration-preconditions"
          testId="agent-flow-llm-tool-preconditions-json-editor"
          parseValue={parseToolPreconditionsProtocolInput}
          renderFields={renderPreconditionRows}
          stringifyValue={stringifyToolPreconditions}
          value={draft.preconditions}
          onChange={onPreconditionsChange}
          onValidityChange={onPreconditionsValidityChange}
        />
      </div>
      <div style={TOOL_FORM_ROW_STYLE}>
        <span>{i18nText('agentFlow', 'auto.input_parameters')}</span>
        <div className="agent-flow-llm-tool-registration-schema">
          <JsonSchemaInlineEditor
            parseSchemaInput={parseToolRegistrationSchemaInput}
            resetKey={`${editingIndex ?? 'new'}:${schemaEditorRevision}`}
            schema={draft.input_schema}
            onChange={onSchemaChange}
            onValidityChange={onSchemaValidityChange}
          />
        </div>
      </div>
    </form>
  );
}

export function LlmToolRegistrationsField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  const [editorState, dispatchEditor] = useReducer(
    llmToolRegistrationsEditorReducer,
    CLOSED_LLM_TOOL_REGISTRATION_EDITOR_STATE
  );
  const {
    draft,
    editingIndex,
    preconditionsEditorValid,
    schemaEditorRevision,
    schemaEditorValid
  } = editorState;
  const toolEditorTriggerRef = useRef<HTMLElement | null>(null);
  const currentNode = getCurrentNode(adapter);

  if (!currentNode) {
    return null;
  }

  const currentConfig = getNodeConfig(currentNode);
  const enabled = getLlmVisibleInternalToolsEnabled(currentConfig);
  const tools = getLlmVisibleInternalTools(currentConfig);
  const existingToolNames = new Set(
    tools.flatMap((tool, index) =>
      index === editingIndex ? [] : [tool.tool_name]
    )
  );
  const existingConnectorIds = new Set(
    tools.flatMap((tool, index) =>
      index === editingIndex ? [] : [tool.connector_id || tool.tool_name]
    )
  );
  const toolNameError = draft
    ? identifierError(draft.tool_name, existingToolNames)
    : null;
  const connectorIdError = draft
    ? identifierError(draft.connector_id, existingConnectorIds)
    : null;
  const toolEditorValid =
    draft !== null &&
    !toolNameError &&
    !connectorIdError &&
    schemaEditorValid &&
    preconditionsEditorValid;

  function updateTools(nextTools: LlmVisibleInternalTool[]) {
    adapter.setValue('config.visible_internal_llm_tools', nextTools);
  }

  function openToolEditor(
    index: number | null,
    tool: LlmVisibleInternalTool,
    trigger: HTMLElement | null
  ) {
    const nextDraft = draftFromTool(tool);

    toolEditorTriggerRef.current = trigger;
    dispatchEditor({
      type: 'open',
      editingIndex: index,
      draft: nextDraft
    });
  }

  function closeToolEditor() {
    dispatchEditor({ type: 'close' });
  }

  function updateDraft(patch: Partial<LlmToolRegistrationDraft>) {
    dispatchEditor({ type: 'update-draft', patch });
  }

  function saveDraft() {
    if (!draft) {
      return;
    }
    const currentTargetNodeId =
      editingIndex === null ? '' : (tools[editingIndex]?.target_node_id ?? '');
    const nextTool = toolFromDraft(draft, currentTargetNodeId);

    if (!nextTool) {
      return;
    }

    if (editingIndex === null) {
      updateTools([...tools, nextTool]);
    } else {
      updateTools(
        tools.map((tool, toolIndex) =>
          toolIndex === editingIndex ? nextTool : tool
        )
      );
    }
    closeToolEditor();
  }

  function updateInputSchema(schema: Record<string, unknown>) {
    const embeddedToolConfig = splitToolRegistrationSchema(schema);

    if (embeddedToolConfig) {
      dispatchEditor({
        type: 'apply-input-schema',
        inputSchema: embeddedToolConfig.input_schema,
        preconditions: embeddedToolConfig.preconditions,
        bumpSchemaRevision: true
      });
      return;
    }

    dispatchEditor({
      type: 'apply-input-schema',
      inputSchema: schema,
      preconditions: null,
      bumpSchemaRevision: false
    });
  }

  function updatePreconditions(
    nextPreconditions: Array<Record<string, unknown>>
  ) {
    dispatchEditor({
      type: 'update-preconditions',
      preconditions: nextPreconditions
    });
  }

  const modalTitle = i18nText('agentFlow', 'auto.edit', {
    value1: i18nText('agentFlow', 'auto.tool_registration')
  });
  const addToolLabel = i18nText('agentFlow', 'auto.add_tool_registration');
  const toolEditorFooter = (
    <div className="agent-flow-llm-tool-registration-panel__footer">
      <Button onClick={closeToolEditor}>
        {i18nText('agentFlow', 'auto.cancel')}
      </Button>
      <Button disabled={!toolEditorValid} type="primary" onClick={saveDraft}>
        {i18nText('agentFlow', 'auto.save_tool')}
      </Button>
    </div>
  );

  return (
    <div className="agent-flow-llm-tool-registrations">
      <div
        className="agent-flow-llm-tool-registrations__toolbar"
        data-testid="agent-flow-llm-tool-registrations-toolbar"
      >
        <Typography.Text
          strong
          className="agent-flow-llm-tool-registrations__label"
        >
          {block.label}
        </Typography.Text>
        <Button
          aria-label={addToolLabel}
          className="agent-flow-llm-tool-registrations__add"
          disabled={!enabled}
          icon={
            <PlusOutlined data-testid="agent-flow-llm-tool-registration-add-icon" />
          }
          shape="circle"
          size="small"
          type="text"
          onClick={(event) =>
            openToolEditor(null, buildNextTool(tools), event.currentTarget)
          }
        />
        <Switch
          aria-label={block.label}
          checked={enabled}
          className="agent-flow-llm-tool-registrations__switch"
          onChange={(checked) =>
            adapter.setValue(
              'config.visible_internal_llm_tools_enabled',
              checked
            )
          }
        />
      </div>
      {enabled ? (
        <List
          aria-label={i18nText('agentFlow', 'auto.tool_registration')}
          bordered
          dataSource={tools}
          locale={{
            emptyText: i18nText('agentFlow', 'auto.no_tool_registrations')
          }}
          renderItem={(tool, index) => {
            const toolName = tool.tool_name || createToolName(index);

            return (
              <List.Item
                actions={[
                  <Button
                    aria-label={i18nText('agentFlow', 'auto.edit', {
                      value1: toolName
                    })}
                    icon={<EditOutlined />}
                    key="edit"
                    size="small"
                    type="text"
                    onClick={(event) =>
                      openToolEditor(index, tool, event.currentTarget)
                    }
                  />,
                  <Button
                    aria-label={i18nText('agentFlow', 'auto.delete_item', {
                      value1: toolName
                    })}
                    danger
                    icon={<DeleteOutlined />}
                    key="delete"
                    size="small"
                    type="text"
                    onClick={() =>
                      updateTools(
                        tools.filter((_, toolIndex) => toolIndex !== index)
                      )
                    }
                  />
                ]}
              >
                {toolName}
              </List.Item>
            );
          }}
          rowKey={(tool) => tool.connector_id || tool.tool_name}
          size="small"
        />
      ) : null}
      <FloatingSettingsPanel
        className="agent-flow-llm-tool-registration-panel"
        closeLabel={i18nText('agentFlow', 'auto.close', {
          value1: i18nText('agentFlow', 'auto.tool_registration')
        })}
        defaultWidth={720}
        dragHandleTestId="agent-flow-llm-tool-registration-drag-handle"
        footer={toolEditorFooter}
        initialHeight={520}
        minHeight={360}
        minWidth={560}
        open={draft !== null}
        title={modalTitle}
        triggerRef={toolEditorTriggerRef}
        onClose={closeToolEditor}
      >
        {draft ? (
          <LlmToolRegistrationForm
            connectorIdError={connectorIdError}
            draft={draft}
            editingIndex={editingIndex}
            schemaEditorRevision={schemaEditorRevision}
            toolNameError={toolNameError}
            onPreconditionsChange={updatePreconditions}
            onPreconditionsValidityChange={(valid) =>
              dispatchEditor({ type: 'set-preconditions-valid', valid })
            }
            onSchemaChange={updateInputSchema}
            onSchemaValidityChange={(valid) =>
              dispatchEditor({ type: 'set-schema-valid', valid })
            }
            onUpdateDraft={updateDraft}
          />
        ) : null}
      </FloatingSettingsPanel>
    </div>
  );
}
