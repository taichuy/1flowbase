import type {
  FlowConditionGroupDocument,
  FlowNodeDocument,
  IfElseBranchDocument
} from '@1flowbase/flow-schema';
import type { ReactNode } from 'react';
import { Input, InputNumber, Select, Switch } from 'antd';

import type {
  SchemaFieldRenderer,
  SchemaFieldRendererProps
} from '../../../shared/schema-ui/registry/create-renderer-registry';

import type { AgentFlowDataModelFieldOption } from '../api/data-model-options';
import { ConditionGroupField } from '../components/bindings/ConditionGroupField';
import { DataModelQueryField } from '../components/bindings/DataModelQueryField';
import { IfElseBranchesField } from '../components/bindings/IfElseBranchesField';
import { NamedBindingsField } from '../components/bindings/NamedBindingsField';
import { SelectorField } from '../components/bindings/SelectorField';
import { StateWriteField } from '../components/bindings/StateWriteField';
import { TemplatedTextField } from '../components/bindings/TemplatedTextField';
import {
  VariableAssignmentField,
  type VariableAssignmentValue
} from '../components/bindings/VariableAssignmentField';
import {
  TemplatedNamedBindingsField,
  type TemplatedNamedBindingValue
} from '../components/bindings/TemplatedNamedBindingsField';
import { OutputContractDefinitionField } from '../components/detail/fields/output-contract/OutputContractDefinitionField';
import { CodeSourceField } from '../components/detail/fields/CodeSourceField';
import { DataModelField } from '../components/detail/fields/DataModelField';
import { LlmModelField } from '../components/detail/fields/LlmModelField';
import { LlmToolRegistrationsField } from '../components/detail/fields/LlmToolRegistrationsField';
import { LlmPromptMessagesField } from '../components/detail/fields/LlmPromptMessagesField';
import { LlmResponseFormatField } from '../components/detail/fields/LlmResponseFormatField';
import { StartInputFieldsField } from '../components/detail/fields/StartInputFieldsField';
import { StartModelListField } from '../components/detail/fields/StartModelListField';
import { HttpRequestBodyField } from '../components/detail/fields/HttpRequestBodyField';
import { HttpRequestCurlImportField } from '../components/detail/fields/HttpRequestCurlImportField';
import { HttpRequestKeyValuesField } from '../components/detail/fields/HttpRequestKeyValuesField';
import { HttpRequestTemplateInput } from '../components/detail/fields/HttpRequestTemplateInput';
import {
  DATA_MODEL_QUERY_DEFAULT_VALUE,
  normalizeDataModelQueryBindingValue
} from '../lib/data-model-query-binding';
import {
  normalizePromptMessagesBinding,
  toPromptMessagesBinding
} from '../lib/llm-prompt-messages';
import { normalizeIfElseBranches } from '../lib/if-else-branches';
import {
  getLlmContextPolicy,
  getLlmExternalReasoningPolicy
} from '../lib/llm-node-config';
import { HTTP_REQUEST_METHOD_OPTIONS } from '../lib/http-request/contract';
import { getNamedBindingExpression } from '../lib/named-binding-expressions';
import {
  encodeSelectorValue,
  decodeSelectorValue,
  type FlowSelectorOption
} from '../lib/selector-options';
import type { AgentFlowConversationVariable } from '../lib/variables/conversation-variables';
import { codeOutputSelector } from '../lib/output-contract/code-output';
import { outputHasLlmContextSchema } from '../lib/output-contract/schema';
import { createTemplateSelectorToken } from '../lib/template-binding';
import { i18nText } from '../../../shared/i18n/text';

const BYTES_PER_MIB = 1024 * 1024;
const DEFAULT_ENABLED_SWITCH_PATHS = new Set(['config.verify_ssl']);

function getSelectorOptions(adapter: SchemaFieldRendererProps['adapter']) {
  return (
    (adapter.getDerived('selectorOptions') as
      | FlowSelectorOption[]
      | null
      | undefined) ?? []
  );
}

function formatUnavailableSelectorLabel(selector: string[]) {
  return selector.at(-1) ?? '';
}

function hasBindingKind(
  value: unknown,
  kind: string
): value is { kind: string; value: unknown } {
  return (
    typeof value === 'object' &&
    value !== null &&
    'kind' in value &&
    'value' in value &&
    (value as { kind?: unknown }).kind === kind
  );
}

