import { describe, expect, it } from 'vitest';

import {
  DEFAULT_LLM_PROTOCOL_CONTEXT_REFERENCE,
  DEFAULT_LLM_NODE_OUTPUTS,
  DEFAULT_WORKFLOW_START_NODE_CONFIG,
  FLOW_SCHEMA_VERSION,
  NODE_CONTRIBUTION_SCHEMA_VERSION,
  createDefaultAgentFlowDocument,
  createDefaultWorkflowDocument,
  getLlmNodeOutputs,
  validatePublicOutputKey,
  type FlowLlmVisibleInternalToolDocument,
  type NodeRuntimeUiContract
} from '../index';

describe('schema v2 constants', () => {
  it('exports the flow and node contribution schema versions', () => {
    expect(FLOW_SCHEMA_VERSION).toBe('1flowbase.flow/v2');
    expect(NODE_CONTRIBUTION_SCHEMA_VERSION).toBe(
      '1flowbase.node-contribution/v2'
    );
  });
});

describe('public output key validation', () => {
  it('rejects internal public output keys', () => {
    expect(validatePublicOutputKey('__raw')).toEqual({
      ok: false,
      reason: 'reserved_public_output_key'
    });
  });

  it.each([
    'text',
    'structured_output',
    'answer',
    'record_id',
    'usage',
    'error'
  ])('accepts public output key %s', (key) => {
    expect(validatePublicOutputKey(key)).toEqual({ ok: true });
  });
});

describe('LLM authoring outputs', () => {
  it('defaults LLM public outputs to text and usage', () => {
    expect(DEFAULT_LLM_NODE_OUTPUTS).toEqual([
      { key: 'text', title: '模型输出', valueType: 'string' },
      { key: 'usage', title: '用量', valueType: 'json' }
    ]);
  });

  it('seeds the default LLM node without runtime trace outputs', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const llmNode = document.graph.nodes.find((node) => node.id === 'node-llm');

    expect(llmNode?.outputs).toEqual([
      { key: 'text', title: '模型输出', valueType: 'string' },
      { key: 'usage', title: '用量', valueType: 'json' }
    ]);
  });

  it('WP-D1D seeds new public Gateway LLM protocol context as one system reference', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const llmNode = document.graph.nodes.find((node) => node.id === 'node-llm');

    expect(DEFAULT_LLM_PROTOCOL_CONTEXT_REFERENCE).toEqual({
      kind: 'selector',
      value: ['sys', 'protocol_context']
    });
    expect(llmNode?.config.protocol_context).toEqual(
      DEFAULT_LLM_PROTOCOL_CONTEXT_REFERENCE
    );
    expect(llmNode?.config).not.toHaveProperty('protocol_context_enabled');
  });

  it('supports per-tool mounted LLM mode in the authoring contract', () => {
    const tool = {
      type: 'visible_internal_llm_tool',
      tool_name: 'compare_panels',
      connector_id: 'compare_panels',
      target_node_id: 'node-panel-seed',
      tool_mode: 'fusion',
      internal_llm_node_policy: 'allowed',
      external_tool_policy: 'forbidden',
      input_schema: { type: 'object' },
      preconditions: [{ kind: 'media_content_available' }]
    } satisfies FlowLlmVisibleInternalToolDocument;

    expect(tool.tool_mode).toBe('fusion');
  });

  it('adds structured_output only for explicitly structured response formats', () => {
    expect(getLlmNodeOutputs()).toEqual([
      { key: 'text', title: '模型输出', valueType: 'string' },
      { key: 'usage', title: '用量', valueType: 'json' }
    ]);
    expect(getLlmNodeOutputs({ response_format: { mode: 'text' } })).toEqual([
      { key: 'text', title: '模型输出', valueType: 'string' },
      { key: 'usage', title: '用量', valueType: 'json' }
    ]);
    expect(
      getLlmNodeOutputs({ response_format: { mode: 'json_schema' } })
    ).toEqual([
      { key: 'text', title: '模型输出', valueType: 'string' },
      { key: 'usage', title: '用量', valueType: 'json' },
      { key: 'structured_output', title: '结构化输出', valueType: 'json' }
    ]);
    expect(
      getLlmNodeOutputs({ response_format: { mode: 'json_object' } })
    ).toEqual([
      { key: 'text', title: '模型输出', valueType: 'string' },
      { key: 'usage', title: '用量', valueType: 'json' },
      { key: 'structured_output', title: '结构化输出', valueType: 'json' }
    ]);
  });
});

describe('workflow authoring defaults', () => {
  it('seeds workflow_start -> workflow_end without AgentFlow start and answer nodes', () => {
    const document = createDefaultWorkflowDocument({ flowId: 'flow-1' });

    expect(document.schemaVersion).toBe(FLOW_SCHEMA_VERSION);
    expect(document.graph.nodes.map((node) => node.type)).toEqual([
      'workflow_start',
      'workflow_end'
    ]);
    expect(
      document.graph.edges.map((edge) => [edge.source, edge.target])
    ).toEqual([['node-workflow-start', 'node-workflow-end']]);
    expect(document.graph.nodes[0]?.config).toEqual(
      DEFAULT_WORKFLOW_START_NODE_CONFIG
    );
    expect(document.graph.nodes[1]?.outputs).toEqual([]);
  });
});

describe('node runtime UI contract type', () => {
  it('supports the required first-class contract sections', () => {
    const contract = {
      meta: {
        type: 'llm',
        title: 'LLM'
      },
      defaults: {
        alias: 'LLM',
        description: '',
        configVersion: 1,
        config: {},
        bindings: {},
        outputs: [{ key: 'text', title: '模型输出', valueType: 'string' }]
      },
      ports: {
        inputs: [{ key: 'input', title: 'Input' }],
        outputs: [{ key: 'output', title: 'Output' }]
      },
      card: {
        title: 'LLM',
        description: 'Generate text'
      },
      panel: {
        sections: []
      },
      runtime: {
        outputs: [{ key: 'text', title: '模型输出', valueType: 'string' }]
      },
      policies: {
        sideEffect: 'external_read'
      }
    } satisfies NodeRuntimeUiContract;

    expect(contract.defaults.outputs[0]?.key).toBe('text');
  });
});
