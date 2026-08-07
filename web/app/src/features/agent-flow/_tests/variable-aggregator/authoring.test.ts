import {
  createDefaultAgentFlowDocument,
  type FlowAuthoringDocument,
  type FlowNodeDocument
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

  test('AC-010 atomically saves group truth and materializes same-name ordered outputs', () => {
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

  test('AC-011 rejects empty and incompatible candidates without legacy compatibility', () => {
    const document = appendAggregator(
      createDefaultAgentFlowDocument({ flowId: 'validate-aggregator' })
    );
    const aggregator = document.graph.nodes.find(
      (node) => node.id === 'node-variable-aggregator'
    );

    if (!aggregator) {
      throw new Error('Variable Aggregator fixture is missing');
    }

    const emptyCandidateIssues = validateDocument(document);

    expect(
      emptyCandidateIssues.some(
        (issue) =>
          issue.nodeId === aggregator.id && issue.fieldKey === 'bindings.groups'
      )
    ).toBe(true);
    expect(
      emptyCandidateIssues.some((issue) =>
        isVariableAggregatorCandidateTypeMismatchIssue(issue)
      )
    ).toBe(false);

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

    const mismatchIssue = validateDocument(document).find((issue) =>
      isVariableAggregatorCandidateTypeMismatchIssue(issue)
    );

    expect(mismatchIssue?.id).toMatch(
      new RegExp(`^${VARIABLE_AGGREGATOR_CANDIDATE_TYPE_MISMATCH_ISSUE_CODE}:`)
    );

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

  test('AC-017/020 preserves invalid intermediate references then atomically renames the exhaustive selector inventory', () => {
    const initialDocument = appendAggregator(
      createDefaultAgentFlowDocument({ flowId: 'rename-aggregator' })
    );
    const aggregator = initialDocument.graph.nodes.find(
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
      {
        key: 'group1',
        title: 'group1',
        valueType: 'string',
        selector: [aggregator.id, 'group1']
      }
    ];

    const inventoryNode: FlowNodeDocument = {
      ...createNodeDocument('code', 'node-selector-inventory'),
      config: {
        protocol_context: {
          kind: 'selector',
          value: [aggregator.id, 'group1', 'protocol-tail']
        }
      },
      bindings: {
        template: {
          kind: 'templated_text',
          value: `{{${aggregator.id}.group1.template-tail}} {{${aggregator.id}.other_group}} {{node-other.group1}}`
        },
        selector: {
          kind: 'selector',
          value: [aggregator.id, 'group1', 'selector-tail']
        },
        selector_list: {
          kind: 'selector_list',
          value: [
            [aggregator.id, 'group1', 'list-tail'],
            [aggregator.id, 'other_group', 'same-node-unrelated-tail'],
            ['node-other', 'group1']
          ]
        },
        groups: {
          kind: 'variable_groups',
          value: [
            {
              key: 'downstream',
              valueType: 'string',
              candidates: [[aggregator.id, 'group1', 'candidate-tail']]
            }
          ]
        },
        messages: {
          kind: 'prompt_messages',
          value: [
            {
              id: 'message-1',
              role: 'user',
              content: {
                kind: 'templated_text',
                value: `Use {{${aggregator.id}.group1.prompt-tail}}.`
              }
            }
          ]
        },
        named: {
          kind: 'named_bindings',
          value: [
            {
              name: 'canonical_selector',
              value: {
                kind: 'selector',
                selector: [aggregator.id, 'group1', 'named-tail']
              }
            },
            {
              name: 'canonical_template',
              value: {
                kind: 'templated_text',
                value: `{{${aggregator.id}.group1.named-template-tail}}`
              }
            },
            {
              name: 'legacy_selector',
              selector: [aggregator.id, 'group1', 'legacy-tail']
            },
            {
              name: 'legacy_template',
              content: {
                kind: 'templated_text',
                value: `{{${aggregator.id}.group1.legacy-template-tail}}`
              }
            }
          ]
        },
        conditions: {
          kind: 'condition_group',
          value: {
            operator: 'and',
            conditions: [
              {
                left: [aggregator.id, 'group1', 'left-tail'],
                comparator: 'equals',
                right: {
                  kind: 'selector',
                  selector: [aggregator.id, 'group1', 'right-tail']
                }
              },
              {
                operator: 'or',
                conditions: [
                  {
                    left: [aggregator.id, 'group1', 'nested-tail'],
                    comparator: 'exists'
                  }
                ]
              }
            ]
          }
        },
        branches: {
          kind: 'if_else_branches',
          value: {
            branches: [
              {
                id: 'if-1',
                kind: 'if',
                title: 'If',
                sourceHandle: 'if-1',
                condition: {
                  operator: 'and',
                  conditions: [
                    {
                      left: [aggregator.id, 'group1', 'branch-tail'],
                      comparator: 'exists'
                    }
                  ]
                }
              }
            ]
          }
        },
        writes: {
          kind: 'state_write',
          value: [
            {
              path: ['conversation', 'target'],
              operator: 'set',
              source: [aggregator.id, 'group1', 'source-tail']
            },
            {
              path: ['conversation', 'selector'],
              operator: 'set',
              value: {
                kind: 'selector',
                selector: [aggregator.id, 'group1', 'write-tail']
              }
            },
            {
              path: ['conversation', 'template'],
              operator: 'set',
              value: {
                kind: 'templated_text',
                value: `{{${aggregator.id}.group1.write-template-tail}}`
              }
            }
          ]
        },
        query: {
          kind: 'data_model_query',
          value: {
            filters: [
              {
                field_code: 'status',
                operator: 'eq',
                value: {
                  kind: 'selector',
                  selector: [aggregator.id, 'group1', 'filter-tail']
                }
              }
            ],
            sorts: [],
            expand_relations: [],
            page: {
              kind: 'selector',
              selector: [aggregator.id, 'group1', 'page-tail']
            },
            page_size: {
              kind: 'selector',
              selector: [aggregator.id, 'group1', 'page-size-tail']
            }
          }
        }
      },
      outputs: [
        {
          key: 'result',
          title: 'result',
          valueType: 'string',
          selector: [aggregator.id, 'group1', 'output-tail']
        }
      ]
    };
    const selectorAnswer = {
      ...createNodeDocument('answer', 'node-answer-selector'),
      bindings: {
        answer_template: {
          kind: 'selector' as const,
          value: [aggregator.id, 'group1', 'answer-tail']
        }
      }
    };
    const templateAnswer = {
      ...createNodeDocument('answer', 'node-answer-template'),
      bindings: {
        answer_template: {
          kind: 'templated_text' as const,
          value: `{{${aggregator.id}.group1.answer-template-tail}}`
        }
      }
    };
    initialDocument.graph.nodes.push(
      inventoryNode,
      selectorAnswer,
      templateAnswer
    );
    initialDocument.graph.edges.push({
      id: 'edge-aggregator-inventory',
      source: aggregator.id,
      target: inventoryNode.id,
      sourceHandle: 'group1',
      targetHandle: null,
      containerId: null,
      points: []
    });

    let savedDocument = initialDocument;
    const adapter = createAgentFlowNodeSchemaAdapter({
      document: initialDocument,
      nodeId: aggregator.id,
      setWorkingDocument(update) {
        savedDocument =
          typeof update === 'function' ? update(savedDocument) : update;
      },
      dispatch() {}
    });

    adapter.setValue('bindings.groups', {
      kind: 'variable_groups',
      value: [{ ...STRING_GROUPS[0], key: 'bad-key' }]
    });

    const invalidAggregator = savedDocument.graph.nodes.find(
      (node) => node.id === aggregator.id
    );
    expect(invalidAggregator?.bindings.groups).toMatchObject({
      value: [{ key: 'bad-key' }]
    });
    expect(invalidAggregator?.outputs[0]?.key).toBe('group1');
    expect(
      savedDocument.graph.nodes.find((node) => node.id === inventoryNode.id)
        ?.bindings
    ).toBe(inventoryNode.bindings);
    const invalidIssueIds = validateDocument(savedDocument)
      .filter(isVariableAggregatorCandidateTypeMismatchIssue)
      .map((issue) => issue.id);
    expect(invalidIssueIds).toEqual([
      `variable_aggregator_group_key_invalid:${aggregator.id}:0`
    ]);
    expect(
      validateDocument(savedDocument)
        .filter(isVariableAggregatorCandidateTypeMismatchIssue)
        .map((issue) => issue.id)
    ).toEqual(invalidIssueIds);

    adapter.setValue('bindings.groups', {
      kind: 'variable_groups',
      value: [{ ...STRING_GROUPS[0], key: 'renamed_group' }]
    });

    const renamedAggregator = savedDocument.graph.nodes.find(
      (node) => node.id === aggregator.id
    );
    const renamedInventory = savedDocument.graph.nodes.find(
      (node) => node.id === inventoryNode.id
    );
    const serializedBindings = JSON.stringify(renamedInventory?.bindings);
    expect(renamedAggregator?.bindings.groups).toMatchObject({
      value: [{ key: 'renamed_group' }]
    });
    expect(renamedAggregator?.outputs).toEqual([
      { key: 'renamed_group', title: 'renamed_group', valueType: 'string' }
    ]);
    expect(serializedBindings).not.toContain(`${aggregator.id}.group1`);
    expect(serializedBindings).not.toContain(`\"${aggregator.id}\",\"group1\"`);
    expect(serializedBindings).toContain('renamed_group');
    expect(serializedBindings).toContain(`{{${aggregator.id}.other_group}}`);
    expect(serializedBindings).toContain(
      `\"${aggregator.id}\",\"other_group\",\"same-node-unrelated-tail\"`
    );
    for (const tail of [
      'template-tail',
      'selector-tail',
      'list-tail',
      'candidate-tail',
      'prompt-tail',
      'named-tail',
      'named-template-tail',
      'legacy-tail',
      'legacy-template-tail',
      'left-tail',
      'right-tail',
      'nested-tail',
      'branch-tail',
      'source-tail',
      'write-tail',
      'write-template-tail',
      'filter-tail',
      'page-tail',
      'page-size-tail',
      'same-node-unrelated-tail'
    ]) {
      expect(serializedBindings).toContain(tail);
    }
    expect(serializedBindings).toContain('node-other');
    expect(renamedInventory?.config.protocol_context).toEqual({
      kind: 'selector',
      value: [aggregator.id, 'renamed_group', 'protocol-tail']
    });
    expect(renamedInventory?.outputs[0]?.selector).toEqual([
      aggregator.id,
      'group1',
      'output-tail'
    ]);
    expect(
      savedDocument.graph.nodes.find((node) => node.id === selectorAnswer.id)
        ?.bindings.answer_template
    ).toMatchObject({
      value: [aggregator.id, 'renamed_group', 'answer-tail']
    });
    expect(
      savedDocument.graph.nodes.find((node) => node.id === templateAnswer.id)
        ?.bindings.answer_template
    ).toMatchObject({
      value: `{{${aggregator.id}.renamed_group.answer-template-tail}}`
    });
    expect(
      savedDocument.graph.edges.find(
        (edge) => edge.id === 'edge-aggregator-inventory'
      )?.sourceHandle
    ).toBe('group1');
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