function getBindingValue<T>(value: unknown, kind: string, fallback: T): T {
  return hasBindingKind(value, kind) ? (value.value as T) : fallback;
}

function renderTextField({ adapter, block }: SchemaFieldRendererProps) {
  const value = adapter.getValue(block.path);

  return (
    <Input
      aria-label={block.label}
      value={typeof value === 'string' ? value : ''}
      onChange={(event) => adapter.setValue(block.path, event.target.value)}
    />
  );
}

function renderNumberField({ adapter, block }: SchemaFieldRendererProps) {
  const value = adapter.getValue(block.path);
  const usesMibDisplay = block.numberFormat === 'bytes_as_mib';
  const numberValue =
    typeof value === 'number' && Number.isFinite(value) ? value : null;
  const displayValue =
    usesMibDisplay && numberValue !== null
      ? numberValue / BYTES_PER_MIB
      : numberValue;
  const displayMin =
    usesMibDisplay && typeof block.min === 'number'
      ? block.min / BYTES_PER_MIB
      : block.min;
  const displayMax =
    usesMibDisplay && typeof block.max === 'number'
      ? block.max / BYTES_PER_MIB
      : block.max;
  const displayStep =
    usesMibDisplay && typeof block.step === 'number'
      ? block.step / BYTES_PER_MIB
      : block.step;
  const isCompactNumberField =
    block.path === 'config.timeout_ms' ||
    block.path === 'config.max_response_bytes';

  return (
    <InputNumber
      aria-label={block.label}
      className={[
        'agent-flow-editor__number-field',
        isCompactNumberField ? 'agent-flow-editor__number-field--compact' : null
      ]
        .filter(Boolean)
        .join(' ')}
      min={displayMin}
      max={displayMax}
      step={displayStep}
      value={displayValue}
      onChange={(nextValue) => {
        const normalizedValue =
          typeof nextValue === 'number' && Number.isFinite(nextValue)
            ? nextValue
            : null;
        adapter.setValue(
          block.path,
          usesMibDisplay && normalizedValue !== null
            ? Math.round(normalizedValue * BYTES_PER_MIB)
            : normalizedValue
        );
      }}
    />
  );
}

function renderStaticSelectField({ adapter, block }: SchemaFieldRendererProps) {
  const value = adapter.getValue(block.path);

  return (
    <Select
      aria-label={block.label}
      options={block.options ?? []}
      value={typeof value === 'string' ? value : undefined}
      onChange={(nextValue) => adapter.setValue(block.path, nextValue)}
    />
  );
}

function renderSwitchField({ adapter, block }: SchemaFieldRendererProps) {
  const value = adapter.getValue(block.path);
  const checked =
    typeof value === 'boolean'
      ? value
      : DEFAULT_ENABLED_SWITCH_PATHS.has(block.path);

  return (
    <Switch
      aria-label={block.label}
      checked={checked}
      onChange={(checked) => adapter.setValue(block.path, checked)}
    />
  );
}

function renderSelectorField({ adapter, block }: SchemaFieldRendererProps) {
  const value = adapter.getValue(block.path);
  const binding = getBindingValue<string[]>(value, 'selector', []);

  return (
    <SelectorField
      ariaLabel={block.label}
      options={getSelectorOptions(adapter)}
      value={binding}
      onChange={(nextValue) =>
        adapter.setValue(block.path, {
          kind: 'selector',
          value: nextValue as string[]
        })
      }
    />
  );
}

function renderSelectorListField({ adapter, block }: SchemaFieldRendererProps) {
  const value = adapter.getValue(block.path);
  const binding = getBindingValue<string[][]>(value, 'selector_list', []);

  return (
    <SelectorField
      ariaLabel={block.label}
      multiple
      options={getSelectorOptions(adapter)}
      value={binding}
      onChange={(nextValue) =>
        adapter.setValue(block.path, {
          kind: 'selector_list',
          value: nextValue as string[][]
        })
      }
    />
  );
}

