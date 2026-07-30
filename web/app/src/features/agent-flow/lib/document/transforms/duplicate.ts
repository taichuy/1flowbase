import type {
  FlowAuthoringDocument,
  FlowBinding,
  FlowConditionExpressionDocument,
  FlowConditionGroupDocument,
  FlowConditionRuleDocument,
  FlowNodeDocument
} from '@1flowbase/flow-schema';

import { getNodeById } from '../selectors';
import { isConditionGroup, isConditionRule } from '../../if-else-branches';
import { remapTemplateSelectorTokens } from '../../template-binding';
import { remapDataModelQueryBinding } from '../../data-model-query-binding';
import { remapNamedBindingEntry } from '../../named-binding-expressions';
import { i18nText } from '../../../../../shared/i18n/text';

function collectDuplicatedNodeIds(
  document: FlowAuthoringDocument,
  rootNodeId: string
) {
  const queue = [rootNodeId];
  const collectedIds: string[] = [];

  while (queue.length > 0) {
    const currentNodeId = queue.shift();

    if (!currentNodeId || collectedIds.includes(currentNodeId)) {
      continue;
    }

    collectedIds.push(currentNodeId);

    for (const candidate of document.graph.nodes) {
      if (candidate.containerId === currentNodeId) {
        queue.push(candidate.id);
      }
    }
  }

  return collectedIds;
}

export function getDuplicatedNodeId(existingIds: string[], sourceNodeId: string) {
  let nextId = `${sourceNodeId}-copy`;
  let index = 2;

  while (existingIds.includes(nextId)) {
    nextId = `${sourceNodeId}-copy-${index}`;
    index += 1;
  }

  return nextId;
}

function getDuplicatedEdgeId(existingIds: string[], sourceEdgeId: string) {
  let nextId = `${sourceEdgeId}-copy`;
  let index = 2;

  while (existingIds.includes(nextId)) {
    nextId = `${sourceEdgeId}-copy-${index}`;
    index += 1;
  }

  return nextId;
}

function remapSelector(
  selector: string[],
  idMap: Map<string, string>
) {
  if (selector.length === 0 || !idMap.has(selector[0])) {
    return selector;
  }

  return [idMap.get(selector[0])!, ...selector.slice(1)];
}

function remapConditionRule(
  rule: FlowConditionRuleDocument,
  idMap: Map<string, string>
): FlowConditionRuleDocument {
  return {
    ...rule,
    left: remapSelector(rule.left, idMap),
    right:
      rule.right?.kind === 'selector'
        ? { ...rule.right, selector: remapSelector(rule.right.selector, idMap) }
        : rule.right
  };
}

function remapConditionExpression(
  condition: FlowConditionExpressionDocument,
  idMap: Map<string, string>
): FlowConditionExpressionDocument {
  if (isConditionGroup(condition)) {
    return remapConditionGroup(condition, idMap);
  }

  return isConditionRule(condition)
    ? remapConditionRule(condition, idMap)
    : condition;
}

function remapConditionGroup(
  group: FlowConditionGroupDocument,
  idMap: Map<string, string>
): FlowConditionGroupDocument {
  return {
    ...group,
    conditions: group.conditions.map((condition) =>
      remapConditionExpression(condition, idMap)
    )
  };
}

function remapStateWriteValue(
  value: Extract<FlowBinding, { kind: 'state_write' }>['value'][number]['value'],
  idMap: Map<string, string>
) {
  if (!value) {
    return value;
  }

  if (value.kind === 'selector') {
    return {
      ...value,
      selector: remapSelector(value.selector, idMap)
    };
  }

  if (value.kind === 'templated_text') {
    return {
      ...value,
      value: remapTemplateSelectorTokens(value.value, idMap)
    };
  }

  return value;
}

