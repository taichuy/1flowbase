import {
  createDefaultAgentFlowDocument,
  type FlowNodeDocument
} from '@1flowbase/flow-schema';
import type {
  SchemaBlock,
  SchemaFieldBlock
} from '../../../shared/schema-ui/contracts/canvas-node-schema';
import { describe, expect, test, vi } from 'vitest';

import { agentFlowRendererRegistry } from '../schema/agent-flow-renderer-registry';
import { buildCommonConfigBlocks } from '../schema/node-schema-fragments';
import { createAgentFlowNodeSchemaAdapter } from '../schema/node-schema-adapter';
import { resolveAgentFlowNodeSchema } from '../schema/node-schema-registry';
import { createNodeDocument } from '../lib/document/node-factory';
import {
  builtinNodeRuntimeContractTypes,
  getBuiltinNodeRuntimeContract
} from '../lib/node-definitions/contracts';
import {
  BUILTIN_NODE_PICKER_OPTIONS,
  WORKFLOW_BUILTIN_NODE_PICKER_OPTIONS,
  type NodePickerOption
} from '../lib/plugin-node-definitions';

function getNode(
  document: ReturnType<typeof createDefaultAgentFlowDocument>,
  nodeId: string
) {
  const node = document.graph.nodes.find(
    (candidate) => candidate.id === nodeId
  );

  if (!node) {
    throw new Error(`Missing node ${nodeId}`);
  }

  return node;
}

function findFieldBlock(
  blocks: SchemaBlock[],
  path: string
): SchemaFieldBlock | null {
  for (const block of blocks) {
    if (block.kind === 'field' && block.path === path) {
      return block;
    }

    if ('blocks' in block) {
      const found = findFieldBlock(block.blocks, path);

      if (found) {
        return found;
      }
    }
  }

  return null;
}

const DEFAULT_CODE_NODE_SOURCE = `function main({arg1, arg2}) {
   const param=arg1 + arg2
    console.log(param)

    return {
        result: param
    }
}`;

