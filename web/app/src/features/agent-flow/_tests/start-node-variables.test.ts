import { beforeEach, describe, expect, test } from 'vitest';

import {
  createDefaultAgentFlowDocument,
  createDefaultWorkflowDocument
} from '@1flowbase/flow-schema';

import { buildFlowDebugRunInput } from '../api/runtime';
import { createNodeDocument } from '../lib/document/node-factory';
import {
  isSelectorVisible,
  listLlmContextSelectorOptions,
  listLlmProtocolContextSelectorOptions,
  listVisibleSelectorOptions,
  toCascaderSelectorOptions
} from '../lib/selector-options';
import { getStartInputFields } from '../lib/variables/start-node-variables';
import { appI18n } from '../../../shared/i18n/app-i18n';

describe('start node variables', () => {
  beforeEach(async () => {
    await appI18n.changeLanguage('zh_Hans');
  });

  test('does not expose if else branch decisions as downstream selector values', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const ifElseNode = createNodeDocument('if_else', 'node-if-else-1');

    document.graph.nodes.push({
      ...ifElseNode,
      outputs: [{ key: 'result', title: '条件结果', valueType: 'boolean' }]
    });
    document.graph.edges.push(
      {
        id: 'edge-start-if-else',
        source: 'node-start',
        target: 'node-if-else-1',
        sourceHandle: null,
        targetHandle: null,
        containerId: null,
        points: []
      },
      {
        id: 'edge-if-else-llm',
        source: 'node-if-else-1',
        target: 'node-llm',
        sourceHandle: null,
        targetHandle: null,
        containerId: null,
        points: []
      }
    );

    expect(ifElseNode.outputs).toEqual([]);
    expect(
      listVisibleSelectorOptions(document, 'node-llm').map(
        (option) => option.value
      )
    ).not.toContainEqual(['node-if-else-1', 'result']);
  });

  test('exposes custom input fields and readonly system variables to downstream selectors', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const startNode = document.graph.nodes.find(
      (node) => node.id === 'node-start'
    );

    if (!startNode) {
      throw new Error('expected start node');
    }

    startNode.config.input_fields = [
      {
        key: 'customer_name',
        label: '客户姓名',
        inputType: 'text',
        valueType: 'string',
        required: true
      },
      {
        key: 'attachments',
        label: '附件',
        inputType: 'file_list',
        valueType: 'array[object]',
        required: false
      }
    ];

    expect(
      listVisibleSelectorOptions(document, 'node-llm').map((option) => ({
        value: option.value,
        label: option.displayLabel
      }))
    ).toEqual(
      expect.arrayContaining([
        {
          value: ['node-start', 'customer_name'],
          label: 'Start/customer_name'
        },
        {
          value: ['node-start', 'attachments'],
          label: 'Start/attachments'
        },
        { value: ['node-start', 'query'], label: 'Start/query' },
        { value: ['node-start', 'system'], label: 'Start/system' },
        { value: ['node-start', 'model'], label: 'Start/model' },
        {
          value: ['node-start', 'reasoning_effort'],
          label: 'Start/reasoning_effort'
        },
        { value: ['node-start', 'history'], label: 'Start/history' },
        { value: ['node-start', 'files'], label: 'Start/files' },
        { value: ['node-start', 'tools'], label: 'Start/tools' },
        { value: ['node-start', 'tool_choice'], label: 'Start/tool_choice' }
      ])
    );
  });

  test('carries output value type and schema metadata into selector options', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const codeNode = createNodeDocument('code', 'node-code');

    codeNode.outputs = [
      {
        key: 'chat_history',
        title: 'Chat History',
        valueType: 'array',
        jsonSchema: {
          type: 'array',
          items: {
            type: 'object',
            required: ['role', 'content'],
            properties: {
              role: { type: 'string' },
              content: { type: 'string' }
            }
          }
        }
      }
    ];
    document.graph.nodes.push(codeNode);
    document.graph.edges.push({
      id: 'edge-code-llm',
      source: 'node-code',
      target: 'node-llm',
      sourceHandle: null,
      targetHandle: null,
      containerId: null,
      points: []
    });

    expect(listVisibleSelectorOptions(document, 'node-llm')).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          value: ['node-start', 'history'],
          valueType: 'array',
          jsonSchema: expect.objectContaining({ type: 'array' })
        }),
        expect.objectContaining({
          value: ['node-code', 'result', 'chat_history'],
          valueType: 'array',
          jsonSchema: expect.objectContaining({ type: 'array' })
        })
      ])
    );
  });

  test('filters LLM context options to history-compatible schemas', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const codeNode = createNodeDocument('code', 'node-code');

    codeNode.outputs = [
      {
        key: 'chat_history',
        title: 'Chat History',
        valueType: 'array',
        jsonSchema: {
          type: 'array',
          items: {
            type: 'object',
            required: ['role', 'content'],
            properties: {
              role: { type: 'string' },
              content: { type: 'string' }
            }
          }
        }
      },
      {
        key: 'raw_payload',
        title: 'Raw Payload',
        valueType: 'json',
        jsonSchema: {
          type: 'array',
          items: {
            type: 'object',
            required: ['role', 'content'],
            properties: {
              role: { type: 'string' },
              content: { type: 'string' }
            }
          }
        }
      }
    ];
    document.graph.nodes.push(codeNode);
    document.graph.edges.push({
      id: 'edge-code-llm',
      source: 'node-code',
      target: 'node-llm',
      sourceHandle: null,
      targetHandle: null,
      containerId: null,
      points: []
    });

    expect(
      listLlmContextSelectorOptions(document, 'node-llm').map(
        (option) => option.value
      )
    ).toEqual(
      expect.arrayContaining([
        ['node-start', 'history'],
        ['node-code', 'result', 'chat_history']
      ])
    );
    expect(
      listLlmContextSelectorOptions(document, 'node-llm').map(
        (option) => option.value
      )
    ).not.toContainEqual(['node-code', 'result', 'raw_payload']);
  });

  test('WP-D1D lists only whole protocol context sources without enumerating JSON fields', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const codeNode = createNodeDocument('code', 'node-code');

    codeNode.outputs = [
      {
        key: 'protocol_context',
        title: 'Protocol Context',
        valueType: 'json',
        jsonSchema: {
          type: 'object',
          properties: {
            source_protocol: { type: 'string' },
            headers: { type: 'object' }
          }
        }
      },
      {
        key: 'summary',
        title: 'Summary',
        valueType: 'string'
      }
    ];
    document.graph.nodes.push(codeNode);
    document.graph.edges.push({
      id: 'edge-code-llm',
      source: 'node-code',
      target: 'node-llm',
      sourceHandle: null,
      targetHandle: null,
      containerId: null,
      points: []
    });

    expect(listLlmProtocolContextSelectorOptions(document, 'node-llm')).toEqual([
      expect.objectContaining({
        nodeId: 'sys',
        nodeLabel: 'Start',
        outputLabel: 'sys.protocol_context',
        value: ['sys', 'protocol_context']
      }),
      expect.objectContaining({
        nodeId: 'node-code',
        nodeLabel: 'Code',
        outputLabel: 'protocol_context',
        value: ['node-code', 'result', 'protocol_context']
      })
    ]);
    expect(
      listLlmProtocolContextSelectorOptions(document, 'node-llm').flatMap(
        (option) => option.value
      )
    ).not.toContain('headers');
  });

  test('builds nested cascader paths for code result output selectors', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const codeNode = createNodeDocument('code', 'node-code');

    document.graph.nodes.push(codeNode);
    document.graph.edges.push({
      id: 'edge-code-llm',
      source: 'node-code',
      target: 'node-llm',
      sourceHandle: null,
      targetHandle: null,
      containerId: null,
      points: []
    });

    expect(
      toCascaderSelectorOptions(
        listVisibleSelectorOptions(document, 'node-llm')
      )
    ).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          value: 'node-code',
          children: expect.arrayContaining([
            expect.objectContaining({
              value: 'result',
              children: expect.arrayContaining([
                expect.objectContaining({ value: 'result', label: 'result' })
              ])
            })
          ])
        })
      ])
    );
  });

  test('exposes external model parameters without exposing context window as a runtime variable', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const options = listVisibleSelectorOptions(document, 'node-llm');

    expect(options.map((option) => option.value)).toEqual(
      expect.arrayContaining([
        ['sys', 'model_parameters'],
        ['node-start', 'reasoning_effort'],
        ['node-start', 'max_output_tokens']
      ])
    );
    expect(options.map((option) => option.value)).not.toContainEqual([
      'sys',
      'reasoning_effort'
    ]);
    expect(options.map((option) => option.value)).not.toContainEqual([
      'sys',
      'context_window'
    ]);
    expect(options.map((option) => option.value)).not.toContainEqual([
      'node-start',
      'context_window'
    ]);
  });

  test('exposes system variables to any node without upstream edges', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    document.graph.edges = document.graph.edges.filter(
      (edge) => edge.target !== 'node-answer'
    );

    expect(
      listVisibleSelectorOptions(document, 'node-answer').map((option) => ({
        value: option.value,
        label: option.displayLabel
      }))
    ).toEqual(
      expect.arrayContaining([
        {
          value: ['sys', 'conversation_id'],
          label: 'sys.conversation_id'
        },
        {
          value: ['sys', 'application_id'],
          label: 'sys.application_id'
        },
        {
          value: ['sys', 'workflow_run_id'],
          label: 'sys.workflow_run_id'
        }
      ])
    );
    expect(
      listVisibleSelectorOptions(document, 'node-answer').map(
        (option) => option.value
      )
    ).not.toContainEqual(['sys', 'app_id']);
  });

  test('exposes mounted LLM tool schema arguments to tool branch selectors', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const llmNode = document.graph.nodes.find((node) => node.id === 'node-llm');
    const mountedLlmNode = createNodeDocument('llm', 'node-mounted-llm');
    const mountedAnswerNode = createNodeDocument(
      'answer',
      'node-mounted-answer'
    );

    if (!llmNode) {
      throw new Error('expected llm node');
    }

    llmNode.config.visible_internal_llm_tools_enabled = true;
    llmNode.config.visible_internal_llm_tools = [
      {
        type: 'visible_internal_llm_tool',
        tool_name: 'image_llm',
        connector_id: 'image_llm',
        target_node_id: 'node-mounted-llm',
        input_schema: {
          type: 'object',
          required: ['task'],
          properties: {
            task: {
              type: 'string',
              description: '给多模态模型任务指示提示词'
            },
            retry_count: { type: 'number' }
          }
        }
      }
    ];
    document.graph.nodes.push(mountedLlmNode, mountedAnswerNode);
    document.graph.edges.push(
      {
        id: 'edge-llm-mounted-tool',
        source: 'node-llm',
        target: 'node-mounted-llm',
        sourceHandle: 'visible_internal_llm_tool:image_llm',
        targetHandle: null,
        containerId: null,
        points: []
      },
      {
        id: 'edge-mounted-llm-answer',
        source: 'node-mounted-llm',
        target: 'node-mounted-answer',
        sourceHandle: null,
        targetHandle: null,
        containerId: null,
        points: []
      }
    );

    expect(listVisibleSelectorOptions(document, 'node-mounted-answer')).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          nodeId: 'visible_internal_llm_tool',
          nodeLabel: 'tool',
          outputKey: 'task',
          outputLabel: 'task',
          valueType: 'string',
          jsonSchema: expect.objectContaining({
            type: 'string',
            description: '给多模态模型任务指示提示词'
          }),
          value: ['visible_internal_llm_tool', 'arguments', 'task'],
          displayLabel: 'tool.task'
        }),
        expect.objectContaining({
          outputKey: 'retry_count',
          valueType: 'number',
          value: ['visible_internal_llm_tool', 'arguments', 'retry_count']
        })
      ])
    );
    expect(
      isSelectorVisible(document, 'node-mounted-answer', [
        'visible_internal_llm_tool',
        'arguments',
        'task'
      ])
    ).toBe(true);
  });

  test('does not expose mounted LLM tool arguments outside the tool branch', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const llmNode = document.graph.nodes.find((node) => node.id === 'node-llm');
    const mountedLlmNode = createNodeDocument('llm', 'node-mounted-llm');

    if (!llmNode) {
      throw new Error('expected llm node');
    }

    llmNode.config.visible_internal_llm_tools_enabled = true;
    llmNode.config.visible_internal_llm_tools = [
      {
        type: 'visible_internal_llm_tool',
        tool_name: 'image_llm',
        connector_id: 'image_llm',
        target_node_id: 'node-mounted-llm',
        input_schema: {
          type: 'object',
          properties: {
            task: { type: 'string' }
          }
        }
      }
    ];
    document.graph.nodes.push(mountedLlmNode);
    document.graph.edges.push({
      id: 'edge-llm-mounted-tool',
      source: 'node-llm',
      target: 'node-mounted-llm',
      sourceHandle: 'visible_internal_llm_tool:image_llm',
      targetHandle: null,
      containerId: null,
      points: []
    });

    expect(
      listVisibleSelectorOptions(document, 'node-answer').map(
        (option) => option.value
      )
    ).not.toContainEqual(['visible_internal_llm_tool', 'arguments', 'task']);
    expect(
      isSelectorVisible(document, 'node-answer', [
        'visible_internal_llm_tool',
        'arguments',
        'task'
      ])
    ).toBe(false);
  });

  test('exposes application environment variables to any node', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    document.graph.edges = document.graph.edges.filter(
      (edge) => edge.target !== 'node-answer'
    );

    expect(
      listVisibleSelectorOptions(document, 'node-answer', [
        {
          name: 'ApiBaseUrl',
          value_type: 'string',
          value: 'https://api.example.com',
          description: '当前应用 API 地址'
        }
      ]).map((option) => ({
        value: option.value,
        label: option.displayLabel
      }))
    ).toEqual(
      expect.arrayContaining([
        {
          value: ['env', 'ApiBaseUrl'],
          label: 'env.ApiBaseUrl'
        }
      ])
    );
  });

  test('exposes conversation variables to any node', () => {
    const document = {
      ...createDefaultAgentFlowDocument({ flowId: 'flow-1' }),
      variables: {
        conversation: [
          {
            name: 'ApiBaseUrl',
            valueType: 'string',
            description: '本轮会话 API 地址'
          }
        ]
      }
    } as ReturnType<typeof createDefaultAgentFlowDocument>;

    document.graph.edges = document.graph.edges.filter(
      (edge) => edge.target !== 'node-answer'
    );

    expect(
      listVisibleSelectorOptions(document, 'node-answer').map((option) => ({
        value: option.value,
        label: option.displayLabel
      }))
    ).toEqual(
      expect.arrayContaining([
        {
          value: ['conversation', 'ApiBaseUrl'],
          label: 'conversation.ApiBaseUrl'
        }
      ])
    );
  });

  test('exposes variable assignment node outputs from the selected variable type', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const variableNode = createNodeDocument(
      'variable_assigner',
      'node-env-update'
    );

    variableNode.outputs = [
      {
        key: 'ApiBaseUrl',
        title: 'conversation.ApiBaseUrl',
        valueType: 'string'
      }
    ];
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
      listVisibleSelectorOptions(document, 'node-answer').map((option) => ({
        value: option.value,
        label: option.displayLabel,
        valueType: option.valueType
      }))
    ).toEqual(
      expect.arrayContaining([
        {
          value: ['node-env-update', 'ApiBaseUrl'],
          label: '变量赋值/conversation.ApiBaseUrl',
          valueType: 'string'
        }
      ])
    );
  });

  test('exposes only public LLM runtime output variables to downstream selectors', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const llmNode = document.graph.nodes.find((node) => node.id === 'node-llm');

    if (!llmNode) {
      throw new Error('expected llm node');
    }

    llmNode.outputs = [
      { key: 'text', title: '模型输出', valueType: 'string' },
      { key: 'reasoning_content', title: '思考', valueType: 'string' },
      { key: 'usage', title: 'Token 使用', valueType: 'json' }
    ];

    const selectorLabels = listVisibleSelectorOptions(
      document,
      'node-answer'
    ).map((option) => option.displayLabel);

    const options = listVisibleSelectorOptions(document, 'node-answer');
    const textOutput = options.find(
      (option) => option.value[0] === 'node-llm' && option.outputKey === 'text'
    );

    expect(selectorLabels).toEqual(
      expect.arrayContaining([
        'Start/query',
        'Start/system',
        'Start/model',
        'Start/history',
        'Start/files',
        'LLM/text',
        'LLM/usage'
      ])
    );
    expect(textOutput?.outputLabel).toBe('text');
    expect(textOutput?.value).toEqual(['node-llm', 'text']);

    expect(options).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          outputKey: 'text',
          outputLabel: 'text',
          value: ['node-llm', 'text'],
          displayLabel: 'LLM/text'
        }),
        expect.objectContaining({
          outputKey: 'usage',
          outputLabel: 'usage',
          value: ['node-llm', 'usage'],
          displayLabel: 'LLM/usage'
        })
      ])
    );

    expect(selectorLabels).not.toContain('模型输出');
    expect(selectorLabels).not.toContain('LLM/reasoning_content');
  });

  test('fails fast when a start node carries unexpected outputs', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const startNode = document.graph.nodes.find(
      (node) => node.id === 'node-start'
    );

    if (!startNode) {
      throw new Error('expected start node');
    }

    startNode.outputs = [
      { key: 'query', title: 'unexpected query', valueType: 'string' },
      { key: 'files', title: 'unexpected files', valueType: 'array' }
    ];

    expect(() => listVisibleSelectorOptions(document, 'node-llm')).toThrow(
      'Start node outputs must be empty'
    );
  });

  test('builds flow debug input from start input field value types', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const startNode = document.graph.nodes.find(
      (node) => node.id === 'node-start'
    );

    if (!startNode) {
      throw new Error('expected start node');
    }

    startNode.config.input_fields = [
      {
        key: 'customer_name',
        label: '客户姓名',
        inputType: 'text',
        valueType: 'string',
        required: true
      },
      {
        key: 'age',
        label: '年龄',
        inputType: 'number',
        valueType: 'number',
        required: false
      },
      {
        key: 'files',
        label: '附件',
        inputType: 'file_list',
        valueType: 'array[object]',
        required: false
      }
    ];

    expect(buildFlowDebugRunInput(document)).toEqual({
      input_payload: {
        'node-start': {
          customer_name: 'Start customer_name 调试值',
          age: 1,
          files: [],
          tools: [],
          tool_choice: {},
          system: '',
          model: '',
          reasoning_effort: '',
          max_output_tokens: null,
          history: [],
          query: ''
        }
      }
    });
  });

  test('uses start input field default values for flow debug input', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const startNode = document.graph.nodes.find(
      (node) => node.id === 'node-start'
    );

    if (!startNode) {
      throw new Error('expected start node');
    }

    startNode.config.input_fields = [
      {
        key: 'priority',
        label: '优先级',
        inputType: 'select',
        valueType: 'string',
        required: false,
        options: ['高', '低'],
        defaultValue: '低'
      },
      {
        key: 'confirmed',
        label: '已确认',
        inputType: 'checkbox',
        valueType: 'boolean',
        required: false,
        defaultValue: false
      }
    ];

    expect(buildFlowDebugRunInput(document)).toEqual({
      input_payload: {
        'node-start': {
          priority: '低',
          confirmed: false,
          system: '',
          model: '',
          reasoning_effort: '',
          max_output_tokens: null,
          history: [],
          files: [],
          tools: [],
          tool_choice: {},
          query: ''
        }
      }
    });
  });

  test('adds application environment variables to flow debug input', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    expect(
      buildFlowDebugRunInput(document, undefined, [
        {
          name: 'ApiBaseUrl',
          value_type: 'string',
          value: 'https://api.example.com',
          description: ''
        }
      ])
    ).toEqual(
      expect.objectContaining({
        input_payload: expect.objectContaining({
          env: {
            ApiBaseUrl: 'https://api.example.com'
          }
        })
      })
    );
  });

  test('normalizes rich start input field configuration', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const startNode = document.graph.nodes.find(
      (node) => node.id === 'node-start'
    );

    if (!startNode) {
      throw new Error('expected start node');
    }

    startNode.config.input_fields = [
      {
        key: 'priority',
        label: '优先级',
        inputType: 'select',
        valueType: 'string',
        required: false,
        placeholder: '请选择优先级',
        options: ['高', 1, '低', ''],
        defaultValue: '低',
        hidden: true
      },
      {
        key: 'summary',
        label: '摘要',
        inputType: 'paragraph',
        valueType: 'string',
        required: true,
        maxLength: 120,
        defaultValue: '默认摘要'
      }
    ];

    expect(getStartInputFields(startNode)).toEqual([
      expect.objectContaining({
        key: 'priority',
        label: '优先级',
        inputType: 'select',
        valueType: 'string',
        placeholder: '请选择优先级',
        options: ['高', '低'],
        defaultValue: '低',
        hidden: true
      }),
      expect.objectContaining({
        key: 'summary',
        label: '摘要',
        inputType: 'paragraph',
        valueType: 'string',
        required: true,
        maxLength: 120,
        defaultValue: '默认摘要'
      })
    ]);
  });

  test('AC-1237 preserves valid workflow parameter sources and drops invalid values', () => {
    const document = createDefaultWorkflowDocument({ flowId: 'workflow-1' });
    const startNode = document.graph.nodes.find(
      (node) => node.type === 'workflow_start'
    );

    if (!startNode) {
      throw new Error('expected workflow start node');
    }

    startNode.config.input_fields = [
      {
        key: 'order_id',
        label: '订单 ID',
        inputType: 'text',
        valueType: 'string',
        required: true,
        source: 'path'
      },
      {
        key: 'payload',
        label: '请求体',
        inputType: 'text',
        valueType: 'string',
        required: false,
        source: 'cookie'
      }
    ];

    expect(getStartInputFields(startNode)).toEqual([
      expect.objectContaining({ key: 'order_id', source: 'path' }),
      expect.not.objectContaining({ source: expect.anything() })
    ]);
  });
});
