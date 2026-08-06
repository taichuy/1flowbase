import type {
  FlowAuthoringDocument,
  FlowBinding,
  FlowNodeDocument,
  FlowNodeOutputDocument
} from '@1flowbase/flow-schema';
import type { ConsoleNodeLastRun } from '@1flowbase/api-client';

import { i18nText } from '../../../shared/i18n/text';
import {
  extractDataModelQuerySelectors,
  getActiveNodeBindings
} from '../lib/data-model-query-binding';
import {
  collectConditionSelectors,
  collectIfElseBranchSelectors
} from '../lib/if-else-branches';
import { extractNamedBindingSelectors } from '../lib/named-binding-expressions';
import { parseTemplateSelectorTokens } from '../lib/template-binding';
import type { AgentFlowEnvironmentVariable } from '../lib/variables/application-environment-variables';
import {
  conversationVariableNodeId,
  formatConversationVariableTitle,
  listConversationVariables
} from '../lib/variables/conversation-variables';
import {
  agentFlowSystemVariables,
  systemVariableNodeId
} from '../lib/variables/system-variables';
import {
  getNodeVariableOutputs,
  getStartInputFields
} from '../lib/variables/start-node-variables';

export interface NodeDebugPreviewVariableField {
  nodeId: string;
  nodeLabel: string;
  key: string;
  title: string;
  valueType: FlowNodeDocument['outputs'][number]['valueType'];
  value: unknown;
  inputPath?: string[];
}

export type NodeDebugPreviewVariableCache = Record<
  string,
  Record<string, unknown>
>;

export interface NodeDebugPreviewPlan {
  input_payload: Record<string, Record<string, unknown>>;
  missing_fields: NodeDebugPreviewVariableField[];
}

export interface NodeDebugVariableConfirmationPlan {
  input_payload: Record<string, Record<string, unknown>>;
  fields: NodeDebugPreviewVariableField[];
}

type StateWriteValueExpression =
  | { kind: 'constant'; value: unknown }
  | { kind: 'selector'; selector: string[] }
  | { kind: 'templated_text'; value: string };

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value));
}

export function extractNodePreviewVariableOutput(
  lastRun: ConsoleNodeLastRun
): Record<string, unknown> {
  const outputPayload = lastRun.node_run.output_payload;

  if (!isRecord(outputPayload)) {
    return {};
  }

  return outputPayload;
}

export function buildFlowDebugRunInput(
  document: FlowAuthoringDocument,
  inputValues?: Record<string, unknown>,
  environmentVariables: AgentFlowEnvironmentVariable[] = []
) {
  const startNode = document.graph.nodes.find((node) => node.type === 'start');
  const startPayload: Record<string, unknown> = {};
  const inputPayload: Record<string, Record<string, unknown>> = {
    [startNode?.id ?? 'node-start']: startPayload
  };

  for (const output of startNode ? getNodeVariableOutputs(startNode) : []) {
    if (output.valueType === 'protocol_context') {
      startPayload[output.key] = null;
      continue;
    }
    startPayload[output.key] =
      inputValues &&
      Object.prototype.hasOwnProperty.call(inputValues, output.key)
        ? inputValues[output.key]
        : buildPreviewValue(startNode, output.key);
  }

  if (environmentVariables.length > 0) {
    inputPayload.env = environmentVariables.reduce<Record<string, unknown>>(
      (payload, variable) => {
        payload[variable.name] = variable.value;
        return payload;
      },
      {}
    );
  }

  return {
    input_payload: inputPayload
  };
}

function normalizeSelectorPath(value: string[] | null | undefined) {
  if (!value || value.length < 2) {
    return null;
  }

  return value;
}

function extractTemplateSelectors(template: string) {
  return parseTemplateSelectorTokens(template);
}

function normalizeConfigSelectorPath(value: unknown) {
  if (!Array.isArray(value)) {
    return null;
  }

  const selector = value.filter(
    (segment): segment is string => typeof segment === 'string'
  );

  return selector.length >= 2 ? selector : null;
}

function configKeyCarriesSelector(key: string | undefined) {
  return (
    key === 'selector' ||
    Boolean(key?.endsWith('_selector')) ||
    Boolean(key?.endsWith('Selector'))
  );
}