function renderTemplatedTextField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  const value = adapter.getValue(block.path);
  const selectorOptions = getSelectorOptions(adapter);
  const isBindingPath = block.path.startsWith('bindings.');
  const stringValue = isBindingPath
    ? hasBindingKind(value, 'templated_text')
      ? getBindingValue<string>(value, 'templated_text', '')
      : hasBindingKind(value, 'selector')
        ? createTemplateSelectorToken(
            getBindingValue<string[]>(value, 'selector', [])
          )
        : ''
    : typeof value === 'string'
      ? value
      : '';

  return (
    <TemplatedTextField
      label={block.label}
      ariaLabel={block.label}
      placeholder={i18nText(
        'agentFlow',
        'auto.support_text_variable_block_enter_left_curly_bracket_quick_reference'
      )}
      options={selectorOptions}
      value={stringValue}
      onChange={(nextValue) =>
        adapter.setValue(
          block.path,
          isBindingPath
            ? { kind: 'templated_text', value: nextValue }
            : nextValue
        )
      }
    />
  );
}

function renderLlmPromptMessagesField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  const contextPolicy = getLlmContextPolicy({
    context_policy: adapter.getValue('config.context_policy')
  });
  const messages = normalizePromptMessagesBinding(adapter.getValue(block.path));

  return (
    <LlmPromptMessagesField
      integrationContextEnabled={
        contextPolicy.integration_context === 'enabled'
      }
      options={getSelectorOptions(adapter)}
      value={messages}
      onChange={(nextValue) =>
        adapter.setValue(block.path, toPromptMessagesBinding(nextValue))
      }
    />
  );
}

function renderLlmContextPolicyField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  const contextPolicy = getLlmContextPolicy({
    context_policy: adapter.getValue(block.path)
  });
  const contextOptions = getSelectorOptions(adapter).filter((option) =>
    outputHasLlmContextSchema(option)
  );
  const selectedSelector =
    contextPolicy.context_selector ?? contextOptions[0]?.value ?? [];
  const selectedValue =
    selectedSelector.length > 0
      ? encodeSelectorValue(selectedSelector)
      : undefined;
  const hasSelectedOption = contextOptions.some(
    (option) => encodeSelectorValue(option.value) === selectedValue
  );
  const selectOptions: Array<{
    label: ReactNode;
    value: string;
    disabled?: boolean;
  }> = contextOptions.map((option) => ({
    label: option.displayLabel,
    value: encodeSelectorValue(option.value)
  }));

  if (selectedValue && !hasSelectedOption) {
    selectOptions.push({
      label: (
        <span className="agent-flow-node-detail__context-select-missing-value">
          {formatUnavailableSelectorLabel(selectedSelector)}
        </span>
      ),
      value: selectedValue,
      disabled: true
    });
  }

  return (
    <div className="agent-flow-node-detail__context-policy">
      <Select
        aria-label="上下文变量"
        className="agent-flow-node-detail__context-select"
        disabled={contextPolicy.integration_context === 'disabled'}
        options={selectOptions}
        placeholder="选择上下文变量"
        value={selectedValue}
        onChange={(nextValue) =>
          adapter.setValue(block.path, {
            integration_context: contextPolicy.integration_context,
            context_selector: decodeSelectorValue(nextValue)
          })
        }
      />
      <Switch
        aria-label={block.label}
        checked={contextPolicy.integration_context === 'enabled'}
        onChange={(checked) =>
          adapter.setValue(block.path, {
            integration_context: checked ? 'enabled' : 'disabled',
            context_selector: selectedSelector
          })
        }
      />
    </div>
  );
}

function renderLlmExternalReasoningPolicyField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  const externalReasoningPolicy = getLlmExternalReasoningPolicy({
    external_reasoning_policy: adapter.getValue(block.path)
  });

  return (
    <Switch
      aria-label={block.label}
      checked={externalReasoningPolicy.follow_external_reasoning}
      onChange={(checked) =>
        adapter.setValue(block.path, {
          follow_external_reasoning: checked
        })
      }
    />
  );
}

