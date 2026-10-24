import { type FlowAuthoringDocument } from '@1flowbase/flow-schema';
import type { FlowNodeDocument } from '@1flowbase/flow-schema';

import { getActiveNodeBindings } from '../data-model-query-binding';

import { parseTemplateSelectorTokens } from '../template-binding';

import { i18nText } from '../../../../shared/i18n/text';

import { pushFieldIssue, type AgentFlowIssue } from './issues';

interface AnswerPresentationReference {
  nodeId: string;
  outputKey: string;
}

function collectAnswerPresentationReferences(
  node: FlowNodeDocument
): AnswerPresentationReference[] {
  const binding = node.bindings.answer_template;

  if (!binding) {
    return [];
  }

  const selectors =
    binding.kind === 'selector'
      ? [binding.value]
      : binding.kind === 'templated_text'
        ? parseTemplateSelectorTokens(binding.value)
        : [];

  return selectors
    .filter((selector) => selector.length >= 2)
    .map((selector) => ({
      nodeId: selector[0],
      outputKey: selector.slice(1).join('.')
    }));
}

export function buildNodeDependencyMap(
  document: FlowAuthoringDocument,
  nodeIds: Set<string>
): Map<string, Set<string>> {
  const dependencies = new Map<string, Set<string>>();

  for (const node of document.graph.nodes) {
    dependencies.set(node.id, new Set());
  }

  for (const edge of document.graph.edges) {
    if (!nodeIds.has(edge.source) || !nodeIds.has(edge.target)) {
      continue;
    }
    dependencies.get(edge.target)?.add(edge.source);
  }

  for (const node of document.graph.nodes) {
    const nodeDependencies = dependencies.get(node.id);
    if (!nodeDependencies) {
      continue;
    }

    for (const [, binding] of getActiveNodeBindings(node)) {
      for (const selector of collectBindingSelectors(binding)) {
        const sourceNodeId = selector[0] ?? '';
        if (
          selector.length >= 2 &&
          !isRuntimeSelectorSource(sourceNodeId) &&
          nodeIds.has(sourceNodeId) &&
          sourceNodeId !== node.id
        ) {
          nodeDependencies.add(sourceNodeId);
        }
      }
    }
  }

  return dependencies;
}

function collectNodeAncestors(
  dependencies: Map<string, Set<string>>,
  nodeId: string,
  firstLaterNodeId: string
): Set<string> {
  const stack = [nodeId];
  const visited = new Set<string>();
  const ancestors = new Set<string>();

  while (stack.length > 0) {
    const current = stack.pop();
    if (!current || visited.has(current)) {
      continue;
    }
    visited.add(current);

    for (const dependency of dependencies.get(current) ?? []) {
      ancestors.add(dependency);
      if (dependency === firstLaterNodeId) return ancestors;
      stack.push(dependency);
    }
  }

  return ancestors;
}

function formatAnswerPresentationReference(
  reference: AnswerPresentationReference,
  nodeById: Map<string, FlowNodeDocument>
): string {
  const node = nodeById.get(reference.nodeId);
  const nodeLabel = node?.alias.trim() || reference.nodeId;

  return `${nodeLabel}.${reference.outputKey}`;
}

export function validateAnswerPresentationReferences(
  issues: AgentFlowIssue[],
  answerNode: FlowNodeDocument,
  nodeById: Map<string, FlowNodeDocument>,
  dependencies: Map<string, Set<string>>
) {
  const references = collectAnswerPresentationReferences(answerNode);
  const seen = new Set<string>();

  for (const reference of references) {
    const key = `${reference.nodeId}.${reference.outputKey}`;
    if (!seen.has(key)) {
      seen.add(key);
      continue;
    }

    pushFieldIssue(
      issues,
      answerNode,
      'bindings.answer_template',
      i18nText('agentFlow', 'auto.answer_repeated_reference_output_variable'),
      i18nText(
        'agentFlow',
        'auto.same_output_variable_referenced_answer_template',
        { value1: formatAnswerPresentationReference(reference, nodeById) }
      )
    );
  }

  for (let index = 0; index < references.length; index += 1) {
    const current = references[index];
    let ancestors: Set<string> | undefined;
    for (
      let laterIndex = index + 1;
      laterIndex < references.length;
      laterIndex += 1
    ) {
      const later = references[laterIndex];
      // A first-candidate hit ends this reference. Otherwise the completed
      // traversal can answer all remaining candidates without another DFS.
      ancestors ??= collectNodeAncestors(
        dependencies,
        current.nodeId,
        later.nodeId
      );
      if (!ancestors.has(later.nodeId)) {
        continue;
      }

      pushFieldIssue(
        issues,
        answerNode,
        'bindings.answer_template',
        i18nText(
          'agentFlow',
          'auto.answer_display_order_violates_execution_dependencies'
        ),
        i18nText(
          'agentFlow',
          'auto.template_puts_front_former_depends_execution_result_latter_adjust_answer',
          {
            value1: formatAnswerPresentationReference(current, nodeById),
            value2: formatAnswerPresentationReference(later, nodeById)
          }
        )
      );
      break;
    }
  }
}

export function validateTerminalTopology(
  issues: AgentFlowIssue[],
  document: FlowAuthoringDocument
) {
  const sourceNodeIds = new Set(
    document.graph.edges.map((edge) => edge.source)
  );
  for (const node of document.graph.nodes.filter(
    (candidate) => candidate.type === 'answer'
  )) {
    if (sourceNodeIds.has(node.id)) {
      issues.push({
        id: `${node.id}-terminal-outgoing-edge`,
        scope: 'node',
        level: 'error',
        nodeId: node.id,
        sectionKey: 'basics',
        fieldKey: null,
        title: i18nText('agentFlow', 'auto.terminal_connection_invalid'),
        message: i18nText(
          'agentFlow',
          'auto.terminal_connection_invalid_message'
        )
      });
    }
  }
}
import {
  collectBindingSelectors,
  isRuntimeSelectorSource
} from './binding-selectors';