function extractConfigReferenceSelectors(
  value: unknown,
  key?: string
): string[][] {
  const selectors: string[][] = [];

  if (typeof value === 'string') {
    selectors.push(...extractTemplateSelectors(value));
    return selectors;
  }

  if (configKeyCarriesSelector(key)) {
    const selector = normalizeConfigSelectorPath(value);

    if (selector) {
      selectors.push(selector);
    }
  }

  if (Array.isArray(value)) {
    return [
      ...selectors,
      ...value.flatMap((entry) => extractConfigReferenceSelectors(entry))
    ];
  }

  if (!isRecord(value)) {
    return selectors;
  }

  if (value.kind === 'selector') {
    const selector = normalizeConfigSelectorPath(value.selector);

    if (selector) {
      selectors.push(selector);
    }
  }

  return [
    ...selectors,
    ...Object.entries(value).flatMap(([entryKey, entryValue]) =>
      extractConfigReferenceSelectors(entryValue, entryKey)
    )
  ];
}

function normalizeStateWriteValueExpression(
  value: unknown
): StateWriteValueExpression | null {
  if (!isRecord(value)) {
    return null;
  }

  if (value.kind === 'constant') {
    return {
      kind: 'constant',
      value: value.value
    };
  }

  if (value.kind === 'selector') {
    const selector = normalizeConfigSelectorPath(value.selector);

    return selector ? { kind: 'selector', selector } : null;
  }

  if (value.kind === 'templated_text' && typeof value.value === 'string') {
    return {
      kind: 'templated_text',
      value: value.value
    };
  }

  return null;
}

function extractStateWriteValueSelectors(value: unknown): string[][] {
  const expression = normalizeStateWriteValueExpression(value);

  if (!expression) {
    return [];
  }

  if (expression.kind === 'selector') {
    return [expression.selector];
  }

  if (expression.kind === 'templated_text') {
    return extractTemplateSelectors(expression.value);
  }

  return [];
}

function extractSelectors(binding: FlowBinding): string[][] {
  switch (binding.kind) {
    case 'i18n_text':
      return [];
    case 'selector': {
      const selector = normalizeSelectorPath(binding.value);

      return selector ? [selector] : [];
    }
    case 'selector_list':
      return binding.value
        .map((value) => normalizeSelectorPath(value))
        .filter((value): value is string[] => value !== null);
    case 'variable_groups':
      return binding.value.flatMap((group) =>
        group.candidates
          .map((candidate) => normalizeSelectorPath(candidate))
          .filter((candidate): candidate is string[] => candidate !== null)
      );
    case 'prompt_messages':
      return binding.value.flatMap((message) =>
        extractTemplateSelectors(message.content.value)
      );
    case 'named_bindings':
      return extractNamedBindingSelectors(binding.value);
    case 'condition_group':
      return collectConditionSelectors(binding.value)
        .map((condition) => normalizeSelectorPath(condition))
        .filter((value): value is string[] => value !== null);
    case 'if_else_branches':
      return collectIfElseBranchSelectors(binding.value.branches)
        .map((condition) => normalizeSelectorPath(condition))
        .filter((value): value is string[] => value !== null);
    case 'state_write':
      return binding.value.flatMap((entry) => {
        const source = normalizeSelectorPath(entry.source);

        return [
          ...(source ? [source] : []),
          ...extractStateWriteValueSelectors(entry.value)
        ];
      });
    case 'data_model_query':
      return extractDataModelQuerySelectors(binding.value)
        .map((value) => normalizeSelectorPath(value))
        .filter((value): value is string[] => value !== null);
    case 'templated_text':
      return extractTemplateSelectors(binding.value);
  }
}

function collectNodeReferenceSelectors(node: FlowNodeDocument) {
  return [
    ...getActiveNodeBindings(node).flatMap(([, binding]) =>
      extractSelectors(binding)
    ),
    ...extractConfigReferenceSelectors(node.config)
  ];
}

function hasPreviewVariableValue(value: unknown) {
  return value !== undefined && value !== null && value !== '';
}

function findNodeOutput(node: FlowNodeDocument | undefined, outputKey: string) {
  return node
    ? getNodeVariableOutputs(node).find((output) => output.key === outputKey)
    : undefined;
}

function findExternalVariableOutput(
  document: FlowAuthoringDocument,
  nodeId: string,
  outputKey: string
) {
  if (nodeId === systemVariableNodeId) {
    return agentFlowSystemVariables.find((output) => output.key === outputKey);
  }

  if (nodeId === conversationVariableNodeId) {
    const variable = listConversationVariables(document).find(
      (entry) => entry.name === outputKey
    );

    return variable
      ? {
          key: variable.name,
          title: formatConversationVariableTitle(variable.name),
          valueType: variable.valueType
        }
      : undefined;
  }

  return undefined;
}