function remapBinding(
  binding: FlowBinding,
  idMap: Map<string, string>
): FlowBinding {
  switch (binding.kind) {
    case 'templated_text':
      return {
        ...binding,
        value: remapTemplateSelectorTokens(binding.value, idMap)
      };
    case 'i18n_text':
      return { ...binding, value: { ...binding.value } };
    case 'selector':
      return {
        ...binding,
        value: remapSelector(binding.value, idMap)
      };
    case 'selector_list':
      return {
        ...binding,
        value: binding.value.map((selector) => remapSelector(selector, idMap))
      };
    case 'prompt_messages':
      return {
        ...binding,
        value: binding.value.map((message) => ({
          ...message,
          content: {
            ...message.content,
            value: remapTemplateSelectorTokens(message.content.value, idMap)
          }
        }))
      };
    case 'named_bindings':
      return {
        ...binding,
        value: binding.value.map((entry) =>
          remapNamedBindingEntry(entry, idMap)
        )
      };
    case 'condition_group':
      return {
        ...binding,
        value: remapConditionGroup(binding.value, idMap)
      };
    case 'if_else_branches':
      return {
        ...binding,
        value: {
          branches: binding.value.branches.map((branch) => ({
            ...branch,
            condition: branch.condition
              ? remapConditionGroup(branch.condition, idMap)
              : undefined
          }))
        }
      };
    case 'state_write':
      return {
        ...binding,
        value: binding.value.map((entry) => ({
          ...entry,
          source: entry.source ? remapSelector(entry.source, idMap) : null,
          value: remapStateWriteValue(entry.value, idMap)
        }))
      };
    case 'data_model_query':
      return remapDataModelQueryBinding(binding, (selector) =>
        remapSelector(selector, idMap)
      );
  }
}

function remapBindings(
  bindings: FlowNodeDocument['bindings'],
  idMap: Map<string, string>
) {
  return Object.fromEntries(
    Object.entries(bindings).map(([key, value]) => [key, remapBinding(value, idMap)])
  );
}

function getNodeIdsToDuplicate(
  document: FlowAuthoringDocument,
  sourceNode: FlowNodeDocument
) {
  return sourceNode.type === 'iteration' || sourceNode.type === 'loop'
    ? collectDuplicatedNodeIds(document, sourceNode.id)
    : [sourceNode.id];
}

export function duplicateNodeSubgraph(
  document: FlowAuthoringDocument,
  payload: { nodeId: string }
) {
  const sourceNode = getNodeById(document, payload.nodeId);

  if (!sourceNode) {
    return document;
  }

  const sourceIds = getNodeIdsToDuplicate(document, sourceNode);
  const existingNodeIds = document.graph.nodes.map((node) => node.id);
  const idMap = new Map(
    sourceIds.map((id) => [id, getDuplicatedNodeId(existingNodeIds, id)])
  );
  const duplicatedNodes = document.graph.nodes
    .filter((node) => sourceIds.includes(node.id))
    .map((node) => ({
      ...node,
      id: idMap.get(node.id)!,
      alias: i18nText("agentFlow", "auto.copy_option", { value1: node.alias }),
      containerId:
        node.containerId && idMap.has(node.containerId)
          ? idMap.get(node.containerId)!
          : node.containerId,
      position: { x: node.position.x + 48, y: node.position.y + 48 },
      bindings: remapBindings(node.bindings, idMap)
    }));
  const duplicatedEdgeIds = document.graph.edges.map((edge) => edge.id);
  const duplicatedEdges = document.graph.edges
    .filter(
      (edge) => sourceIds.includes(edge.source) && sourceIds.includes(edge.target)
    )
    .map((edge) => ({
      ...edge,
      id: getDuplicatedEdgeId(duplicatedEdgeIds, edge.id),
      source: idMap.get(edge.source)!,
      target: idMap.get(edge.target)!,
      containerId:
        edge.containerId && idMap.has(edge.containerId)
          ? idMap.get(edge.containerId)!
          : edge.containerId
    }));

  return {
    ...document,
    graph: {
      ...document.graph,
      nodes: [...document.graph.nodes, ...duplicatedNodes],
      edges: [...document.graph.edges, ...duplicatedEdges]
    }
  };
}
