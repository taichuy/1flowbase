import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';

import { getNodeVariableOutputs } from './variables/start-node-variables';
import {
  agentFlowSystemVariables,
  systemVariableNodeId,
  workflowSystemVariables
} from './variables/system-variables';
import {
  getWorkflowTriggerVariables,
  workflowTriggerVariableNodeId
} from './variables/workflow-trigger-variables';
import {
  environmentVariableNodeId,
  formatEnvironmentVariableTitle,
  type AgentFlowEnvironmentVariable
} from './variables/application-environment-variables';
import {
  conversationVariableNodeId,
  formatConversationVariableTitle,
  listConversationVariables
} from './variables/conversation-variables';
import {
  getLlmVisibleInternalTools,
  getLlmVisibleInternalToolsEnabled,
  parseLlmToolSourceHandleId
} from './llm-node-config';
import { outputHasLlmContextSchema } from './output-contract/schema';
import { formatNodeVariableLabel } from './variables/variable-labels';
import { i18nText } from '../../../shared/i18n/text';

const visibleInternalLlmToolNodeId = 'visible_internal_llm_tool';
const visibleInternalLlmToolArgumentsKey = 'arguments';
const visibleInternalLlmToolDisplayNodeLabel = 'tool';

export interface FlowSelectorOption {
  nodeId: string;
  nodeLabel: string;
  outputKey: string;
  outputLabel: string;
  valueType: string;
  jsonSchema?: Record<string, unknown>;
  value: string[];
  displayLabel: string;
}

interface CascaderSelectorOption {
  label: string;
  value: string;
  children?: CascaderSelectorOption[];
}

function outputSelectorValue(
  nodeId: string,
  output: { key: string; selector?: string[] }
) {
  return [
    nodeId,
    ...(output.selector && output.selector.length > 0
      ? output.selector
      : [output.key])
  ];
}