function selectorsMatch(left: string[] | undefined, right: string[]) {
  return (
    Array.isArray(left) &&
    left.length === right.length &&
    left.every((segment, index) => segment === right[index])
  );
}

function findNodeOutputBySelector(
  document: FlowAuthoringDocument,
  node: FlowNodeDocument | undefined,
  selector: string[]
) {
  const outputKey = selectorOutputKey(selector);

  if (!node) {
    return findExternalVariableOutput(document, selector[0] ?? '', outputKey);
  }

  const selectorTail = selector.slice(1);
  const outputs = getNodeVariableOutputs(node);

  return (
    outputs.find((output) => selectorsMatch(output.selector, selectorTail)) ??
    outputs.find((output) => output.key === outputKey) ??
    (selectorTail.length === 1
      ? outputs.find((output) => output.key === selectorTail[0])
      : undefined)
  );
}

function selectorOutputKey(selector: string[]) {
  return selector[selector.length - 1] ?? '';
}

function writeNestedPreviewValue(
  target: Record<string, unknown>,
  path: string[],
  value: unknown
) {
  let current = target;

  for (const [index, segment] of path.entries()) {
    if (index === path.length - 1) {
      current[segment] = value;
      return;
    }

    const next = current[segment];

    if (!isRecord(next)) {
      current[segment] = {};
    }

    current = current[segment] as Record<string, unknown>;
  }
}

function writePreviewInputValue({
  inputPayload,
  sourceNodeId,
  sourceNode,
  sourceOutput,
  outputKey,
  value
}: {
  inputPayload: Record<string, Record<string, unknown>>;
  sourceNodeId: string;
  sourceNode: FlowNodeDocument | undefined;
  sourceOutput: FlowNodeOutputDocument | undefined;
  outputKey: string;
  value: unknown;
}) {
  const inputNodeId = sourceNode?.id ?? sourceNodeId;

  if (!inputNodeId) {
    return;
  }

  inputPayload[inputNodeId] ??= {};

  if (sourceNode?.type === 'code' && sourceOutput?.selector?.length) {
    writeNestedPreviewValue(
      inputPayload[inputNodeId],
      sourceOutput.selector,
      value
    );
    inputPayload[inputNodeId].error ??= null;
    return;
  }

  inputPayload[inputNodeId][outputKey] = value;
}

function readCachedOutputValue(
  payload: Record<string, unknown> | undefined,
  output: FlowNodeOutputDocument | undefined,
  outputKey: string
): { found: true; value: unknown } | { found: false } {
  if (!payload) {
    return { found: false };
  }

  if (Object.prototype.hasOwnProperty.call(payload, outputKey)) {
    return { found: true, value: payload[outputKey] };
  }

  if (!output?.selector?.length) {
    return { found: false };
  }

  let current: unknown = payload;

  for (const segment of output.selector) {
    if (
      !isRecord(current) ||
      !Object.prototype.hasOwnProperty.call(current, segment)
    ) {
      return { found: false };
    }

    current = current[segment];
  }

  return { found: true, value: current };
}

function buildNodePreviewVariableField({
  document,
  nodeId,
  outputKey,
  sourceNode,
  sourceOutput,
  value
}: {
  document: FlowAuthoringDocument;
  nodeId: string;
  outputKey: string;
  sourceNode?: FlowNodeDocument;
  sourceOutput?: FlowNodeOutputDocument;
  value: unknown;
}): NodeDebugPreviewVariableField {
  const node =
    sourceNode ?? document.graph.nodes.find((entry) => entry.id === nodeId);
  const output =
    sourceOutput ??
    findNodeOutput(node, outputKey) ??
    findExternalVariableOutput(document, nodeId, outputKey);
  const inputPath =
    node?.type === 'code' && sourceOutput?.selector?.length
      ? [...sourceOutput.selector]
      : undefined;

  return {
    nodeId,
    nodeLabel: node?.alias ?? nodeId,
    key: outputKey,
    title: output?.title ?? `${node?.alias ?? nodeId}.${outputKey}`,
    valueType: output?.valueType ?? inferPreviewValueType(value),
    value,
    inputPath
  };
}

function inferPreviewValueType(
  value: unknown
): FlowNodeDocument['outputs'][number]['valueType'] {
  if (Array.isArray(value)) {
    return 'array';
  }

  switch (typeof value) {
    case 'boolean':
      return 'boolean';
    case 'number':
      return 'number';
    case 'string':
      return 'string';
    case 'object':
      return value === null ? 'unknown' : 'json';
    default:
      return 'unknown';
  }
}

