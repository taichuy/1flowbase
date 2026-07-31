import { describe, expect, test } from 'vitest';
import {
  createDefaultAgentFlowDocument,
  createDefaultWorkflowDocument
} from '@1flowbase/flow-schema';

import '../../../workflow/register';

import { classifyDocumentChange } from '../../lib/document/change-kind';
import { createEdgeDocument } from '../../lib/document/edge-factory';
import { createNodeDocument } from '../../lib/document/node-factory';
import { getContainerPathForNode } from '../../lib/document/transforms/container';
import { duplicateNodeSubgraph } from '../../lib/document/transforms/duplicate';
import {
  removeEdge,
  connectNodes,
  connectNodeFromSource,
  insertNodeOnEdge,
  reconnectEdge,
  validateConnection
} from '../../lib/document/transforms/edge';
import {
  moveNodes,
  insertNodeAfter,
  removeNodeSubgraph,
  replaceNodeWithOption,
  updateNodeField
} from '../../lib/document/transforms/node';
import { setViewport } from '../../lib/document/transforms/viewport';
import { createLlmToolSourceHandleId } from '../../lib/llm-node-config';
import { BUILTIN_NODE_PICKER_OPTIONS } from '../../lib/plugin-node-definitions';

function createNestedContainerDocument() {
  const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

  document.graph.nodes.push(
    {
      ...createNodeDocument('iteration', 'node-iteration-1', 640, 240),
      containerId: null
    },
    {
      ...createNodeDocument('answer', 'node-inner-answer-1', 920, 240),
      containerId: 'node-iteration-1'
    }
  );
  document.graph.edges.push(
    createEdgeDocument({
      id: 'edge-iteration-answer',
      source: 'node-iteration-1',
      target: 'node-inner-answer-1',
      containerId: 'node-iteration-1'
    })
  );

  return document;
}