function renderNamedBindingsField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  const value = adapter.getValue(block.path);
  const binding = getBindingValue<Array<{ name: string; selector: string[] }>>(
    value,
    'named_bindings',
    []
  );
  const fields =
    (adapter.getValue('config.data_model_fields') as
      | AgentFlowDataModelFieldOption[]
      | null
      | undefined) ?? [];
  const isDataModelPayload = block.path === 'bindings.payload';
  const nameOptions = isDataModelPayload
    ? fields.reduce<Array<{ value: string; label: string }>>(
        (options, field) => {
          if (field.writable === false) {
            return options;
          }

          options.push({
            value: field.code,
            label: field.title || field.code
          });
          return options;
        },
        []
      )
    : undefined;

  return (
    <NamedBindingsField
      ariaLabel={block.label}
      options={getSelectorOptions(adapter)}
      value={binding}
      nameOptions={nameOptions}
      namePlaceholder={
        isDataModelPayload ? i18nText('agentFlow', 'auto.field') : undefined
      }
      selectorLabel={isDataModelPayload ? 'variable' : undefined}
      addButtonLabel={
        isDataModelPayload
          ? i18nText('agentFlow', 'auto.add_new_field_assignment')
          : undefined
      }
      onChange={(nextValue) =>
        adapter.setValue(block.path, {
          kind: 'named_bindings',
          value: nextValue
        })
      }
    />
  );
}

function findSelectorOption(options: FlowSelectorOption[], selector: string[]) {
  return options.find(
    (option) =>
      option.value.length === selector.length &&
      option.value.every((segment, index) => selector[index] === segment)
  );
}

function normalizeBindingValueType(valueType: string | undefined) {
  return valueType?.startsWith('array') ? 'array' : valueType;
}

function normalizeCodeInputExpression(
  expression: NonNullable<ReturnType<typeof getNamedBindingExpression>>,
  valueType: string | undefined
) {
  const normalizedValueType = normalizeBindingValueType(valueType);

  if (
    expression.kind === 'selector' &&
    (normalizedValueType === 'string' || normalizedValueType === 'number')
  ) {
    return {
      kind: 'templated_text' as const,
      value: createTemplateSelectorToken(expression.selector)
    };
  }

  return expression;
}

function normalizeTemplatedNamedBindingEntries(
  value: unknown,
  options: FlowSelectorOption[]
) {
  return getBindingValue<TemplatedNamedBindingValue[]>(
    value,
    'named_bindings',
    []
  ).map((entry) => {
    const expression = getNamedBindingExpression(entry) ?? {
      kind: 'constant' as const,
      value: ''
    };
    const valueType =
      entry.valueType ??
      (expression.kind === 'selector'
        ? findSelectorOption(options, expression.selector)?.valueType
        : undefined);

    return {
      name: entry.name,
      valueType,
      value: normalizeCodeInputExpression(expression, valueType)
    };
  });
}

function renderTemplatedNamedBindingsField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  const value = adapter.getValue(block.path);
  const options = getSelectorOptions(adapter);
  const binding = normalizeTemplatedNamedBindingEntries(value, options);

  return (
    <TemplatedNamedBindingsField
      ariaLabel={block.label}
      options={options}
      value={binding}
      onChange={(nextValue: TemplatedNamedBindingValue[]) =>
        adapter.setValue(block.path, {
          kind: 'named_bindings',
          value: nextValue
        })
      }
    />
  );
}

function renderConditionGroupField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  const value = adapter.getValue(block.path);
  const binding = getBindingValue<FlowConditionGroupDocument>(
    value,
    'condition_group',
    { operator: 'and', conditions: [] }
  );

  return (
    <ConditionGroupField
      ariaLabel={block.label}
      options={getSelectorOptions(adapter)}
      value={binding}
      onChange={(nextValue) =>
        adapter.setValue(block.path, {
          kind: 'condition_group',
          value: nextValue
        })
      }
    />
  );
}

function renderIfElseBranchesField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  const value = adapter.getValue(block.path);
  const binding = getBindingValue<{ branches: IfElseBranchDocument[] }>(
    value,
    'if_else_branches',
    { branches: [] }
  );

  return (
    <IfElseBranchesField
      ariaLabel={block.label}
      options={getSelectorOptions(adapter)}
      value={normalizeIfElseBranches(binding.branches)}
      onChange={(branches) =>
        adapter.setValue(block.path, {
          kind: 'if_else_branches',
          value: { branches }
        })
      }
    />
  );
}

function renderStateWriteField({ adapter, block }: SchemaFieldRendererProps) {
  const value = adapter.getValue(block.path);
  const binding = getBindingValue<
    Array<{ path: string[]; operator: string; source: string[] | null }>
  >(value, 'state_write', []);

  return (
    <StateWriteField
      ariaLabel={block.label}
      options={getSelectorOptions(adapter)}
      value={binding as never}
      onChange={(nextValue) =>
        adapter.setValue(block.path, {
          kind: 'state_write',
          value: nextValue
        })
      }
    />
  );
}