function collectUpstreamNodeIds(
  document: FlowAuthoringDocument,
  nodeId: string
): Set<string> {
  const visited = new Set<string>();
  const queue = [nodeId];

  while (queue.length > 0) {
    const currentNodeId = queue.shift();

    if (!currentNodeId) {
      continue;
    }

    for (const edge of document.graph.edges) {
      if (edge.target !== currentNodeId || visited.has(edge.source)) {
        continue;
      }

      visited.add(edge.source);
      queue.push(edge.source);
    }
  }

  return visited;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function jsonSchemaPropertyValueType(property: unknown) {
  if (!isRecord(property)) {
    return 'json';
  }

  const type = property.type;

  if (type === 'string' || type === 'number' || type === 'boolean') {
    return type;
  }

  if (type === 'integer') {
    return 'number';
  }

  if (type === 'array') {
    return 'array';
  }

  return 'json';
}

function visibleInternalLlmToolArgumentOptions(
  document: FlowAuthoringDocument,
  nodeId: string
): FlowSelectorOption[] {
  const visitedNodeIds = new Set<string>();
  const queue = [nodeId];
  const optionsByField = new Map<string, FlowSelectorOption>();

  while (queue.length > 0) {
    const currentNodeId = queue.shift();

    if (!currentNodeId || visitedNodeIds.has(currentNodeId)) {
      continue;
    }

    visitedNodeIds.add(currentNodeId);

    for (const edge of document.graph.edges) {
      if (edge.target !== currentNodeId) {
        continue;
      }

      queue.push(edge.source);

      const connectorId = parseLlmToolSourceHandleId(edge.sourceHandle);

      if (!connectorId) {
        continue;
      }

      const sourceNode = document.graph.nodes.find(
        (node) => node.id === edge.source
      );

      if (
        sourceNode?.type !== 'llm' ||
        !getLlmVisibleInternalToolsEnabled(sourceNode.config)
      ) {
        continue;
      }

      const tool = getLlmVisibleInternalTools(sourceNode.config).find(
        (candidate) =>
          (candidate.connector_id || candidate.tool_name) === connectorId
      );
      const properties = isRecord(tool?.input_schema?.properties)
        ? tool.input_schema.properties
        : {};

      for (const [fieldKey, property] of Object.entries(properties)) {
        const trimmedFieldKey = fieldKey.trim();

        if (!trimmedFieldKey || optionsByField.has(trimmedFieldKey)) {
          continue;
        }

        optionsByField.set(trimmedFieldKey, {
          nodeId: visibleInternalLlmToolNodeId,
          nodeLabel: visibleInternalLlmToolDisplayNodeLabel,
          outputKey: trimmedFieldKey,
          outputLabel: trimmedFieldKey,
          valueType: jsonSchemaPropertyValueType(property),
          jsonSchema: isRecord(property) ? property : undefined,
          value: [
            visibleInternalLlmToolNodeId,
            visibleInternalLlmToolArgumentsKey,
            trimmedFieldKey
          ],
          displayLabel: `${visibleInternalLlmToolDisplayNodeLabel}.${trimmedFieldKey}`
        });
      }
    }
  }

  return [...optionsByField.values()];
}

export function listVisibleSelectorOptions(
  document: FlowAuthoringDocument,
  nodeId: string,
  environmentVariables: AgentFlowEnvironmentVariable[] = [],
  workflowTriggerContext: unknown = null
): FlowSelectorOption[] {
  const visibleNodeIds = collectUpstreamNodeIds(document, nodeId);
  const isWorkflowDocument = document.graph.nodes.some(
    (node) => node.type === 'workflow_start'
  );
  const triggerType =
    typeof workflowTriggerContext === 'object' &&
    workflowTriggerContext !== null &&
    'triggerType' in workflowTriggerContext
      ? workflowTriggerContext.triggerType
      : null;
  const systemVariables = isWorkflowDocument
    ? workflowSystemVariables
    : agentFlowSystemVariables;
  const systemOptions = systemVariables.map((variable) => ({
    nodeId: systemVariableNodeId,
    nodeLabel: i18nText('agentFlow', 'auto.system_variables'),
    outputKey: variable.key,
    outputLabel: variable.title,
    valueType: variable.valueType,
    jsonSchema: variable.jsonSchema,
    value: [systemVariableNodeId, variable.key],
    displayLabel: variable.title
  }));
  const triggerOptions = isWorkflowDocument
    ? getWorkflowTriggerVariables(triggerType).map((variable) => ({
        nodeId: workflowTriggerVariableNodeId,
        nodeLabel: i18nText('workflow', 'auto.trigger_variables'),
        outputKey: variable.key,
        outputLabel: variable.title,
        valueType: variable.valueType,
        value: [workflowTriggerVariableNodeId, variable.key],
        displayLabel: variable.title
      }))
    : [];
  const environmentOptions = environmentVariables.map((variable) => ({
    nodeId: environmentVariableNodeId,
    nodeLabel: i18nText('agentFlow', 'auto.environment_variables'),
    outputKey: variable.name,
    outputLabel: formatEnvironmentVariableTitle(variable.name),
    valueType: variable.value_type,
    value: [environmentVariableNodeId, variable.name],
    displayLabel: formatEnvironmentVariableTitle(variable.name)
  }));
  const conversationOptions = isWorkflowDocument
    ? []
    : listConversationVariables(document).map((variable) => ({
        nodeId: conversationVariableNodeId,
        nodeLabel: i18nText('agentFlow', 'auto.conversation_variables'),
        outputKey: variable.name,
        outputLabel: formatConversationVariableTitle(variable.name),
        valueType: variable.valueType,
        value: [conversationVariableNodeId, variable.name],
        displayLabel: formatConversationVariableTitle(variable.name)
      }));

  const nodeOptions = document.graph.nodes
    .filter((node) => visibleNodeIds.has(node.id))
    .flatMap((node) =>
      getNodeVariableOutputs(node).map((output) => {
        const outputLabel =
          node.type === 'variable_assigner' ? output.title : output.key;

        return {
          nodeId: node.id,
          nodeLabel: node.alias,
          outputKey: output.key,
          outputLabel,
          valueType: output.valueType,
          jsonSchema: output.jsonSchema,
          value: outputSelectorValue(node.id, output),
          displayLabel: formatNodeVariableLabel(node.alias, outputLabel)
        };
      })
    );

  const toolArgumentOptions = visibleInternalLlmToolArgumentOptions(
    document,
    nodeId
  );

  return [
    ...systemOptions,
    ...triggerOptions,
    ...environmentOptions,
    ...conversationOptions,
    ...toolArgumentOptions,
    ...nodeOptions
  ];
}

export function listLlmContextSelectorOptions(
  document: FlowAuthoringDocument,
  nodeId: string,
  environmentVariables: AgentFlowEnvironmentVariable[] = []
) {
  return listVisibleSelectorOptions(
    document,
    nodeId,
    environmentVariables
  ).filter((option) => outputHasLlmContextSchema(option));
}

export function toCascaderSelectorOptions(options: FlowSelectorOption[]) {
  const groups = new Map<
    string,
    {
      label: string;
      value: string;
      children: CascaderSelectorOption[];
    }
  >();

  for (const option of options) {
    if (!groups.has(option.nodeId)) {
      groups.set(option.nodeId, {
        label: option.nodeLabel,
        value: option.nodeId,
        children: []
      });
    }

    const group = groups.get(option.nodeId);
    if (!group) {
      continue;
    }

    appendCascaderSelectorPath(
      group.children,
      option.value.slice(1),
      option.outputLabel
    );
  }

  return [...groups.values()];
}

function appendCascaderSelectorPath(
  children: CascaderSelectorOption[],
  path: string[],
  outputLabel: string
) {
  const [segment, ...rest] = path;

  if (!segment) {
    return;
  }

  const label = rest.length === 0 ? outputLabel : segment;
  let child = children.find((candidate) => candidate.value === segment);

  if (!child) {
    child = { label, value: segment };
    children.push(child);
  } else if (rest.length === 0) {
    child.label = label;
  }

  if (rest.length > 0) {
    child.children ??= [];
    appendCascaderSelectorPath(child.children, rest, outputLabel);
  }
}

export function isSelectorVisible(
  document: FlowAuthoringDocument,
  nodeId: string,
  selector: string[],
  environmentVariables: AgentFlowEnvironmentVariable[] = [],
  workflowTriggerContext: unknown = null
): boolean {
  if (selector.length < 2) {
    return false;
  }

  return listVisibleSelectorOptions(
    document,
    nodeId,
    environmentVariables,
    workflowTriggerContext
  ).some(
    (option) =>
      option.value.length === selector.length &&
      option.value.every((segment, index) => segment === selector[index])
  );
}

export function encodeSelectorValue(value: string[]): string {
  return JSON.stringify(value);
}

export function decodeSelectorValue(value: string): string[] {
  try {
    const parsed = JSON.parse(value);

    return Array.isArray(parsed)
      ? parsed.filter((segment) => typeof segment === 'string')
      : [];
  } catch {
    return [];
  }
}