describe('agent flow document transforms', () => {
  test('AC-001/002 rejects connections against workflow entry and terminal topology', () => {
    const document = createDefaultWorkflowDocument({ flowId: 'flow-1' });
    const processingNode = createNodeDocument('code', 'node-code');
    document.graph.nodes.push(processingNode);

    expect(
      validateConnection(document, {
        source: processingNode.id,
        target: 'node-workflow-start'
      })
    ).toBe(false);
    expect(
      validateConnection(document, {
        source: 'node-workflow-end',
        target: processingNode.id
      })
    ).toBe(false);

    const reconnected = reconnectEdge(document, {
      edgeId: 'edge-workflow-start-end',
      connection: {
        source: 'node-workflow-end',
        target: 'node-workflow-start'
      }
    });
    expect(reconnected).toBe(document);

    const inserted = insertNodeOnEdge(document, {
      edgeId: 'edge-workflow-start-end',
      node: createNodeDocument('workflow_end', 'node-workflow-end-2')
    });
    expect(inserted).toBe(document);

    const appendedAfterTerminal = connectNodeFromSource(document, {
      sourceNodeId: 'node-workflow-end',
      node: createNodeDocument('code', 'node-code-after-end')
    });
    expect(appendedAfterTerminal).toBe(document);

    const insertedEntryAfterStart = insertNodeAfter(
      document,
      'node-workflow-start',
      createNodeDocument('workflow_start', 'node-workflow-start-2')
    );
    expect(insertedEntryAfterStart).toBe(document);
  });

  test('inserts a node in the middle of an existing edge', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const inserted = createNodeDocument(
      'template_transform',
      'node-template-transform-1'
    );

    const next = insertNodeOnEdge(document, {
      edgeId: 'edge-llm-answer',
      node: inserted
    });

    expect(next.graph.nodes.map((node) => node.id)).toContain(
      'node-template-transform-1'
    );
    expect(next.graph.edges).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          source: 'node-llm',
          target: 'node-template-transform-1'
        }),
        expect.objectContaining({
          source: 'node-template-transform-1',
          target: 'node-answer'
        })
      ])
    );
  });

  test('reconnects an edge only when source and target stay inside the same container', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    const next = reconnectEdge(document, {
      edgeId: 'edge-start-llm',
      connection: {
        source: 'node-start',
        target: 'node-answer',
        sourceHandle: 'source-right',
        targetHandle: 'target-left'
      }
    });

    expect(next.graph.edges).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'edge-start-llm',
          source: 'node-start',
          target: 'node-answer',
          sourceHandle: 'source-right',
          targetHandle: 'target-left'
        })
      ])
    );

    expect(
      validateConnection(document, {
        source: 'node-start',
        target: 'missing-node'
      })
    ).toBe(false);
  });

  test('classifies viewport changes as layout and field changes as logical', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const moved = moveNodes(document, {
      'node-llm': { x: 520, y: 260 }
    });
    const viewport = setViewport(document, { x: 120, y: 48, zoom: 0.85 });
    const logical = updateNodeField(document, {
      nodeId: 'node-llm',
      fieldKey: 'alias',
      value: 'Dialogue Model'
    });

    expect(classifyDocumentChange(document, moved)).toBe('layout');
    expect(classifyDocumentChange(document, viewport)).toBe('layout');
    expect(classifyDocumentChange(document, logical)).toBe('logical');
  });

  test('updates LLM public outputs when response format explicitly enables structured output', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    document.graph.nodes = document.graph.nodes.map((node) =>
      node.id === 'node-llm'
        ? {
            ...node,
            outputs: [
              {
                key: 'text',
                title: 'Final answer',
                valueType: 'string',
                description: 'User-facing model text'
              }
            ]
          }
        : node
    );

    const structured = updateNodeField(document, {
      nodeId: 'node-llm',
      fieldKey: 'config.response_format',
      value: { mode: 'json_object' }
    });
    const structuredNode = structured.graph.nodes.find(
      (node) => node.id === 'node-llm'
    );

    expect(structuredNode?.outputs).toEqual([
      {
        key: 'text',
        title: 'Final answer',
        valueType: 'string',
        description: 'User-facing model text'
      },
      {
        key: 'usage',
        title: '用量',
        valueType: 'json'
      },
      {
        key: 'structured_output',
        title: '结构化输出',
        valueType: 'json'
      }
    ]);

    const textOnly = updateNodeField(structured, {
      nodeId: 'node-llm',
      fieldKey: 'config.response_format',
      value: { mode: 'text' }
    });
    const textOnlyNode = textOnly.graph.nodes.find(
      (node) => node.id === 'node-llm'
    );

    expect(textOnlyNode?.outputs).toEqual([
      {
        key: 'text',
        title: 'Final answer',
        valueType: 'string',
        description: 'User-facing model text'
      },
      {
        key: 'usage',
        title: '用量',
        valueType: 'json'
      }
    ]);
  });

  test('resolves nested container path from document structure', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    document.graph.nodes.push({
      ...createNodeDocument('iteration', 'node-iteration-1'),
      containerId: null
    });
    document.graph.nodes.push({
      ...createNodeDocument('answer', 'node-inner-answer-1'),
      containerId: 'node-iteration-1'
    });

    expect(getContainerPathForNode(document, 'node-inner-answer-1')).toEqual([
      'node-iteration-1'
    ]);
  });

  test('removes a single edge by id without touching sibling edges', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    const next = removeEdge(document, {
      edgeId: 'edge-llm-answer'
    });

    expect(next.graph.edges).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'edge-llm-answer'
        })
      ])
    );
    expect(next.graph.edges).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'edge-start-llm'
        })
      ])
    );
  });

  test('duplicates a container subtree and rewrites internal ids', () => {
    const document = createNestedContainerDocument();

    const next = duplicateNodeSubgraph(document, {
      nodeId: 'node-iteration-1'
    });

    expect(
      next.graph.nodes.some((node) => node.id === 'node-iteration-1-copy')
    ).toBe(true);
    expect(
      next.graph.nodes.some(
        (node) => node.containerId === 'node-iteration-1-copy'
      )
    ).toBe(true);
    expect(
      next.graph.edges.some(
        (edge) => edge.source.includes('-copy') && edge.target.includes('-copy')
      )
    ).toBe(true);
  });

  test('duplicates prompt message bindings and rewrites selector tokens', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const llmNode = document.graph.nodes.find((node) => node.id === 'node-llm');

    if (!llmNode) {
      throw new Error('expected default LLM node');
    }

    llmNode.bindings.prompt_messages = {
      kind: 'prompt_messages',
      value: [
        {
          id: 'system-1',
          role: 'system',
          content: {
            kind: 'templated_text',
            value: 'Review {{node-llm.output}} before answering.'
          }
        }
      ]
    };

    const next = duplicateNodeSubgraph(document, {
      nodeId: 'node-llm'
    });
    const duplicatedNode = next.graph.nodes.find(
      (node) => node.id === 'node-llm-copy'
    );

    expect(duplicatedNode?.bindings.prompt_messages).toMatchObject({
      kind: 'prompt_messages',
      value: [
        {
          content: {
            value: 'Review {{node-llm-copy.output}} before answering.'
          }
        }
      ]
    });
  });

  test('duplicates Code named binding expressions and rewrites selectors', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const codeNode = {
      ...createNodeDocument('code', 'node-code'),
      bindings: {
        named_bindings: {
          kind: 'named_bindings' as const,
          value: [
            {
              name: 'score',
              valueType: 'number',
              value: {
                kind: 'selector' as const,
                selector: ['node-code', 'result']
              }
            },
            {
              name: 'prompt',
              valueType: 'string',
              value: {
                kind: 'templated_text' as const,
                value: 'Score: {{node-code.result}}'
              }
            },
            {
              name: 'limit',
              valueType: 'number',
              value: { kind: 'constant' as const, value: 10 }
            }
          ]
        }
      }
    };
    document.graph.nodes.push(codeNode);

    const duplicated = duplicateNodeSubgraph(document, {
      nodeId: 'node-code'
    });
    const copied = duplicated.graph.nodes.find(
      (node) => node.id === 'node-code-copy'
    );

    expect(copied?.bindings.named_bindings).toMatchObject({
      kind: 'named_bindings',
      value: [
        {
          value: {
            kind: 'selector',
            selector: ['node-code-copy', 'result']
          }
        },
        {
          value: {
            kind: 'templated_text',
            value: 'Score: {{node-code-copy.result}}'
          }
        },
        {
          value: { kind: 'constant', value: 10 }
        }
      ]
    });
  });

  test('duplicates Data Model query binding and rewrites selector values', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const sourceNode = createNodeDocument('data_model_list', 'node-data-model');
    sourceNode.bindings.query = {
      kind: 'data_model_query',
      value: {
        filters: [
          {
            field_code: 'status',
            operator: 'eq',
            value: { kind: 'selector', selector: ['node-data-model', 'total'] }
          }
        ],
        sorts: [],
        expand_relations: [],
        page: { kind: 'constant', value: 1 },
        page_size: { kind: 'constant', value: 20 }
      }
    };
    document.graph.nodes.push(sourceNode);

    const duplicated = duplicateNodeSubgraph(document, {
      nodeId: 'node-data-model'
    });
    const copied = duplicated.graph.nodes.find(
      (node) => node.id === 'node-data-model-copy'
    );

    expect(copied?.bindings.query).toMatchObject({
      kind: 'data_model_query',
      value: {
        filters: [
          {
            value: {
              kind: 'selector',
              selector: ['node-data-model-copy', 'total']
            }
          }
        ]
      }
    });
  });

  test('duplicates malformed Data Model query binding without crashing', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const sourceNode = createNodeDocument('data_model_list', 'node-data-model');
    sourceNode.bindings.query = {
      kind: 'data_model_query',
      value: {
        filters: [
          {},
          {
            field_code: 'status',
            operator: 'eq',
            value: {
              kind: 'selector',
              selector: ['node-data-model', 'total', 1]
            }
          }
        ],
        sorts: {},
        expand_relations: [false, 'customer'],
        page: {
          kind: 'selector',
          selector: ['node-data-model', 'total', null]
        }
      }
    } as never;
    document.graph.nodes.push(sourceNode);

    const duplicated = duplicateNodeSubgraph(document, {
      nodeId: 'node-data-model'
    });
    const copied = duplicated.graph.nodes.find(
      (node) => node.id === 'node-data-model-copy'
    );

    expect(copied?.bindings.query).toMatchObject({
      kind: 'data_model_query',
      value: {
        filters: [
          {
            field_code: 'status',
            operator: 'eq',
            value: {
              kind: 'selector',
              selector: ['node-data-model-copy', 'total']
            }
          }
        ],
        sorts: [],
        expand_relations: ['customer'],
        page: {
          kind: 'selector',
          selector: ['node-data-model-copy', 'total']
        },
        page_size: { kind: 'constant', value: 20 }
      }
    });
  });

  test('removes a selected node together with connected edges', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    const next = removeNodeSubgraph(document, {
      nodeId: 'node-llm'
    });

    expect(next.graph.nodes).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'node-llm'
        })
      ])
    );
    expect(next.graph.edges).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'edge-start-llm'
        }),
        expect.objectContaining({
          id: 'edge-llm-answer'
        })
      ])
    );
    expect(next.graph.nodes).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'node-start'
        }),
        expect.objectContaining({
          id: 'node-answer'
        })
      ])
    );
  });

  test('replaces a node while keeping its id position and connected edges', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const originalNode = document.graph.nodes.find(
      (node) => node.id === 'node-llm'
    );

    const next = replaceNodeWithOption(document, {
      nodeId: 'node-llm',
      option: BUILTIN_NODE_PICKER_OPTIONS.find(
        (option) => option.type === 'tool_result'
      )!
    });
    const replacedNode = next.graph.nodes.find(
      (node) => node.id === 'node-llm'
    );

    expect(replacedNode).toEqual(
      expect.objectContaining({
        id: 'node-llm',
        type: 'tool_result',
        alias: 'Tool Result',
        position: originalNode?.position,
        containerId: originalNode?.containerId
      })
    );
    expect(next.graph.edges).toEqual(document.graph.edges);
  });

  test('inserts a node after one If / Else branch without rewriting sibling branch edges', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const ifElseNode = createNodeDocument('if_else', 'node-if-else', 640, 240);
    const ifTarget = createNodeDocument('answer', 'node-if-target', 920, 180);
    const elseTarget = createNodeDocument(
      'answer',
      'node-else-target',
      920,
      320
    );
    const insertedNode = createNodeDocument('code', 'node-code', 0, 0);

    document.graph.nodes.push(ifElseNode, ifTarget, elseTarget);
    document.graph.edges.push(
      createEdgeDocument({
        id: 'edge-if-else-if-target',
        source: 'node-if-else',
        target: 'node-if-target',
        sourceHandle: 'if'
      }),
      createEdgeDocument({
        id: 'edge-if-else-else-target',
        source: 'node-if-else',
        target: 'node-else-target',
        sourceHandle: 'else'
      })
    );

    const next = insertNodeAfter(document, 'node-if-else', insertedNode, 'if');

    expect(next.graph.edges).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          source: 'node-if-else',
          target: 'node-code',
          sourceHandle: 'if'
        }),
        expect.objectContaining({
          source: 'node-code',
          target: 'node-if-target',
          sourceHandle: null
        }),
        expect.objectContaining({
          id: 'edge-if-else-else-target',
          source: 'node-if-else',
          target: 'node-else-target',
          sourceHandle: 'else'
        })
      ])
    );
    expect(next.graph.edges).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'edge-if-else-if-target',
          source: 'node-if-else',
          target: 'node-if-target'
        })
      ])
    );
  });

  test('defaults If / Else insert-after connections to the if branch when no handle is provided', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const ifElseNode = createNodeDocument('if_else', 'node-if-else', 640, 240);
    const insertedNode = createNodeDocument('code', 'node-code', 0, 0);

    document.graph.nodes.push(ifElseNode);

    const next = insertNodeAfter(document, 'node-if-else', insertedNode);

    expect(next.graph.edges).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          source: 'node-if-else',
          target: 'node-code',
          sourceHandle: 'if'
        })
      ])
    );
  });

  test('places inserted LLM tool-mounted nodes below the source node mount lane', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const firstToolHandle = createLlmToolSourceHandleId('search_tool');
    const secondToolHandle = createLlmToolSourceHandleId('quote_tool');
    const existingMountedNode = createNodeDocument(
      'llm',
      'node-mounted-llm-existing',
      0,
      0
    );
    const insertedNode = createNodeDocument(
      'tool_result',
      'node-tool-result-mounted',
      0,
      0
    );

    document.graph.nodes = document.graph.nodes.map((node) =>
      node.id === 'node-llm' ? { ...node, position: { x: 400, y: 160 } } : node
    );
    const withExistingMount = insertNodeAfter(
      document,
      'node-llm',
      existingMountedNode,
      firstToolHandle
    );
    const next = insertNodeAfter(
      withExistingMount,
      'node-llm',
      insertedNode,
      secondToolHandle
    );
    const llmNode = next.graph.nodes.find((node) => node.id === 'node-llm');
    const firstMountedNode = next.graph.nodes.find(
      (node) => node.id === 'node-mounted-llm-existing'
    );
    const secondMountedNode = next.graph.nodes.find(
      (node) => node.id === 'node-tool-result-mounted'
    );

    if (!llmNode || !firstMountedNode || !secondMountedNode) {
      throw new Error('expected mounted nodes to exist');
    }

    expect(firstMountedNode.position.x).toBe(llmNode.position.x + 280);
    expect(firstMountedNode.position.y).toBeGreaterThan(
      llmNode.position.y + 96
    );
    expect(secondMountedNode.position.x).toBe(llmNode.position.x + 280);
    expect(
      secondMountedNode.position.y - firstMountedNode.position.y
    ).toBeGreaterThanOrEqual(136);
  });

  test('defaults If / Else drag-created connections to the if branch when no handle is provided', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const ifElseNode = createNodeDocument('if_else', 'node-if-else', 640, 240);
    const insertedNode = createNodeDocument('code', 'node-code', 920, 240);

    document.graph.nodes.push(ifElseNode);

    const next = connectNodeFromSource(document, {
      sourceNodeId: 'node-if-else',
      node: insertedNode
    });

    expect(next.graph.edges).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          source: 'node-if-else',
          target: 'node-code',
          sourceHandle: 'if'
        })
      ])
    );
  });

  test('removes outgoing edges for deleted If / Else branch handles', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const ifElseNode = createNodeDocument('if_else', 'node-if-else', 640, 240);

    ifElseNode.bindings.branches = {
      kind: 'if_else_branches',
      value: {
        branches: [
          {
            id: 'if',
            kind: 'if',
            title: 'If',
            sourceHandle: 'if',
            condition: { operator: 'and', conditions: [] }
          },
          {
            id: 'else-if-1',
            kind: 'else_if',
            title: 'Else If 1',
            sourceHandle: 'else-if-1',
            condition: { operator: 'and', conditions: [] }
          },
          {
            id: 'else',
            kind: 'else',
            title: 'Else',
            sourceHandle: 'else'
          }
        ]
      }
    };
    document.graph.nodes.push(
      ifElseNode,
      createNodeDocument('answer', 'node-if-target'),
      createNodeDocument('answer', 'node-else-if-target'),
      createNodeDocument('answer', 'node-else-target')
    );
    document.graph.edges.push(
      createEdgeDocument({
        id: 'edge-if',
        source: 'node-if-else',
        target: 'node-if-target',
        sourceHandle: 'if'
      }),
      createEdgeDocument({
        id: 'edge-else-if',
        source: 'node-if-else',
        target: 'node-else-if-target',
        sourceHandle: 'else-if-1'
      }),
      createEdgeDocument({
        id: 'edge-else',
        source: 'node-if-else',
        target: 'node-else-target',
        sourceHandle: 'else'
      })
    );

    const next = updateNodeField(document, {
      nodeId: 'node-if-else',
      fieldKey: 'bindings.branches',
      value: {
        kind: 'if_else_branches',
        value: {
          branches: [
            {
              id: 'if',
              kind: 'if',
              title: 'If',
              sourceHandle: 'if',
              condition: { operator: 'and', conditions: [] }
            },
            {
              id: 'else',
              kind: 'else',
              title: 'Else',
              sourceHandle: 'else'
            }
          ]
        }
      }
    });

    expect(next.graph.edges).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: 'edge-if' }),
        expect.objectContaining({ id: 'edge-else' })
      ])
    );
    expect(next.graph.edges).not.toEqual(
      expect.arrayContaining([expect.objectContaining({ id: 'edge-else-if' })])
    );
  });

  test('removes nested container children when deleting a container node', () => {
    const document = createNestedContainerDocument();

    const next = removeNodeSubgraph(document, {
      nodeId: 'node-iteration-1'
    });

    expect(next.graph.nodes).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'node-iteration-1'
        }),
        expect.objectContaining({
          id: 'node-inner-answer-1'
        })
      ])
    );
    expect(next.graph.edges).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'edge-iteration-answer'
        })
      ])
    );
  });
});
