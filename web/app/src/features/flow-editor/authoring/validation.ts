import {
  validatePublicOutputKey,
  type FlowAuthoringDocument
} from '@1flowbase/flow-schema';

import type { InspectorSectionKey } from './node-definition-types';

export interface FlowAuthoringIssue {
  id: string;
  scope: 'global' | 'node';
  level: 'error' | 'warning';
  nodeId: string | null;
  sectionKey: InspectorSectionKey | null;
  fieldKey: string | null;
  title: string;
  message: string;
}

export function validateAuthoringDocument(
  document: FlowAuthoringDocument
): FlowAuthoringIssue[] {
  const issues: FlowAuthoringIssue[] = [];
  const nodeIds = new Set(document.graph.nodes.map((node) => node.id));

  for (const edge of document.graph.edges) {
    if (nodeIds.has(edge.source) && nodeIds.has(edge.target)) {
      continue;
    }

    issues.push({
      id: `${edge.id}-dangling`,
      scope: nodeIds.has(edge.source) ? 'node' : 'global',
      level: 'warning',
      nodeId: nodeIds.has(edge.source) ? edge.source : null,
      sectionKey: nodeIds.has(edge.source) ? 'basics' : null,
      fieldKey: null,
      title: 'Invalid node connection',
      message: 'The connection refers to a node that is no longer present.'
    });
  }

  for (const node of document.graph.nodes) {
    const seenOutputKeys = new Set<string>();

    for (const output of node.outputs) {
      const outputKey = output.key.trim();
      const validation = validatePublicOutputKey(outputKey);

      if (!outputKey || seenOutputKeys.has(outputKey) || !validation.ok) {
        issues.push({
          id: `${node.id}-output-contract-${outputKey || 'empty'}`,
          scope: 'node',
          level: 'error',
          nodeId: node.id,
          sectionKey: 'outputs',
          fieldKey: 'config.output_contract',
          title: 'Invalid output contract',
          message: !outputKey
            ? 'Output keys cannot be empty.'
            : seenOutputKeys.has(outputKey)
              ? 'Output keys must be unique.'
              : 'Output keys cannot use reserved names.'
        });
      }

      seenOutputKeys.add(outputKey);
    }

    if (
      node.type !== 'start' &&
      node.type !== 'workflow_start' &&
      !document.graph.edges.some(
        (edge) => edge.target === node.id && nodeIds.has(edge.source)
      )
    ) {
      issues.push({
        id: `${node.id}-orphan`,
        scope: 'node',
        level: 'warning',
        nodeId: node.id,
        sectionKey: 'basics',
        fieldKey: null,
        title: 'Node is not connected',
        message: 'The node has no valid incoming connection.'
      });
    }
  }

  return issues;
}
