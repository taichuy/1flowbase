import {
  createDefaultAgentFlowDocument,
  type FlowAuthoringDocument
} from '@1flowbase/flow-schema';
import { describe, expect, test } from 'vitest';

import { createBuiltinCatalogNode } from '../fixtures/application-node-catalog';
import { buildNodePickerOptions } from '../../lib/plugin-node-definitions';
import { createNodeDocument } from '../../lib/document/node-factory';
import { duplicateNodeSubgraph } from '../../lib/document/transforms/duplicate';
import { getAgentFlowNodeTypeIcon } from '../../lib/node-type-icons';
import { getBuiltinNodeRuntimeContract } from '../../lib/node-definitions/contracts';
import { validateDocument } from '../../lib/validate-document';
import { createAgentFlowNodeSchemaAdapter } from '../../schema/node-schema-adapter';
import { resolveAgentFlowNodeSchema } from '../../schema/node-schema-registry';

const ORDERED_CANDIDATES = [
  ['node-llm', 'text'],
  ['node-llm', 'usage']
];

function appendAggregator(document: FlowAuthoringDocument) {
  const aggregator = createNodeDocument(
    'variable_aggregator',
    'node-variable-aggregator',
    620,
    220
  );

  return {
    ...document,
    graph: {
      nodes: [...document.graph.nodes, aggregator],
      edges: [
        ...document.graph.edges.filter((edge) => edge.id !== 'edge-llm-answer'),
        {
          id: 'edge-llm-aggregator',
          source: 'node-llm',
          target: aggregator.id,
          sourceHandle: null,
          targetHandle: null,
          containerId: null,
          points: []
        },
        {
          id: 'edge-aggregator-answer',
          source: aggregator.id,
          target: 'node-answer',
          sourceHandle: null,
          targetHandle: null,
          containerId: null,
          points: []
        }
      ]
    }
  } satisfies FlowAuthoringDocument;
}

describe('Variable Aggregator shared authoring fixtures', () => {
  test('AC-005 freezes the runtime contract and schema to ordered candidates with one value output', () => {
    const contract = getBuiltinNodeRuntimeContract('variable_aggregator');
    const schema = resolveAgentFlowNodeSchema('variable_aggregator');
    const configBlocks = JSON.stringify(schema.detail.tabs.config.blocks);

    expect(contract).not.toBeNull();
    expect(contract?.meta.type).toBe('variable_aggregator');
    expect(contract?.defaults.config).toEqual({});
    expect(contract?.defaults.bindings).toEqual({
      candidates: { kind: 'selector_list', value: [] }
    });
    expect(contract?.defaults.outputs).toEqual([
      expect.objectContaining({ key: 'value', valueType: 'unknown' })
    ]);
    expect(contract?.defaults.outputs).toHaveLength(1);
    expect(configBlocks).toContain('"path":"bindings.candidates"');
    expect(configBlocks).toContain('"renderer":"selector_list"');
    expect(getAgentFlowNodeTypeIcon('variable_aggregator')).not.toBeNull();
  });

  test('AC-005 creates and saves candidates without changing their priority order', () => {
    const initialDocument = appendAggregator(
      createDefaultAgentFlowDocument({ flowId: 'agent-flow-aggregator' })
    );
    let savedDocument: FlowAuthoringDocument = initialDocument;
    const adapter = createAgentFlowNodeSchemaAdapter({
      document: initialDocument,
      nodeId: 'node-variable-aggregator',
      setWorkingDocument(update) {
        savedDocument =
          typeof update === 'function' ? update(savedDocument) : update;
      },
      dispatch() {}
    });

    adapter.setValue('bindings.candidates', {
      kind: 'selector_list',
      value: ORDERED_CANDIDATES
    });

    expect(
      savedDocument.graph.nodes.find(
        (node) => node.id === 'node-variable-aggregator'
      )?.bindings.candidates
    ).toEqual({ kind: 'selector_list', value: ORDERED_CANDIDATES });
  });

  test('AC-005 duplicates the node with an independent ordered candidate list', () => {
    const document = appendAggregator(
      createDefaultAgentFlowDocument({ flowId: 'duplicate-aggregator' })
    );
    const source = document.graph.nodes.find(
      (node) => node.id === 'node-variable-aggregator'
    );

    if (!source) {
      throw new Error('Variable Aggregator fixture is missing');
    }

    source.bindings.candidates = {
      kind: 'selector_list',
      value: ORDERED_CANDIDATES
    };

    const duplicated = duplicateNodeSubgraph(document, {
      nodeId: source.id
    });
    const copy = duplicated.graph.nodes.find(
      (node) => node.id === 'node-variable-aggregator-copy'
    );

    expect(copy?.bindings.candidates).toEqual(source.bindings.candidates);
    expect(copy?.bindings.candidates).not.toBe(source.bindings.candidates);
    expect(copy?.outputs).toEqual(source.outputs);
  });

  test('AC-006 reports an incomplete candidate list and accepts ordered upstream selectors', () => {
    const document = appendAggregator(
      createDefaultAgentFlowDocument({ flowId: 'validate-aggregator' })
    );
    const aggregator = document.graph.nodes.find(
      (node) => node.id === 'node-variable-aggregator'
    );

    if (!aggregator) {
      throw new Error('Variable Aggregator fixture is missing');
    }

    expect(
      validateDocument(document).some(
        (issue) =>
          issue.nodeId === aggregator.id &&
          issue.fieldKey === 'bindings.candidates'
      )
    ).toBe(true);

    aggregator.bindings.candidates = {
      kind: 'selector_list',
      value: [[]]
    };

    expect(
      validateDocument(document).some(
        (issue) =>
          issue.nodeId === aggregator.id &&
          issue.fieldKey === 'bindings.candidates'
      )
    ).toBe(true);

    aggregator.bindings.candidates = {
      kind: 'selector_list',
      value: ORDERED_CANDIDATES
    };

    expect(
      validateDocument(document).filter(
        (issue) =>
          issue.nodeId === aggregator.id &&
          issue.fieldKey === 'bindings.candidates'
      )
    ).toEqual([]);
  });

  test('AC-006 exposes the registered builtin through the shared catalog picker', () => {
    const [option] = buildNodePickerOptions([
      createBuiltinCatalogNode('variable_aggregator', {
        title: 'Variable Aggregator',
        category: 'data'
      })
    ]);

    expect(option).toEqual(
      expect.objectContaining({
        kind: 'builtin',
        type: 'variable_aggregator',
        label: 'Variable Aggregator',
        disabled: false
      })
    );
    expect(createNodeDocument(option, 'node-from-picker')).toEqual(
      expect.objectContaining({
        id: 'node-from-picker',
        type: 'variable_aggregator',
        bindings: {
          candidates: { kind: 'selector_list', value: [] }
        }
      })
    );
  });
});