function buildMissingPreviewField(
  document: FlowAuthoringDocument,
  nodeId: string,
  outputKey: string
): NodeDebugPreviewVariableField {
  const node = document.graph.nodes.find((entry) => entry.id === nodeId);

  return buildNodePreviewVariableField({
    document,
    nodeId,
    outputKey,
    sourceNode: node,
    sourceOutput: findNodeOutput(node, outputKey),
    value: buildPreviewValue(node, outputKey)
  });
}

function isRequiredStartPreviewKey(node: FlowNodeDocument, outputKey: string) {
  if (outputKey === 'query') {
    return true;
  }

  return getStartInputFields(node).some(
    (field) => field.key === outputKey && field.required
  );
}

function canUseStartPreviewDefault(
  node: FlowNodeDocument | undefined,
  outputKey: string
) {
  return node?.type === 'start' && !isRequiredStartPreviewKey(node, outputKey);
}

function buildStringPreviewValue(
  node: FlowNodeDocument | undefined,
  outputKey: string
) {
  if (
    node?.type === 'start' &&
    (outputKey === 'query' ||
      outputKey === 'system' ||
      outputKey === 'model' ||
      outputKey === 'reasoning_effort')
  ) {
    return '';
  }

  if (outputKey === 'text' || outputKey === 'answer') {
    return i18nText('agentFlow', 'auto.debug_preview_output');
  }

  return i18nText('agentFlow', 'auto.debug_preview_value', {
    value1: node?.alias ?? i18nText('agentFlow', 'auto.fallback_node_label'),
    value2: outputKey
  });
}

function buildPreviewValue(
  node: FlowNodeDocument | undefined,
  outputKey: string
) {
  const startInputField =
    node?.type === 'start'
      ? getStartInputFields(node).find((field) => field.key === outputKey)
      : undefined;

  if (startInputField?.defaultValue !== undefined) {
    return startInputField.defaultValue;
  }

  if (node?.type === 'start' && outputKey === 'max_output_tokens') {
    return null;
  }

  const output = node
    ? getNodeVariableOutputs(node).find((entry) => entry.key === outputKey)
    : undefined;

  if (output?.valueType.startsWith('array')) {
    return [];
  }

  switch (output?.valueType) {
    case 'number':
      return 1;
    case 'boolean':
      return true;
    case 'json':
    case 'unknown':
      return {};
    case 'string':
    default:
      return buildStringPreviewValue(node, outputKey);
  }
}

export function buildNodeDebugPreviewInput(
  document: FlowAuthoringDocument,
  nodeId: string
) {
  const node = document.graph.nodes.find((entry) => entry.id === nodeId);
  const inputPayload: Record<string, Record<string, unknown>> = {};

  if (!node) {
    return { input_payload: inputPayload };
  }

  const selectors = collectNodeReferenceSelectors(node);

  for (const selector of selectors) {
    const sourceNodeId = selector[0] ?? '';
    const outputKey = selectorOutputKey(selector);
    const sourceNode = document.graph.nodes.find(
      (entry) => entry.id === sourceNodeId
    );
    const sourceOutput = findNodeOutputBySelector(
      document,
      sourceNode,
      selector
    );

    writePreviewInputValue({
      inputPayload,
      sourceNodeId,
      sourceNode,
      sourceOutput,
      outputKey,
      value: buildPreviewValue(sourceNode, outputKey)
    });
  }

  return { input_payload: inputPayload };
}

