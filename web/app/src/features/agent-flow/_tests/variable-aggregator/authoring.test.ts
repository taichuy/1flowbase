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
import {
  isVariableAggregatorCandidateTypeMismatchIssue,
  validateDocument,
  VARIABLE_AGGREGATOR_CANDIDATE_TYPE_MISMATCH_ISSUE_CODE
} from '../../lib/validate-document';
import { listVisibleSelectorOptions } from '../../lib/selector-options';
import { createAgentFlowNodeSchemaAdapter } from '../../schema/node-schema-adapter';
import { resolveAgentFlowNodeSchema } from '../../schema/node-schema-registry';

const STRING_GROUPS = [
  {
    key: 'group1',
    valueType: 'string' as const,
    candidates: [['node-llm', 'text']]
  }
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
  test('AC-008 freezes first-class variable groups in the control category', () => {
    const contract = getBuiltinNodeRuntimeContract('variable_aggregator');
    const schema = resolveAgentFlowNodeSchema('variable_aggregator');
    const configBlocks = JSON.stringify(schema.detail.tabs.config.blocks);

    expect(contract).not.toBeNull();
    expect(contract?.meta.type).toBe('variable_aggregator');
    expect(contract?.card.category).toBe('control');
    expect(contract?.defaults.config).toEqual({});
    expect(contract?.defaults.bindings).toEqual({
      groups: {
        kind: 'variable_groups',
        value: [{ key: 'group1', valueType: 'string', candidates: [[]] }]
      }
    });
    expect(contract?.defaults.outputs).toEqual([
      { key: 'group1', title: 'group1', valueType: 'string' }
    ]);
    expect(contract?.defaults.outputs).toHaveLength(1);
    expect(contract?.defaults.bindings).not.toHaveProperty('candidates');
    expect(contract?.defaults.outputs[0]).not.toHaveProperty('value');
    expect(configBlocks).toContain('"path":"bindings.groups"');
    expect(configBlocks).toContain('"renderer":"variable_groups"');
    expect(getAgentFlowNodeTypeIcon('variable_aggregator')).not.toBeNull();
  });

  test('AC-009 atomically saves group truth and materializes ordered outputs', () => {
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

    adapter.setValue('bindings.groups', {
      kind: 'variable_groups',
      value: [
        ...STRING_GROUPS,
        {
          key: 'group2',
          valueType: 'array',
          candidates: [['node-start', 'files']]
        }
      ]
    });

    const savedNode = savedDocument.graph.nodes.find(
      (node) => node.id === 'node-variable-aggregator'
    );

    expect(savedNode?.bindings.groups).toEqual({
      kind: 'variable_groups',
      value: [
        ...STRING_GROUPS,
        {
          key: 'group2',
          valueType: 'array',
          candidates: [['node-start', 'files']]
        }
      ]
    });
    expect(savedNode?.outputs).toEqual([
      { key: 'group1', title: 'group1', valueType: 'string' },
      { key: 'group2', title: 'group2', valueType: 'array' }
    ]);
    expect(
      validateDocument(savedDocument).filter(
        (issue) =>
          issue.nodeId === savedNode?.id &&
          issue.fieldKey === 'config.output_contract'
      )
    ).toEqual([]);
  });

  test('AC-011 duplicates group candidates independently and remaps internal selectors', () => {
    const document = appendAggregator(
      createDefaultAgentFlowDocument({ flowId: 'duplicate-aggregator' })
    );
    const source = document.graph.nodes.find(
      (node) => node.id === 'node-variable-aggregator'
    );

    if (!source) {
      throw new Error('Variable Aggregator fixture is missing');
    }

    source.bindings.groups = {
      kind: 'variable_groups',
      value: [
        {
          key: 'group1',
          valueType: 'string',
          candidates: [[source.id, 'group1']]
        }
      ]
    };

    const duplicated = duplicateNodeSubgraph(document, {
      nodeId: source.id
    });
    const copy = duplicated.graph.nodes.find(
      (node) => node.id === 'node-variable-aggregator-copy'
    );

    expect(copy?.bindings.groups).toEqual({
      kind: 'variable_groups',
      value: [
        {
          key: 'group1',
          valueType: 'string',
          candidates: [['node-variable-aggregator-copy', 'group1']]
        }
      ]
    });
    expect(copy?.bindings.groups).not.toBe(source.bindings.groups);
    expect(copy?.outputs).toEqual(source.outputs);
  });

  test('AC-010 rejects empty and incompatible candidates without legacy compatibility', () => {
    const document = appendAggregator(
      createDefaultAgentFlowDocument({ flowId: 'validate-aggregator' })
    );
    const aggregator = document.graph.nodes.find(
      (node) => node.id === 'node-variable-aggregator'
    );

    if (!aggregator) {
      throw new Error('Variable Aggregator fixture is missing');
    }

    const mismatchIssue = validateDocument(document).find((issue) =>
      isVariableAggregatorCandidateTypeMismatchIssue(issue)
    );

    expect(mismatchIssue?.id).toMatch(
      new RegExp(`^${VARIABLE_AGGREGATOR_CANDIDATE_TYPE_MISMATCH_ISSUE_CODE}:`)
    );

    aggregator.bindings.groups = {
      kind: 'variable_groups',
      value: [{ key: 'group1', valueType: 'string', candidates: [[]] }]
    };

    expect(
      validateDocument(document).some(
        (issue) =>
          issue.nodeId === aggregator.id && issue.fieldKey === 'bindings.groups'
      )
    ).toBe(true);

    aggregator.bindings.groups = {
      kind: 'variable_groups',
      value: [
        {
          key: 'group1',
          valueType: 'string',
          candidates: [['node-llm', 'usage']]
        }
      ]
    };

    expect(
      validateDocument(document).some(
        (issue) =>
          issue.nodeId === aggregator.id && issue.fieldKey === 'bindings.groups'
      )
    ).toBe(true);

    aggregator.bindings.groups = {
      kind: 'variable_groups',
      value: STRING_GROUPS
    };

    expect(
      validateDocument(document).filter((issue) =>
        isVariableAggregatorCandidateTypeMismatchIssue(issue)
      )
    ).toEqual([]);
  });

  test('AC-014 exposes materialized group outputs to downstream Answer selectors', () => {
    const document = appendAggregator(
      createDefaultAgentFlowDocument({ flowId: 'consume-aggregator' })
    );
    const aggregator = document.graph.nodes.find(
      (node) => node.id === 'node-variable-aggregator'
    );

    if (!aggregator) {
      throw new Error('Variable Aggregator fixture is missing');
    }

    aggregator.bindings.groups = {
      kind: 'variable_groups',
      value: STRING_GROUPS
    };
    aggregator.outputs = [
      { key: 'group1', title: 'group1', valueType: 'string' }
    ];

    expect(
      listVisibleSelectorOptions(document, 'node-answer').some(
        (option) =>
          option.value.join('.') === 'node-variable-aggregator.group1' &&
          option.valueType === 'string'
      )
    ).toBe(true);
  });

  test('AC-015 exposes the registered builtin without legacy candidates/value fields', () => {
    const [option] = buildNodePickerOptions([
      createBuiltinCatalogNode('variable_aggregator', {
        title: 'Variable Aggregator',
        category: 'control'
      })
    ]);

    expect(option).toEqual(
      expect.objectContaining({
        kind: 'builtin',
        type: 'variable_aggregator',
        label: 'Variable Aggregator',
        category: 'control',
        disabled: false
      })
    );
    const createdNode = createNodeDocument(option, 'node-from-picker');

    expect(createdNode).toEqual(
      expect.objectContaining({
        id: 'node-from-picker',
        type: 'variable_aggregator',
        bindings: {
          groups: {
            kind: 'variable_groups',
            value: [{ key: 'group1', valueType: 'string', candidates: [[]] }]
          }
        }
      })
    );
    expect(createdNode.bindings).not.toHaveProperty('candidates');
    expect(createdNode.outputs.map((output) => output.key)).toEqual(['group1']);
  });
});