describe('agent-flow node schema registry', () => {
  test('keeps identity fields in the header and config fields in the config tab', () => {
    const schema = resolveAgentFlowNodeSchema('llm');

    expect(schema.nodeType).toBe('llm');
    expect(schema.detail.header.blocks).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ kind: 'field', path: 'alias' }),
        expect.objectContaining({ kind: 'field', path: 'description' })
      ])
    );
    expect(schema.detail.tabs.config.blocks.length).toBeGreaterThan(0);
    expect(
      JSON.stringify(schema.detail.tabs.config.blocks).includes(
        '"path":"alias"'
      )
    ).toBe(false);
    expect(
      JSON.stringify(schema.detail.tabs.config.blocks).includes(
        '"path":"description"'
      )
    ).toBe(false);
  });

  test('exposes a real renderer registry for later schema-driven consumers', () => {
    expect(agentFlowRendererRegistry.fields.text).toBeTypeOf('function');
    expect(agentFlowRendererRegistry.fields.llm_model).toBeTypeOf('function');
    expect(agentFlowRendererRegistry.fields.llm_response_format).toBeTypeOf(
      'function'
    );
    expect(agentFlowRendererRegistry.fields.llm_tool_registrations).toBeTypeOf(
      'function'
    );
    expect(agentFlowRendererRegistry.fields.code_source).toBeTypeOf('function');
    expect(
      agentFlowRendererRegistry.fields.output_contract_definition
    ).toBeTypeOf('function');
    expect(agentFlowRendererRegistry.fields.start_input_fields).toBeTypeOf(
      'function'
    );
    expect(agentFlowRendererRegistry.fields.start_model_list).toBeTypeOf(
      'function'
    );
    expect(agentFlowRendererRegistry.fields.variable_assignment).toBeTypeOf(
      'function'
    );
    expect(agentFlowRendererRegistry.fields.data_model_query).toBeTypeOf(
      'function'
    );
    expect(agentFlowRendererRegistry.dynamicForms.llm_parameters).toBeTypeOf(
      'function'
    );
    expect(agentFlowRendererRegistry.views.summary).toBeTypeOf('function');
    expect(agentFlowRendererRegistry.views.relations).toBeTypeOf('function');
  });

  test('uses a narrow variable assignment editor for Variable Assigner', () => {
    const configBlocks = buildCommonConfigBlocks('variable_assigner');
    const operationsField = findFieldBlock(configBlocks, 'bindings.operations');
    const contract = getBuiltinNodeRuntimeContract('variable_assigner');

    expect(contract?.meta.title).toBe('变量赋值');
    expect(contract?.defaults.alias).toBe('变量赋值');
    expect(contract?.defaults.outputs).toEqual([]);
    expect(operationsField).toEqual(
      expect.objectContaining({
        path: 'bindings.operations',
        renderer: 'variable_assignment'
      })
    );
  });

  test('syncs Variable Assigner outputs from selected conversation variables', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const variableNode = createNodeDocument(
      'variable_assigner',
      'node-env-update'
    );
    const variableDocument = {
      ...document,
      variables: {
        conversation: [
          {
            name: 'ApiBaseUrl',
            valueType: 'string',
            description: ''
          }
        ]
      },
      graph: {
        ...document.graph,
        nodes: [variableNode]
      }
    };
    const setWorkingDocument = vi.fn();
    const adapter = createAgentFlowNodeSchemaAdapter({
      document: variableDocument,
      nodeId: 'node-env-update',
      conversationVariables: variableDocument.variables.conversation,
      setWorkingDocument,
      dispatch: vi.fn()
    });

    adapter.setValue('bindings.operations', {
      kind: 'state_write',
      value: [
        {
          path: ['conversation', 'ApiBaseUrl'],
          operator: 'set',
          value: { kind: 'templated_text', value: '{{node-start.query}}' }
        }
      ]
    });

    const update = setWorkingDocument.mock.calls[0]?.[0] as
      | typeof variableDocument
      | ((currentDocument: typeof variableDocument) => typeof variableDocument);
    const nextDocument =
      typeof update === 'function' ? update(variableDocument) : update;
    const nextNode = getNode(nextDocument, 'node-env-update');

    expect(nextNode.outputs).toEqual([
      {
        key: 'ApiBaseUrl',
        title: 'conversation.ApiBaseUrl',
        valueType: 'string'
      }
    ]);
  });

  test('renders start input fields before the relations section', () => {
    const schema = resolveAgentFlowNodeSchema('start');

    expect(schema.detail.tabs.config.blocks).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          kind: 'section',
          title: '输入字段',
          blocks: [
            expect.objectContaining({
              kind: 'field',
              path: 'config.input_fields',
              renderer: 'start_input_fields'
            })
          ]
        }),
        expect.objectContaining({
          kind: 'section',
          title: '模型列表',
          blocks: [
            expect.objectContaining({
              kind: 'field',
              path: 'config.model_list',
              renderer: 'start_model_list'
            })
          ]
        }),
        expect.objectContaining({
          kind: 'view',
          renderer: 'relations',
          title: '下一步'
        })
      ])
    );
  });

  test('renders answer content with the templated text editor', () => {
    const schema = resolveAgentFlowNodeSchema('answer');

    expect(schema.detail.tabs.config.blocks).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          kind: 'section',
          title: 'Inputs',
          blocks: [
            expect.objectContaining({
              kind: 'field',
              path: 'bindings.answer_template',
              renderer: 'templated_text'
            })
          ]
        })
      ])
    );
  });

  test('registers start and answer nodes for the built-in node picker', () => {
    expect(BUILTIN_NODE_PICKER_OPTIONS).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          kind: 'builtin',
          type: 'start',
          label: 'Start'
        }),
        expect.objectContaining({
          kind: 'builtin',
          type: 'answer',
          label: 'Answer'
        }),
        expect.objectContaining({
          kind: 'builtin',
          type: 'tool_result',
          label: 'Tool Result'
        })
      ])
    );
    expect(BUILTIN_NODE_PICKER_OPTIONS).toEqual(
      expect.not.arrayContaining([
        expect.objectContaining({
          kind: 'builtin',
          type: 'tool'
        })
      ])
    );
  });

  test('renders generated output variables as a readonly shared config section', () => {
    const schema = resolveAgentFlowNodeSchema('llm');

    expect(schema.detail.tabs.config.blocks).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          kind: 'view',
          renderer: 'output_contract',
          title: '输出变量',
          key: 'llm-generated-outputs'
        })
      ])
    );
  });

  test('exposes LLM tool registration authoring fields without execution role UI', () => {
    const schema = resolveAgentFlowNodeSchema('llm');
    const contract = getBuiltinNodeRuntimeContract('llm');
    const executionRoleField = findFieldBlock(
      schema.detail.tabs.config.blocks,
      'config.execution_role'
    );
    const internalLlmField = findFieldBlock(
      schema.detail.tabs.config.blocks,
      'config.internal_llm_node_policy'
    );
    const mountToolsField = findFieldBlock(
      schema.detail.tabs.config.blocks,
      'config.visible_internal_llm_tools_enabled'
    );

    expect(contract?.defaults.config).toEqual(
      expect.objectContaining({
        visible_internal_llm_tools_enabled: false,
        visible_internal_llm_tools: []
      })
    );
    expect(executionRoleField).toBeNull();
    expect(internalLlmField).toBeNull();
    expect(mountToolsField).toEqual(
      expect.objectContaining({
        renderer: 'llm_tool_registrations'
      })
    );
  });

  test('exposes workflow start inputs and sync timeout through schema contract', () => {
    const schema = resolveAgentFlowNodeSchema('workflow_start');

    expect(schema.nodeType).toBe('workflow_start');
    expect(schema.detail.tabs.config.blocks).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          kind: 'section',
          title: 'Input Parameters',
          blocks: [
            expect.objectContaining({
              kind: 'field',
              path: 'config.input_fields',
              renderer: 'start_input_fields'
            })
          ]
        }),
        expect.objectContaining({
          kind: 'section',
          title: 'Sync Response',
          blocks: [
            expect.objectContaining({
              kind: 'field',
              path: 'config.sync_timeout_ms',
              renderer: 'number'
            })
          ]
        })
      ])
    );
  });

  test('exposes workflow end return fields as editable output contract', () => {
    const schema = resolveAgentFlowNodeSchema('workflow_end');

    expect(schema.nodeType).toBe('workflow_end');
    expect(schema.detail.tabs.config.blocks).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          kind: 'section',
          title: 'Return Fields',
          blocks: [
            expect.objectContaining({
              kind: 'field',
              path: 'config.output_contract',
              renderer: 'output_contract_definition'
            })
          ]
        })
      ])
    );
  });

  test('keeps workflow picker options separate from AgentFlow picker options', () => {
    expect(BUILTIN_NODE_PICKER_OPTIONS.map((option) => option.type)).toEqual(
      expect.arrayContaining(['start', 'answer'])
    );
    expect(
      BUILTIN_NODE_PICKER_OPTIONS.map((option) => option.type)
    ).not.toEqual(expect.arrayContaining(['workflow_start', 'workflow_end']));
    expect(
      WORKFLOW_BUILTIN_NODE_PICKER_OPTIONS.map((option) => option.type)
    ).toEqual(['workflow_start', 'workflow_end']);
  });

  test('keeps Code on the main input, JavaScript source, and editable output flow', () => {
    const schema = resolveAgentFlowNodeSchema('code');
    const serializedConfigBlocks = JSON.stringify(
      schema.detail.tabs.config.blocks
    );
    const inputField = findFieldBlock(
      schema.detail.tabs.config.blocks,
      'bindings.named_bindings'
    );
    const sourceField = findFieldBlock(
      schema.detail.tabs.config.blocks,
      'config.source'
    );
    const outputField = findFieldBlock(
      schema.detail.tabs.config.blocks,
      'config.output_contract'
    );

    expect(inputField).toEqual(
      expect.objectContaining({ renderer: 'templated_named_bindings' })
    );
    expect(sourceField).toEqual(
      expect.objectContaining({ renderer: 'code_source' })
    );
    expect(outputField).toEqual(
      expect.objectContaining({ renderer: 'output_contract_definition' })
    );
    expect(
      serializedConfigBlocks.indexOf('"path":"bindings.named_bindings"')
    ).toBeLessThan(serializedConfigBlocks.indexOf('"path":"config.source"'));
    expect(
      serializedConfigBlocks.indexOf('"path":"config.source"')
    ).toBeLessThan(
      serializedConfigBlocks.indexOf('"path":"config.output_contract"')
    );
    expect(serializedConfigBlocks).toContain('"path":"config.output_contract"');
    expect(serializedConfigBlocks).toContain(
      '"renderer":"output_contract_definition"'
    );
    expect(serializedConfigBlocks).not.toContain('"path":"config.language"');
    expect(serializedConfigBlocks).not.toContain('"title":"Advanced"');
    expect(serializedConfigBlocks).not.toContain(
      '"renderer":"output_contract"'
    );
  });

  test('creates Code nodes with default args, source, and string result output', () => {
    const codeNode = createNodeDocument('code', 'node-code-defaults');

    expect(codeNode.bindings).toEqual({
      named_bindings: {
        kind: 'named_bindings',
        value: [
          {
            name: 'arg1',
            valueType: 'string',
            value: { kind: 'constant', value: '' }
          },
          {
            name: 'arg2',
            valueType: 'string',
            value: { kind: 'constant', value: '' }
          }
        ]
      }
    });
    expect(codeNode.config).toEqual({
      language: 'javascript',
      source: DEFAULT_CODE_NODE_SOURCE
    });
    expect(codeNode.outputs).toEqual([
      {
        key: 'result',
        title: 'result',
        valueType: 'string',
        selector: ['result', 'result']
      }
    ]);
  });

  test('models If / Else branches as first-class source handles', () => {
    const contract = getBuiltinNodeRuntimeContract('if_else');
    const ifElseNode = createNodeDocument('if_else', 'node-if-else');
    const schema = resolveAgentFlowNodeSchema('if_else');
    const loopSchema = resolveAgentFlowNodeSchema('loop');
    const branchField = findFieldBlock(
      schema.detail.tabs.config.blocks,
      'bindings.branches'
    );
    const loopConditionField = findFieldBlock(
      loopSchema.detail.tabs.config.blocks,
      'bindings.entry_condition'
    );

    expect(contract?.defaults.outputs).toEqual([]);
    expect(contract?.ports.outputs).toEqual([
      { key: 'if', title: 'If' },
      { key: 'else', title: 'Else' }
    ]);
    expect(contract?.defaults.bindings.branches).toEqual({
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
    });
    expect(ifElseNode.bindings).toEqual(contract?.defaults.bindings);
    expect(branchField).toEqual(
      expect.objectContaining({
        renderer: 'if_else_branches',
        path: 'bindings.branches'
      })
    );
    expect(loopConditionField).toEqual(
      expect.objectContaining({
        renderer: 'condition_group',
        path: 'bindings.entry_condition'
      })
    );
  });

  test('keeps the start node on input fields instead of the shared output editor', () => {
    const schema = resolveAgentFlowNodeSchema('start');
    const serializedConfigBlocks = JSON.stringify(
      schema.detail.tabs.config.blocks
    );

    expect(serializedConfigBlocks).toContain('"path":"config.input_fields"');
    expect(serializedConfigBlocks).toContain('"path":"config.model_list"');
    expect(serializedConfigBlocks).not.toContain(
      '"path":"config.output_contract"'
    );
  });

  test('registers built-in Data Model CRUD nodes for picker and schema-driven config', () => {
    const pickerTypes = BUILTIN_NODE_PICKER_OPTIONS.map(
      (option) => option.type
    );

    expect(pickerTypes).toEqual(
      expect.arrayContaining([
        'data_model_list',
        'data_model_get',
        'data_model_create',
        'data_model_update',
        'data_model_delete'
      ])
    );
    expect(pickerTypes).not.toContain('data_model');

    for (const nodeType of [
      'data_model_list',
      'data_model_get',
      'data_model_create',
      'data_model_update',
      'data_model_delete'
    ] as const) {
      const schema = resolveAgentFlowNodeSchema(nodeType);
      const serializedConfigBlocks = JSON.stringify(
        schema.detail.tabs.config.blocks
      );

      expect(schema.nodeType).toBe(nodeType);
      expect(serializedConfigBlocks).toContain(
        '"path":"config.data_model_code"'
      );
      expect(serializedConfigBlocks).toContain('"renderer":"data_model"');
      expect(serializedConfigBlocks).not.toContain('"path":"config.action"');
    }
  });

  test('keeps Data Model CRUD node fields fixed by node type', () => {
    const listSchema = resolveAgentFlowNodeSchema('data_model_list');
    const getSchema = resolveAgentFlowNodeSchema('data_model_get');
    const createSchema = resolveAgentFlowNodeSchema('data_model_create');
    const updateSchema = resolveAgentFlowNodeSchema('data_model_update');
    const deleteSchema = resolveAgentFlowNodeSchema('data_model_delete');

    expect(
      findFieldBlock(listSchema.detail.tabs.config.blocks, 'bindings.query')
    ).toEqual(expect.objectContaining({ renderer: 'data_model_query' }));
    expect(
      findFieldBlock(getSchema.detail.tabs.config.blocks, 'bindings.record_id')
    ).toEqual(expect.objectContaining({ renderer: 'selector' }));
    expect(
      findFieldBlock(createSchema.detail.tabs.config.blocks, 'bindings.payload')
    ).toEqual(expect.objectContaining({ renderer: 'named_bindings' }));
    expect(
      findFieldBlock(
        createSchema.detail.tabs.config.blocks,
        'config.side_effect_policy'
      )
    ).toEqual(expect.objectContaining({ renderer: 'static_select' }));
    expect(
      findFieldBlock(
        updateSchema.detail.tabs.config.blocks,
        'bindings.record_id'
      )
    ).toEqual(expect.objectContaining({ renderer: 'selector' }));
    expect(
      findFieldBlock(updateSchema.detail.tabs.config.blocks, 'bindings.payload')
    ).toEqual(expect.objectContaining({ renderer: 'named_bindings' }));
    expect(
      findFieldBlock(
        updateSchema.detail.tabs.config.blocks,
        'config.side_effect_policy'
      )
    ).toEqual(expect.objectContaining({ renderer: 'static_select' }));
    expect(
      findFieldBlock(
        deleteSchema.detail.tabs.config.blocks,
        'bindings.record_id'
      )
    ).toEqual(expect.objectContaining({ renderer: 'selector' }));
    expect(
      findFieldBlock(
        deleteSchema.detail.tabs.config.blocks,
        'config.side_effect_policy'
      )
    ).toEqual(expect.objectContaining({ renderer: 'static_select' }));
    expect(
      findFieldBlock(
        listSchema.detail.tabs.config.blocks,
        'config.side_effect_policy'
      )
    ).toBeNull();
    expect(
      findFieldBlock(
        getSchema.detail.tabs.config.blocks,
        'config.side_effect_policy'
      )
    ).toBeNull();
    expect(
      findFieldBlock(getSchema.detail.tabs.config.blocks, 'bindings.query')
    ).toBeNull();
    expect(
      findFieldBlock(updateSchema.detail.tabs.config.blocks, 'bindings.query')
    ).toBeNull();
    expect(
      findFieldBlock(deleteSchema.detail.tabs.config.blocks, 'bindings.query')
    ).toBeNull();
  });

  test('exposes Data Model list query params without action-scoped visibility', () => {
    const schema = resolveAgentFlowNodeSchema('data_model_list');
    const queryField = findFieldBlock(
      schema.detail.tabs.config.blocks,
      'bindings.query'
    );

    expect(queryField).toEqual(
      expect.objectContaining({
        renderer: 'data_model_query'
      })
    );
    expect(queryField).not.toHaveProperty('visibleWhen');
  });

  test('creates Data Model CRUD nodes with fixed outputs', () => {
    const listNode = createNodeDocument(
      'data_model_list',
      'node-data-model-list'
    );
    const getNode = createNodeDocument('data_model_get', 'node-data-model-get');
    const createNode = createNodeDocument(
      'data_model_create',
      'node-data-model-create'
    );
    const updateNode = createNodeDocument(
      'data_model_update',
      'node-data-model-update'
    );
    const deleteNode = createNodeDocument(
      'data_model_delete',
      'node-data-model-delete'
    );

    expect(listNode.config).toEqual({ data_model_code: '' });
    expect(getNode.config).toEqual({ data_model_code: '' });
    expect(createNode.config).toEqual({
      data_model_code: '',
      side_effect_policy: 'disabled'
    });
    expect(updateNode.config).toEqual({
      data_model_code: '',
      side_effect_policy: 'disabled'
    });
    expect(deleteNode.config).toEqual({
      data_model_code: '',
      side_effect_policy: 'disabled'
    });
    expect(listNode.outputs).toEqual([
      { key: 'records', title: 'Records', valueType: 'array' },
      { key: 'total', title: 'Total', valueType: 'number' }
    ]);
    expect(getNode.outputs).toEqual([
      { key: 'record', title: 'Record', valueType: 'json' }
    ]);
    expect(createNode.outputs).toEqual([
      { key: 'record', title: 'Record', valueType: 'json' }
    ]);
    expect(updateNode.outputs).toEqual([
      { key: 'record', title: 'Record', valueType: 'json' }
    ]);
    expect(deleteNode.outputs).toEqual([
      { key: 'deleted_id', title: 'Deleted ID', valueType: 'string' },
      { key: 'affected_count', title: 'Affected Count', valueType: 'number' }
    ]);
  });

  test('registers builtin node runtime contracts for runtime defaults', () => {
    const expectedTypes = [
      'start',
      'workflow_start',
      'llm',
      'answer',
      'workflow_end',
      'template_transform',
      'knowledge_retrieval',
      'question_classifier',
      'if_else',
      'code',
      'http_request',
      'tool',
      'tool_result',
      'plugin_node',
      'human_input',
      'data_model_list',
      'data_model_get',
      'data_model_create',
      'data_model_update',
      'data_model_delete',
      'variable_assigner',
      'parameter_extractor',
      'iteration',
      'loop'
    ] as const;

    expect([...builtinNodeRuntimeContractTypes]).toEqual(
      expect.arrayContaining([...expectedTypes])
    );
    expect(
      builtinNodeRuntimeContractTypes.every((nodeType) =>
        expectedTypes.includes(nodeType)
      )
    ).toBe(true);

    for (const nodeType of expectedTypes) {
      expect(getBuiltinNodeRuntimeContract(nodeType)).not.toBeNull();
      expect(getBuiltinNodeRuntimeContract(nodeType)!.meta.type).toBe(nodeType);
      expect(
        getBuiltinNodeRuntimeContract(nodeType)!.defaults.configVersion
      ).toBe(1);
      expect(
        getBuiltinNodeRuntimeContract(nodeType)!.defaults.alias
      ).toBeTypeOf('string');
      expect(
        Array.isArray(getBuiltinNodeRuntimeContract(nodeType)!.defaults.outputs)
      ).toBe(true);
    }
  });

  test('builds builtin picker options from runtime contract metadata and ports', () => {
    const llmContract = getBuiltinNodeRuntimeContract('llm');
    const llmOption = BUILTIN_NODE_PICKER_OPTIONS.find(
      (option) => option.type === 'llm'
    );

    expect(llmOption).toEqual(
      expect.objectContaining({
        label: llmContract?.meta.title,
        description: llmContract?.defaults.description,
        category: llmContract?.card.category,
        outputKeys: llmContract?.ports.outputs.map((output) => output.key)
      })
    );
  });

  test('Start defaults to no outputs and derive variables from input config', () => {
    const contract = getBuiltinNodeRuntimeContract('start');

    expect(contract).not.toBeNull();
    expect(contract?.defaults.outputs).toEqual([]);
    expect(contract?.defaults.config).toEqual({
      input_fields: [],
      model_list: []
    });
    expect(contract?.defaults.bindings).toEqual({});
  });

  test('returns isolated builtin contract defaults for mutable nested values', () => {
    const firstContract = getBuiltinNodeRuntimeContract('llm');

    if (!firstContract) {
      throw new Error('expected LLM contract');
    }

    firstContract.defaults.config.model_provider = {
      provider_code: 'mutated-provider',
      model_id: 'mutated-model'
    };
    firstContract.defaults.bindings.prompt_messages = {
      kind: 'prompt_messages',
      value: []
    };
    firstContract.defaults.outputs[0] = {
      key: 'mutated',
      title: 'Mutated',
      valueType: 'string'
    };

    const nextContract = getBuiltinNodeRuntimeContract('llm');

    expect(nextContract?.defaults.config.model_provider).toEqual({
      provider_code: '',
      model_id: ''
    });
    expect(nextContract?.defaults.bindings.prompt_messages).toEqual({
      kind: 'prompt_messages',
      value: [
        {
          id: 'system-1',
          role: 'system',
          content: { kind: 'templated_text', value: '' }
        }
      ]
    });
    expect(nextContract?.defaults.outputs).toEqual([
      { key: 'text', title: '模型输出', valueType: 'string' },
      { key: 'usage', title: '用量', valueType: 'json' }
    ]);
  });

  test('Answer defaults to answer output only', () => {
    const contract = getBuiltinNodeRuntimeContract('answer');

    expect(contract).not.toBeNull();
    expect(contract?.defaults.outputs).toEqual([
      { key: 'answer', title: '对话输出', valueType: 'string' }
    ]);
  });

  test('Template Transform defaults to text output only', () => {
    const contract = getBuiltinNodeRuntimeContract('template_transform');

    expect(contract).not.toBeNull();
    expect(contract?.defaults.outputs).toEqual([
      { key: 'text', title: '转换结果', valueType: 'string' }
    ]);
  });

  test('HTTP Request defaults to executable request contract outputs', () => {
    const contract = getBuiltinNodeRuntimeContract('http_request');
    const node = createNodeDocument('http_request', 'node-http-request');

    expect(contract).not.toBeNull();
    expect(contract?.defaults.alias).toBe('HTTP');
    expect(contract?.defaults.config).toEqual({
      method: 'GET',
      url: '',
      body_type: 'none',
      verify_ssl: true,
      store_response_as_file: false,
      timeout_ms: 30000,
      max_response_bytes: 6291456
    });
    expect(contract?.defaults.bindings).toEqual({
      params: { kind: 'named_bindings', value: [] },
      headers: { kind: 'named_bindings', value: [] },
      body: { kind: 'templated_text', value: '' },
      urlencoded: { kind: 'named_bindings', value: [] },
      form_data: { kind: 'named_bindings', value: [] }
    });
    expect(contract?.defaults.outputs).toEqual([
      { key: 'body', title: 'HTTP 响应正文', valueType: 'string' },
      { key: 'status_code', title: '响应状态码', valueType: 'number' },
      { key: 'headers', title: '响应头列表 JSON', valueType: 'object' },
      { key: 'files', title: 'HTTP 响应文件', valueType: 'Array[File]' }
    ]);
    expect(node.config).toEqual(contract?.defaults.config);
    expect(node.bindings).toEqual(contract?.defaults.bindings);
    expect(node.outputs).toEqual(contract?.defaults.outputs);
  });

  test('HTTP Request config blocks expose request fields around output variables', () => {
    const configBlocks = buildCommonConfigBlocks('http_request');
    const serializedConfigBlocks = JSON.stringify(configBlocks);

    expect(serializedConfigBlocks).toContain('"path":"config.url"');
    expect(serializedConfigBlocks).toContain('"path":"bindings.params"');
    expect(serializedConfigBlocks).toContain('"path":"bindings.headers"');
    expect(serializedConfigBlocks).toContain('"path":"config.body_type"');
    expect(serializedConfigBlocks).toContain('"path":"config.verify_ssl"');
    expect(serializedConfigBlocks).toContain(
      '"path":"config.store_response_as_file"'
    );
    expect(serializedConfigBlocks).toContain('"path":"config.timeout_ms"');
    expect(serializedConfigBlocks).toContain(
      '"path":"config.max_response_bytes"'
    );
    expect(serializedConfigBlocks).toContain('"max":10485760');
    expect(serializedConfigBlocks).toContain(
      '"renderer":"http_request_endpoint"'
    );
    expect(serializedConfigBlocks).toContain(
      '"renderer":"http_request_key_values"'
    );
    expect(serializedConfigBlocks).toContain('"renderer":"http_request_body"');
    expect(serializedConfigBlocks).toContain(
      '"renderer":"http_request_curl_import"'
    );
    expect(serializedConfigBlocks.indexOf('"path":"config.url"')).toBeLessThan(
      serializedConfigBlocks.indexOf('"renderer":"output_contract"')
    );
    expect(
      serializedConfigBlocks.indexOf('"renderer":"output_contract"')
    ).toBeLessThan(
      serializedConfigBlocks.indexOf('"path":"config.timeout_ms"')
    );
    expect(
      serializedConfigBlocks.indexOf('"path":"config.timeout_ms"')
    ).toBeLessThan(
      serializedConfigBlocks.indexOf('"path":"config.max_response_bytes"')
    );
    expect(
      serializedConfigBlocks.indexOf('"path":"config.max_response_bytes"')
    ).toBeLessThan(
      serializedConfigBlocks.indexOf('"path":"config.curl_import"')
    );
    expect(
      serializedConfigBlocks.indexOf('"path":"config.curl_import"')
    ).toBeLessThan(
      serializedConfigBlocks.indexOf('"path":"config.verify_ssl"')
    );
    expect(
      serializedConfigBlocks.indexOf('"path":"config.verify_ssl"')
    ).toBeLessThan(
      serializedConfigBlocks.indexOf('"path":"config.store_response_as_file"')
    );
    expect(
      serializedConfigBlocks.indexOf('"path":"config.store_response_as_file"')
    ).toBeLessThan(serializedConfigBlocks.indexOf('"renderer":"policy_group"'));
  });

  test('Data Model Delete defaults to deleted_id and affected_count outputs', () => {
    const contract = getBuiltinNodeRuntimeContract('data_model_delete');

    expect(contract).not.toBeNull();
    expect(contract?.defaults.outputs).toEqual([
      { key: 'deleted_id', title: 'Deleted ID', valueType: 'string' },
      { key: 'affected_count', title: 'Affected Count', valueType: 'number' }
    ]);
  });

  test('creates contracted built-in node documents from runtime contract defaults', () => {
    const contract = getBuiltinNodeRuntimeContract('human_input');

    if (!contract) {
      throw new Error('expected Human Input contract');
    }

    const node = createNodeDocument(
      'human_input',
      'node-human-input',
      120,
      240
    );

    expect(node).toEqual(
      expect.objectContaining({
        id: 'node-human-input',
        type: 'human_input',
        alias: contract.defaults.alias,
        description: contract.defaults.description,
        position: { x: 120, y: 240 },
        configVersion: contract.defaults.configVersion,
        config: contract.defaults.config,
        bindings: contract.defaults.bindings,
        outputs: contract.defaults.outputs
      })
    );
  });

  test('creates every built-in node document from runtime contract defaults', () => {
    for (const nodeType of builtinNodeRuntimeContractTypes) {
      if (nodeType === 'plugin_node') {
        continue;
      }

      const contract = getBuiltinNodeRuntimeContract(nodeType);

      if (!contract) {
        throw new Error(`expected ${nodeType} contract`);
      }

      const node = createNodeDocument(nodeType, `node-${nodeType}`);

      expect(node).toEqual(
        expect.objectContaining({
          type: contract.meta.type,
          alias: contract.defaults.alias,
          description: contract.defaults.description,
          configVersion: contract.defaults.configVersion,
          config: contract.defaults.config,
          bindings: contract.defaults.bindings,
          outputs: contract.defaults.outputs
        })
      );
    }
  });

  test('rejects unavailable plugin contribution options in the node factory', () => {
    const disabledContributionOption = {
      kind: 'plugin_contribution',
      label: 'Disabled Exporter',
      disabled: true,
      disabledReason: '缺少依赖插件',
      contribution: {
        installation_id: 'installation-1',
        provider_code: 'sql_pack',
        plugin_id: 'sql_pack@0.1.0',
        plugin_version: '0.1.0',
        contribution_code: 'disabled_exporter',
        node_shell: 'action',
        plugin_unique_identifier: 'sql_pack',
        package_id: 'sql_pack@0.1.0',
        contribution_checksum: 'sha256:disabled-exporter',
        compiled_contribution_hash: 'sha256:compiled-disabled-exporter',
        category: 'export',
        title: 'Disabled Exporter',
        description: 'Disabled plugin node',
        dependency_status: 'missing_plugin',
        schema_version: '1flowbase.node-contribution/v2',
        output_schema_snapshot: {
          outputs: [{ key: 'result', title: 'Result', valueType: 'json' }]
        },
        experimental: false,
        icon: 'database',
        schema_ui: {},
        output_schema: {
          outputs: [{ key: 'result', title: 'Result', valueType: 'json' }]
        },
        side_effect_policy: 'external_read',
        infra_contracts: [],
        required_auth: [],
        visibility: 'public',
        dependency_installation_kind: 'model_provider',
        dependency_plugin_version_range: '^0.1.0'
      }
    } satisfies NodePickerOption;

    expect(() =>
      createNodeDocument(disabledContributionOption, 'node-disabled')
    ).toThrow('Plugin contribution is unavailable');
  });

  test('does not share nested contract defaults across created node documents', () => {
    const firstNode = createNodeDocument('llm', 'node-llm-a');

    firstNode.config.model_provider = {
      provider_code: 'mutated-provider',
      model_id: 'mutated-model'
    };
    firstNode.bindings.prompt_messages = {
      kind: 'prompt_messages',
      value: []
    };
    firstNode.outputs[0] = {
      key: 'mutated',
      title: 'Mutated',
      valueType: 'string'
    };

    const nextNode = createNodeDocument('llm', 'node-llm-b');
    const contract = getBuiltinNodeRuntimeContract('llm');

    expect(nextNode.config).toEqual(contract?.defaults.config);
    expect(nextNode.bindings).toEqual(contract?.defaults.bindings);
    expect(nextNode.outputs).toEqual(contract?.defaults.outputs);
  });

  test('reads relative node values and preserves output contract writes on the document', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const setWorkingDocument = vi.fn();
    const dispatch = vi.fn();
    const adapter = createAgentFlowNodeSchemaAdapter({
      document,
      nodeId: 'node-llm',
      setWorkingDocument,
      dispatch
    });

    expect(adapter.getValue('alias')).toBe('LLM');
    expect(adapter.getValue('config.model_provider')).toEqual({
      provider_code: '',
      model_id: ''
    });

    const nextOutputs: FlowNodeDocument['outputs'] = [
      { key: 'answer', title: '最终回复', valueType: 'string' }
    ];

    adapter.setValue('config.output_contract', nextOutputs);

    expect(setWorkingDocument).toHaveBeenCalledTimes(1);

    const update = setWorkingDocument.mock.calls[0]?.[0] as
      | ReturnType<typeof createDefaultAgentFlowDocument>
      | ((
          currentDocument: ReturnType<typeof createDefaultAgentFlowDocument>
        ) => ReturnType<typeof createDefaultAgentFlowDocument>);
    const nextDocument =
      typeof update === 'function' ? update(document) : update;
    const nextNode = getNode(nextDocument, 'node-llm');

    expect(nextNode.outputs).toEqual(nextOutputs);
    expect(nextNode.config).not.toHaveProperty('output_contract');
    expect(nextNode.alias).toBe('LLM');
    expect(dispatch).not.toHaveBeenCalled();
  });

  test('writes LLM tool registrations into node config without graph edges', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const mountedLlm = createNodeDocument('llm', 'node-mounted-llm');
    const documentWithMountedLlm = {
      ...document,
      graph: {
        ...document.graph,
        nodes: [...document.graph.nodes, mountedLlm]
      }
    };
    const setWorkingDocument = vi.fn();
    const dispatch = vi.fn();
    const adapter = createAgentFlowNodeSchemaAdapter({
      document: documentWithMountedLlm,
      nodeId: 'node-llm',
      setWorkingDocument,
      dispatch
    });
    const nextTools = [
      {
        type: 'visible_internal_llm_tool',
        tool_name: 'inspect_visible_context',
        connector_id: 'inspect_visible_context',
        target_node_id: 'node-mounted-llm',
        description: 'Inspect visible context',
        input_schema: { type: 'object' }
      }
    ];

    adapter.setValue('config.visible_internal_llm_tools_enabled', true);
    adapter.setValue('config.visible_internal_llm_tools', nextTools);

    const nextDocument = setWorkingDocument.mock.calls.reduce(
      (currentDocument, [update]) =>
        typeof update === 'function' ? update(currentDocument) : update,
      documentWithMountedLlm
    );

    expect(getNode(nextDocument, 'node-llm').config).toEqual(
      expect.objectContaining({
        visible_internal_llm_tools_enabled: true,
        visible_internal_llm_tools: nextTools
      })
    );
    expect(nextDocument.graph.edges).toEqual(document.graph.edges);
    expect(dispatch).not.toHaveBeenCalled();
  });

  test('splits HTTP Request URL query parameters into Params bindings', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const httpNode = createNodeDocument('http_request', 'node-http-request');
    const httpDocument = {
      ...document,
      graph: {
        ...document.graph,
        nodes: [...document.graph.nodes, httpNode]
      }
    };
    const setWorkingDocument = vi.fn();
    const adapter = createAgentFlowNodeSchemaAdapter({
      document: httpDocument,
      nodeId: 'node-http-request',
      setWorkingDocument,
      dispatch: vi.fn()
    });

    adapter.setValue(
      'config.url',
      'https://api.example.com/orders?page=1&q=refund'
    );

    expect(setWorkingDocument).toHaveBeenCalledTimes(1);

    const update = setWorkingDocument.mock.calls[0]?.[0] as
      | typeof httpDocument
      | ((currentDocument: typeof httpDocument) => typeof httpDocument);
    const nextDocument =
      typeof update === 'function' ? update(httpDocument) : update;
    const nextNode = getNode(nextDocument, 'node-http-request');

    expect(nextNode.config.url).toBe('https://api.example.com/orders');
    expect(nextNode.bindings.params).toEqual({
      kind: 'named_bindings',
      value: [
        {
          name: 'page',
          value: { kind: 'templated_text', value: '1' }
        },
        {
          name: 'q',
          value: { kind: 'templated_text', value: 'refund' }
        }
      ]
    });
  });

  test('fills missing fixed HTTP Request output variables for legacy nodes', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const httpNode = createNodeDocument('http_request', 'node-http-request');
    const legacyHttpNode: FlowNodeDocument = {
      ...httpNode,
      outputs: [{ key: 'body', title: '响应内容', valueType: 'json' }]
    };
    const legacyDocument = {
      ...document,
      graph: {
        ...document.graph,
        nodes: [legacyHttpNode]
      }
    };
    const adapter = createAgentFlowNodeSchemaAdapter({
      document: legacyDocument,
      nodeId: 'node-http-request',
      setWorkingDocument: vi.fn(),
      dispatch: vi.fn()
    });

    expect(adapter.getValue('config.output_contract')).toEqual([
      { key: 'body', title: '响应内容', valueType: 'json' },
      { key: 'status_code', title: '响应状态码', valueType: 'number' },
      { key: 'headers', title: '响应头列表 JSON', valueType: 'object' },
      { key: 'files', title: 'HTTP 响应文件', valueType: 'Array[File]' }
    ]);
  });
});
