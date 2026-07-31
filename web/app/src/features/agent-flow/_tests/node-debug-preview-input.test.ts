import { describe, expect, test } from 'vitest';

import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';

import {
  buildNodeDebugVariableConfirmationPlan,
  buildNodeDebugPreviewPlan,
  extractNodePreviewVariableOutput
} from '../api/runtime';
import { createNodeDocument } from '../lib/document/node-factory';

describe('node debug preview input', () => {
  test('asks for required start input when previewing the start node without cache', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    expect(buildNodeDebugPreviewPlan(document, 'node-start')).toEqual({
      input_payload: {
        'node-start': {
          query: '',
          system: '',
          model: '',
          reasoning_effort: '',
          max_output_tokens: null,
          history: [],
          files: [],
          tools: [],
          tool_choice: {}
        }
      },
      missing_fields: [
        expect.objectContaining({
          nodeId: 'node-start',
          key: 'query',
          title: 'userinput.query',
          valueType: 'string'
        })
      ]
    });
  });

  test('uses cached start input as node preview input when previewing the start node', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    expect(
      buildNodeDebugPreviewPlan(document, 'node-start', {
        'node-start': {
          query: '请总结退款政策',
          files: [{ filename: 'policy.pdf' }]
        }
      })
    ).toEqual({
      input_payload: {
        'node-start': {
          query: '请总结退款政策',
          system: '',
          model: '',
          reasoning_effort: '',
          max_output_tokens: null,
          history: [],
          files: [{ filename: 'policy.pdf' }],
          tools: [],
          tool_choice: {}
        }
      },
      missing_fields: []
    });
  });

  test('builds node preview input from cached referenced variables', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    expect(
      buildNodeDebugPreviewPlan(document, 'node-llm', {
        'node-start': {
          query: '请总结退款政策'
        }
      })
    ).toEqual({
      input_payload: {
        'node-start': {
          history: [],
          query: '请总结退款政策'
        }
      },
      missing_fields: []
    });
  });

  test('AC-003 asks only for variables referenced directly by SQL templates', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const sqlNode = createNodeDocument('sql', 'node-sql');
    sqlNode.bindings = {
      sql: {
        kind: 'templated_text',
        value: 'select * from users where id = {{node-start.query}}'
      }
    };
    document.graph.nodes.push(sqlNode);

    expect(buildNodeDebugPreviewPlan(document, 'node-sql')).toEqual({
      input_payload: {},
      missing_fields: [
        expect.objectContaining({
          nodeId: 'node-start',
          key: 'query',
          title: 'userinput.query',
          valueType: 'string'
        })
      ]
    });
  });

  test('AC-002 ignores variables referenced only by upstream branch nodes', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const ifElseNode = createNodeDocument('if_else', 'node-if-else');
    const sqlNode = createNodeDocument('sql', 'node-sql');

    ifElseNode.bindings.branches = {
      kind: 'if_else_branches',
      value: {
        branches: [
          {
            id: 'if',
            kind: 'if',
            title: 'If',
            sourceHandle: 'if',
            condition: {
              operator: 'and',
              conditions: [
                {
                  kind: 'rule',
                  left: ['node-start', 'model'],
                  comparator: 'equals',
                  right: { kind: 'constant', value: 'claude-opus-4-8' }
                }
              ]
            }
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
    document.graph.nodes.push(ifElseNode, sqlNode);
    document.graph.edges.push(
      {
        id: 'edge-start-if-else',
        source: 'node-start',
        target: 'node-if-else',
        sourceHandle: null,
        targetHandle: null,
        containerId: null,
        points: []
      },
      {
        id: 'edge-if-else-sql',
        source: 'node-if-else',
        target: 'node-sql',
        sourceHandle: 'else',
        targetHandle: null,
        containerId: null,
        points: []
      }
    );

    expect(
      buildNodeDebugPreviewPlan(document, 'node-sql', {
        'node-start': { model: 'claude-opus-4-8' }
      })
    ).toEqual({
      input_payload: {},
      missing_fields: []
    });
  });

  test('builds debug confirmation fields for all referenced variables with cached values', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    expect(
      buildNodeDebugVariableConfirmationPlan(document, 'node-llm', {
        'node-start': {
          query: '请总结退款政策'
        }
      })
    ).toEqual({
      input_payload: {
        'node-start': {
          history: [],
          query: '请总结退款政策'
        }
      },
      fields: [
        expect.objectContaining({
          nodeId: 'node-start',
          key: 'query',
          title: 'userinput.query',
          valueType: 'string',
          value: '请总结退款政策'
        }),
        expect.objectContaining({
          nodeId: 'node-start',
          key: 'history',
          title: 'userinput.history',
          valueType: 'array',
          value: []
        })
      ]
    });
  });

  test('builds debug confirmation fields from config templates and environment variables', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const httpNode = createNodeDocument('http_request', 'node-http');

    httpNode.config = {
      ...httpNode.config,
      url: '{{env.ApiBaseUrl}}/orders?q={{node-start.query}}'
    };
    document.graph.nodes.push(httpNode);

    expect(
      buildNodeDebugVariableConfirmationPlan(document, 'node-http', {
        env: {
          ApiBaseUrl: 'https://api.example.com'
        },
        'node-start': {
          query: '退款'
        }
      })
    ).toEqual({
      input_payload: {
        env: {
          ApiBaseUrl: 'https://api.example.com'
        },
        'node-start': {
          query: '退款'
        }
      },
      fields: [
        expect.objectContaining({
          nodeId: 'env',
          key: 'ApiBaseUrl',
          title: 'env.ApiBaseUrl',
          valueType: 'string',
          value: 'https://api.example.com'
        }),
        expect.objectContaining({
          nodeId: 'node-start',
          key: 'query',
          title: 'userinput.query',
          valueType: 'string',
          value: '退款'
        })
      ]
    });
  });

  test('AC-002 uses the cached direct reference without upstream assignment inputs', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const variableNode = createNodeDocument(
      'variable_assigner',
      'node-env-update'
    );

    variableNode.bindings = {
      operations: {
        kind: 'state_write',
        value: [
          {
            path: ['conversation', 'ApiBaseUrl'],
            operator: 'set',
            value: {
              kind: 'templated_text',
              value: 'https://{{node-start.query}}/v1'
            }
          }
        ]
      }
    };

    document.graph.nodes = document.graph.nodes.map((node) =>
      node.id === 'node-answer'
        ? {
            ...node,
            bindings: {
              answer_template: {
                kind: 'templated_text',
                value: '{{conversation.ApiBaseUrl}}'
              }
            }
          }
        : node
    );
    document.graph.nodes.splice(2, 0, variableNode);
    document.graph.edges = [
      {
        id: 'edge-start-env-update',
        source: 'node-start',
        target: 'node-env-update',
        sourceHandle: null,
        targetHandle: null,
        containerId: null,
        points: []
      },
      {
        id: 'edge-env-update-answer',
        source: 'node-env-update',
        target: 'node-answer',
        sourceHandle: null,
        targetHandle: null,
        containerId: null,
        points: []
      }
    ];

    expect(
      buildNodeDebugPreviewPlan(document, 'node-answer', {
        conversation: {
          ApiBaseUrl: 'https://old.example.com'
        },
        'node-start': {
          query: 'new.example.com'
        }
      })
    ).toEqual({
      input_payload: {
        conversation: {
          ApiBaseUrl: 'https://old.example.com'
        }
      },
      missing_fields: []
    });
  });

  test('AC-002 does not request values used only by upstream assignments', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const variableNode = createNodeDocument(
      'variable_assigner',
      'node-env-update'
    );

    variableNode.bindings = {
      operations: {
        kind: 'state_write',
        value: [
          {
            path: ['conversation', 'ApiBaseUrl'],
            operator: 'set',
            value: {
              kind: 'templated_text',
              value: 'https://{{node-start.query}}/v1'
            }
          }
        ]
      }
    };

    document.graph.nodes = document.graph.nodes.map((node) =>
      node.id === 'node-answer'
        ? {
            ...node,
            bindings: {
              answer_template: {
                kind: 'templated_text',
                value: '{{conversation.ApiBaseUrl}}'
              }
            }
          }
        : node
    );
    document.graph.nodes.splice(2, 0, variableNode);
    document.graph.edges = [
      {
        id: 'edge-start-env-update',
        source: 'node-start',
        target: 'node-env-update',
        sourceHandle: null,
        targetHandle: null,
        containerId: null,
        points: []
      },
      {
        id: 'edge-env-update-answer',
        source: 'node-env-update',
        target: 'node-answer',
        sourceHandle: null,
        targetHandle: null,
        containerId: null,
        points: []
      }
    ];

    expect(
      buildNodeDebugPreviewPlan(document, 'node-answer', {
        conversation: {
          ApiBaseUrl: 'https://old.example.com'
        }
      })
    ).toEqual({
      input_payload: {
        conversation: {
          ApiBaseUrl: 'https://old.example.com'
        }
      },
      missing_fields: []
    });
  });

  test('reports missing node preview variables instead of using placeholders', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    expect(buildNodeDebugPreviewPlan(document, 'node-llm')).toEqual({
      input_payload: {
        'node-start': {
          history: []
        }
      },
      missing_fields: [
        expect.objectContaining({
          nodeId: 'node-start',
          key: 'query',
          title: 'userinput.query',
          valueType: 'string'
        })
      ]
    });
  });

  test('extracts Code named binding expressions and materializes optional start defaults', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    document.graph.nodes.push({
      ...createNodeDocument('code', 'node-code'),
      bindings: {
        named_bindings: {
          kind: 'named_bindings',
          value: [
            {
              name: 'history',
              valueType: 'array',
              value: {
                kind: 'selector',
                selector: ['node-start', 'history']
              }
            },
            {
              name: 'prompt',
              valueType: 'string',
              value: {
                kind: 'templated_text',
                value: '用户问题：{{node-start.query}}'
              }
            },
            {
              name: 'limit',
              valueType: 'number',
              value: { kind: 'constant', value: 10 }
            }
          ]
        }
      }
    });
    document.graph.edges.push({
      id: 'edge-start-code',
      source: 'node-start',
      target: 'node-code',
      sourceHandle: null,
      targetHandle: null,
      containerId: null,
      points: []
    });

    expect(buildNodeDebugPreviewPlan(document, 'node-code')).toEqual({
      input_payload: {
        'node-start': {
          history: []
        }
      },
      missing_fields: [
        expect.objectContaining({
          nodeId: 'node-start',
          key: 'query'
        })
      ]
    });
  });

  test('extracts API-provided node output for downstream previews', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const llmOutput = extractNodePreviewVariableOutput({
      flow_run: {} as never,
      node_run: {
        output_payload: {
          text: '退款政策摘要',
          finish_reason: 'stop'
        }
      } as never,
      checkpoints: [],
      events: []
    });

    expect(llmOutput).toEqual({
      text: '退款政策摘要',
      finish_reason: 'stop'
    });
    expect(
      buildNodeDebugPreviewPlan(document, 'node-answer', {
        'node-llm': llmOutput
      })
    ).toEqual({
      input_payload: {
        'node-llm': {
          text: '退款政策摘要'
        }
      },
      missing_fields: []
    });
  });

  test('builds node preview input from full cached node output using output selector', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const answerNode = document.graph.nodes.find(
      (node) => node.id === 'node-answer'
    );

    if (!answerNode) {
      throw new Error('default document is missing answer node');
    }

    document.graph.nodes.push({
      id: 'node-tool',
      type: 'plugin_node',
      alias: 'Tool',
      description: '',
      containerId: null,
      position: { x: 420, y: 220 },
      configVersion: 1,
      config: {},
      bindings: {},
      outputs: [
        {
          key: 'result',
          title: 'Result',
          valueType: 'string',
          selector: ['message', 'content']
        }
      ]
    });
    answerNode.bindings = {
      answer_template: {
        kind: 'selector',
        value: ['node-tool', 'result']
      }
    };

    expect(
      buildNodeDebugPreviewPlan(document, 'node-answer', {
        'node-tool': {
          message: { content: '退款政策摘要' },
          usage: { total_tokens: 128 },
          raw_response: { id: 'chatcmpl-1' }
        }
      })
    ).toEqual({
      input_payload: {
        'node-tool': {
          result: '退款政策摘要'
        }
      },
      missing_fields: []
    });
  });

  test('keeps code preview input in result error envelope shape', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const codeNode = createNodeDocument('code', 'node-code');
    const answerNode = document.graph.nodes.find(
      (node) => node.id === 'node-answer'
    );

    if (!answerNode) {
      throw new Error('default document is missing answer node');
    }

    codeNode.outputs = [
      {
        key: 'chat_history',
        title: 'chat_history',
        valueType: 'array',
        selector: ['result', 'chat_history']
      }
    ];
    document.graph.nodes.push(codeNode);
    answerNode.bindings = {
      answer_template: {
        kind: 'selector',
        value: ['node-code', 'result', 'chat_history']
      }
    };

    expect(
      buildNodeDebugPreviewPlan(document, 'node-answer', {
        'node-code': {
          result: {
            chat_history: [{ role: 'user', content: 'hello' }]
          },
          error: null
        }
      })
    ).toEqual({
      input_payload: {
        'node-code': {
          result: {
            chat_history: [{ role: 'user', content: 'hello' }]
          },
          error: null
        }
      },
      missing_fields: []
    });
  });

  test('extracts selector dependencies from active Data Model query binding', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    document.graph.nodes.push({
      id: 'node-data-model',
      type: 'data_model_list',
      alias: 'Orders',
      description: '',
      containerId: null,
      position: { x: 720, y: 220 },
      configVersion: 1,
      config: { data_model_code: 'orders' },
      bindings: {
        query: {
          kind: 'data_model_query',
          value: {
            filters: [
              {
                field_code: 'status',
                operator: 'eq',
                value: { kind: 'selector', selector: ['node-start', 'query'] }
              }
            ],
            sorts: [],
            expand_relations: [],
            page: { kind: 'constant', value: 1 },
            page_size: { kind: 'constant', value: 20 }
          }
        },
        record_id: { kind: 'selector', value: ['node-answer', 'answer'] }
      },
      outputs: [
        { key: 'records', title: 'Records', valueType: 'array' },
        { key: 'total', title: 'Total', valueType: 'number' }
      ]
    });

    expect(buildNodeDebugPreviewPlan(document, 'node-data-model')).toEqual({
      input_payload: {},
      missing_fields: [
        expect.objectContaining({
          nodeId: 'node-start',
          key: 'query',
          valueType: 'string'
        })
      ]
    });
  });

  test('normalizes malformed Data Model query binding before preview extraction', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    document.graph.nodes.push({
      id: 'node-data-model',
      type: 'data_model_list',
      alias: 'Orders',
      description: '',
      containerId: null,
      position: { x: 720, y: 220 },
      configVersion: 1,
      config: { data_model_code: 'orders' },
      bindings: {
        query: {
          kind: 'data_model_query',
          value: {
            filters: [
              {},
              {
                field_code: 'status',
                operator: 'eq',
                value: {
                  kind: 'selector',
                  selector: ['node-start', 'query', 1]
                }
              }
            ],
            sorts: 'bad',
            expand_relations: [1, 'customer'],
            page: { kind: 'selector', selector: ['node-start', 'query', null] }
          }
        } as never
      },
      outputs: [
        { key: 'records', title: 'Records', valueType: 'array' },
        { key: 'total', title: 'Total', valueType: 'number' }
      ]
    });

    expect(buildNodeDebugPreviewPlan(document, 'node-data-model')).toEqual({
      input_payload: {},
      missing_fields: [
        expect.objectContaining({
          nodeId: 'node-start',
          key: 'query'
        })
      ]
    });
  });

  test('ignores residual Data Model query binding on create node type', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    document.graph.nodes.push({
      id: 'node-data-model',
      type: 'data_model_create',
      alias: 'Orders',
      description: '',
      containerId: null,
      position: { x: 720, y: 220 },
      configVersion: 1,
      config: { data_model_code: 'orders' },
      bindings: {
        query: {
          kind: 'data_model_query',
          value: {
            filters: [
              {
                field_code: 'status',
                operator: 'eq',
                value: { kind: 'selector', selector: ['node-answer', 'answer'] }
              }
            ],
            sorts: [],
            expand_relations: [],
            page: { kind: 'constant', value: 1 },
            page_size: { kind: 'constant', value: 20 }
          }
        },
        payload: {
          kind: 'named_bindings',
          value: [{ name: 'title', selector: ['node-start', 'query'] }]
        }
      },
      outputs: [{ key: 'record', title: 'Record', valueType: 'json' }]
    });

    expect(buildNodeDebugPreviewPlan(document, 'node-data-model')).toEqual({
      input_payload: {},
      missing_fields: [
        expect.objectContaining({
          nodeId: 'node-start',
          key: 'query'
        })
      ]
    });
  });
});
