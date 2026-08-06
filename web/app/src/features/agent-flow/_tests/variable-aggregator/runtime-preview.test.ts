import { describe, expect, test } from 'vitest';

import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';

import {
  buildNodeDebugPreviewInput,
  buildNodeDebugPreviewPlan
} from '../../api/runtime';
import { createNodeDocument } from '../../lib/document/node-factory';

function createVariableAggregatorPreviewDocument() {
  const document = createDefaultAgentFlowDocument({
    flowId: 'variable-aggregator-preview'
  });
  const aggregator = createNodeDocument(
    'variable_aggregator',
    'node-variable-aggregator'
  );

  aggregator.bindings.groups = {
    kind: 'variable_groups',
    value: [
      {
        key: 'group1',
        valueType: 'string',
        candidates: [
          ['node-llm', 'text'],
          ['node-start', 'query']
        ]
      },
      {
        key: 'group2',
        valueType: 'object',
        candidates: [
          [],
          ['node-llm', 'usage'],
          ['node-start'],
          ['node-start', 'query']
        ]
      }
    ]
  };
  aggregator.outputs = [
    { key: 'group1', title: 'group1', valueType: 'string' },
    { key: 'group2', title: 'group2', valueType: 'object' }
  ];
  document.graph.nodes.push(aggregator);

  return document;
}

describe('Variable Aggregator runtime preview', () => {
  test('AC-014 flattens normalized group candidates into the node preview plan in priority order', () => {
    const document = createVariableAggregatorPreviewDocument();
    const plan = buildNodeDebugPreviewPlan(
      document,
      'node-variable-aggregator'
    );

    expect(
      plan.missing_fields.map((field) => `${field.nodeId}.${field.key}`)
    ).toEqual(['node-llm.text', 'node-start.query', 'node-llm.usage']);
    expect(plan.input_payload).toEqual({});
  });

  test('AC-014 collects every valid candidate through the public preview input entry', () => {
    const document = createVariableAggregatorPreviewDocument();
    const preview = buildNodeDebugPreviewInput(
      document,
      'node-variable-aggregator'
    );

    expect(Object.keys(preview.input_payload['node-llm'] ?? {})).toEqual([
      'text',
      'usage'
    ]);
    expect(Object.keys(preview.input_payload['node-start'] ?? {})).toEqual([
      'query'
    ]);
    expect(preview.input_payload).not.toHaveProperty('undefined');
  });
});