export function buildNodeDebugPreviewPlan(
  document: FlowAuthoringDocument,
  nodeId: string,
  variableCache: NodeDebugPreviewVariableCache = {}
): NodeDebugPreviewPlan {
  const node = document.graph.nodes.find((entry) => entry.id === nodeId);
  const inputPayload: Record<string, Record<string, unknown>> = {};
  const missingFields: NodeDebugPreviewVariableField[] = [];

  if (!node) {
    return { input_payload: inputPayload, missing_fields: missingFields };
  }

  if (node.type === 'start') {
    const visitedStartKeys = new Set<string>();

    for (const output of getNodeVariableOutputs(node)) {
      if (visitedStartKeys.has(output.key)) {
        continue;
      }

      visitedStartKeys.add(output.key);

      const cachedOutput = readCachedOutputValue(
        variableCache[node.id],
        output,
        output.key
      );

      if (cachedOutput.found && hasPreviewVariableValue(cachedOutput.value)) {
        inputPayload[node.id] ??= {};
        inputPayload[node.id][output.key] = cachedOutput.value;
        continue;
      }

      inputPayload[node.id] ??= {};
      inputPayload[node.id][output.key] = buildPreviewValue(node, output.key);

      if (isRequiredStartPreviewKey(node, output.key)) {
        missingFields.push(
          buildMissingPreviewField(document, node.id, output.key)
        );
      }
    }

    return { input_payload: inputPayload, missing_fields: missingFields };
  }

  const selectors = collectNodeReferenceSelectors(node);
  const visited = new Set<string>();

  for (const selector of selectors) {
    const sourceNodeId = selector[0] ?? '';
    const outputKey = selectorOutputKey(selector);
    const cacheKey = selector.join('.');
    const sourceNode = document.graph.nodes.find(
      (entry) => entry.id === sourceNodeId
    );
    const sourceOutput = findNodeOutputBySelector(
      document,
      sourceNode,
      selector
    );
    const cachedOutput = readCachedOutputValue(
      variableCache[sourceNodeId],
      sourceOutput,
      outputKey
    );

    if (visited.has(cacheKey)) {
      continue;
    }

    visited.add(cacheKey);

    if (cachedOutput.found && hasPreviewVariableValue(cachedOutput.value)) {
      writePreviewInputValue({
        inputPayload,
        sourceNodeId,
        sourceNode,
        sourceOutput,
        outputKey,
        value: cachedOutput.value
      });
      continue;
    }

    if (canUseStartPreviewDefault(sourceNode, outputKey)) {
      writePreviewInputValue({
        inputPayload,
        sourceNodeId,
        sourceNode,
        sourceOutput,
        outputKey,
        value: buildPreviewValue(sourceNode, outputKey)
      });
      continue;
    }

    missingFields.push(
      buildMissingPreviewField(document, sourceNodeId, outputKey)
    );
  }

  return { input_payload: inputPayload, missing_fields: missingFields };
}

export function buildNodeDebugVariableConfirmationPlan(
  document: FlowAuthoringDocument,
  nodeId: string,
  variableCache: NodeDebugPreviewVariableCache = {}
): NodeDebugVariableConfirmationPlan {
  const node = document.graph.nodes.find((entry) => entry.id === nodeId);
  const inputPayload: Record<string, Record<string, unknown>> = {};
  const fields: NodeDebugPreviewVariableField[] = [];

  if (!node) {
    return { input_payload: inputPayload, fields };
  }

  if (node.type === 'start') {
    const visitedStartKeys = new Set<string>();

    for (const output of getNodeVariableOutputs(node)) {
      if (visitedStartKeys.has(output.key)) {
        continue;
      }

      visitedStartKeys.add(output.key);

      const cachedOutput = readCachedOutputValue(
        variableCache[node.id],
        output,
        output.key
      );
      const value = cachedOutput.found
        ? cachedOutput.value
        : buildPreviewValue(node, output.key);

      writePreviewInputValue({
        inputPayload,
        sourceNodeId: node.id,
        sourceNode: node,
        sourceOutput: output,
        outputKey: output.key,
        value
      });
      fields.push(
        buildNodePreviewVariableField({
          document,
          nodeId: node.id,
          outputKey: output.key,
          sourceNode: node,
          sourceOutput: output,
          value
        })
      );
    }

    return { input_payload: inputPayload, fields };
  }

  const selectors = collectNodeReferenceSelectors(node);
  const visited = new Set<string>();

  for (const selector of selectors) {
    const sourceNodeId = selector[0] ?? '';
    const outputKey = selectorOutputKey(selector);
    const cacheKey = selector.join('.');
    const sourceNode = document.graph.nodes.find(
      (entry) => entry.id === sourceNodeId
    );
    const sourceOutput = findNodeOutputBySelector(
      document,
      sourceNode,
      selector
    );

    if (visited.has(cacheKey)) {
      continue;
    }

    visited.add(cacheKey);

    const cachedOutput = readCachedOutputValue(
      variableCache[sourceNodeId],
      sourceOutput,
      outputKey
    );
    const value = cachedOutput.found
      ? cachedOutput.value
      : buildPreviewValue(sourceNode, outputKey);

    writePreviewInputValue({
      inputPayload,
      sourceNodeId,
      sourceNode,
      sourceOutput,
      outputKey,
      value
    });
    fields.push(
      buildNodePreviewVariableField({
        document,
        nodeId: sourceNodeId,
        outputKey,
        sourceNode,
        sourceOutput,
        value
      })
    );
  }

  return { input_payload: inputPayload, fields };
}