function renderVariableAssignmentField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  const value = adapter.getValue(block.path);
  const binding = getBindingValue<VariableAssignmentValue[]>(
    value,
    'state_write',
    []
  );
  const conversationVariables =
    (adapter.getDerived('conversationVariables') as
      | AgentFlowConversationVariable[]
      | null
      | undefined) ?? [];

  return (
    <VariableAssignmentField
      ariaLabel={block.label}
      conversationVariables={conversationVariables}
      selectorOptions={getSelectorOptions(adapter)}
      value={binding}
      onChange={(nextValue) =>
        adapter.setValue(block.path, {
          kind: 'state_write',
          value: nextValue
        })
      }
    />
  );
}

function renderOutputContractDefinitionField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  const value = adapter.getValue(block.path);
  const outputs = Array.isArray(value)
    ? (value as FlowNodeDocument['outputs'])
    : [];
  const node = adapter.getDerived('node') as FlowNodeDocument | undefined;

  return (
    <OutputContractDefinitionField
      selectorForKey={node?.type === 'code' ? codeOutputSelector : undefined}
      syncTitleWithKey={node?.type === 'code'}
      value={outputs}
      onChange={(nextValue) => adapter.setValue(block.path, nextValue)}
    />
  );
}

function renderCodeSourceField({ adapter, block }: SchemaFieldRendererProps) {
  return (
    <CodeSourceField
      label={block.label}
      value={adapter.getValue(block.path)}
      onChange={(nextValue) => adapter.setValue(block.path, nextValue)}
    />
  );
}

function renderStartInputFieldsField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  return (
    <StartInputFieldsField
      value={adapter.getValue(block.path)}
      onChange={(nextValue) => adapter.setValue(block.path, nextValue)}
    />
  );
}

function renderStartModelListField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  return (
    <StartModelListField
      value={adapter.getValue(block.path)}
      onChange={(nextValue) => adapter.setValue(block.path, nextValue)}
    />
  );
}

function renderDataModelQueryField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  const value = adapter.getValue(block.path);
  const binding = normalizeDataModelQueryBindingValue(
    getBindingValue(value, 'data_model_query', DATA_MODEL_QUERY_DEFAULT_VALUE)
  );
  const fields =
    (adapter.getValue('config.data_model_fields') as
      | AgentFlowDataModelFieldOption[]
      | null
      | undefined) ?? [];
  const dataModelCode = adapter.getValue('config.data_model_code');

  return (
    <DataModelQueryField
      ariaLabel={block.label}
      hasDataModelSelected={
        typeof dataModelCode === 'string' && dataModelCode.trim().length > 0
      }
      fields={fields}
      selectorOptions={getSelectorOptions(adapter)}
      value={binding}
      includePagination={
        (adapter.getDerived('node') as FlowNodeDocument | null | undefined)
          ?.type === 'data_model_list'
      }
      onChange={(nextValue) =>
        adapter.setValue(block.path, {
          kind: 'data_model_query',
          value: normalizeDataModelQueryBindingValue(nextValue)
        })
      }
    />
  );
}

function renderHttpRequestKeyValuesField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  return (
    <HttpRequestKeyValuesField
      ariaLabel={block.label}
      options={getSelectorOptions(adapter)}
      value={adapter.getValue(block.path)}
      onChange={(nextValue) => adapter.setValue(block.path, nextValue)}
    />
  );
}

function renderHttpRequestEndpointField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  const methodValue = adapter.getValue('config.method');
  const urlValue = adapter.getValue(block.path);

  return (
    <div className="agent-flow-http-request-endpoint">
      <Select
        aria-label={i18nText('agentFlow', 'auto.request_method')}
        className="agent-flow-http-request-endpoint__method"
        options={HTTP_REQUEST_METHOD_OPTIONS}
        value={typeof methodValue === 'string' ? methodValue : 'GET'}
        onChange={(nextValue) => adapter.setValue('config.method', nextValue)}
      />
      <HttpRequestTemplateInput
        ariaLabel={block.label}
        label={block.label}
        options={getSelectorOptions(adapter)}
        value={typeof urlValue === 'string' ? urlValue : ''}
        onChange={(nextValue) => adapter.setValue(block.path, nextValue)}
      />
    </div>
  );
}

function renderHttpRequestBodyField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  return (
    <HttpRequestBodyField
      binaryValue={adapter.getValue('bindings.binary')}
      bodyType={adapter.getValue(block.path)}
      bodyValue={adapter.getValue('bindings.body')}
      formDataValue={adapter.getValue('bindings.form_data')}
      options={getSelectorOptions(adapter)}
      urlencodedValue={adapter.getValue('bindings.urlencoded')}
      onBinaryChange={(nextValue) =>
        adapter.setValue('bindings.binary', nextValue)
      }
      onBodyChange={(nextValue) => adapter.setValue('bindings.body', nextValue)}
      onBodyTypeChange={(nextValue) => adapter.setValue(block.path, nextValue)}
      onFormDataChange={(nextValue) =>
        adapter.setValue('bindings.form_data', nextValue)
      }
      onUrlencodedChange={(nextValue) =>
        adapter.setValue('bindings.urlencoded', nextValue)
      }
    />
  );
}

function renderHttpRequestCurlImportField({
  adapter
}: SchemaFieldRendererProps) {
  return (
    <HttpRequestCurlImportField
      onBodyChange={(nextValue) => adapter.setValue('bindings.body', nextValue)}
      onBodyTypeChange={(nextValue) =>
        adapter.setValue('config.body_type', nextValue)
      }
      onHeadersChange={(nextValue) =>
        adapter.setValue('bindings.headers', nextValue)
      }
      onMethodChange={(nextValue) =>
        adapter.setValue('config.method', nextValue)
      }
      onParamsChange={(nextValue) =>
        adapter.setValue('bindings.params', nextValue)
      }
      onUrlChange={(nextValue) => adapter.setValue('config.url', nextValue)}
    />
  );
}

export const agentFlowFieldRenderers = {
  text: renderTextField,
  static_select: renderStaticSelectField,
  switch: renderSwitchField,
  data_model: DataModelField,
  data_model_query: renderDataModelQueryField,
  code_source: renderCodeSourceField,
  llm_model: LlmModelField,
  llm_context_policy: renderLlmContextPolicyField,
  llm_external_reasoning_policy: renderLlmExternalReasoningPolicyField,
  llm_tool_registrations: LlmToolRegistrationsField,
  llm_prompt_messages: renderLlmPromptMessagesField,
  llm_response_format: LlmResponseFormatField,
  number: renderNumberField,
  selector: renderSelectorField,
  selector_list: renderSelectorListField,
  templated_text: renderTemplatedTextField,
  named_bindings: renderNamedBindingsField,
  templated_named_bindings: renderTemplatedNamedBindingsField,
  condition_group: renderConditionGroupField,
  if_else_branches: renderIfElseBranchesField,
  state_write: renderStateWriteField,
  variable_assignment: renderVariableAssignmentField,
  output_contract_definition: renderOutputContractDefinitionField,
  start_input_fields: renderStartInputFieldsField,
  start_model_list: renderStartModelListField,
  http_request_endpoint: renderHttpRequestEndpointField,
  http_request_key_values: renderHttpRequestKeyValuesField,
  http_request_body: renderHttpRequestBodyField,
  http_request_curl_import: renderHttpRequestCurlImportField,
  header_alias: ({ adapter, block }) => {
    const value = adapter.getValue(block.path);

    return (
      <Input
        aria-label={block.label}
        className="agent-flow-editor__inspector-title-input"
        value={typeof value === 'string' ? value : ''}
        onChange={(event) => adapter.setValue(block.path, event.target.value)}
      />
    );
  },
  header_description: ({ adapter, block }) => {
    const value = adapter.getValue(block.path);

    return (
      <Input.TextArea
        aria-label={block.label}
        autoSize={{ minRows: 1, maxRows: 3 }}
        className="agent-flow-editor__inspector-description-input"
        placeholder={i18nText('agentFlow', 'auto.add_description')}
        value={typeof value === 'string' ? value : ''}
        onChange={(event) => adapter.setValue(block.path, event.target.value)}
      />
    );
  }
} satisfies Record<string, SchemaFieldRenderer>;
