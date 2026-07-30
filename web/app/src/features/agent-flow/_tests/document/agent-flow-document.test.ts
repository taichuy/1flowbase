import { describe, expect, test } from 'vitest';

import {
  DEFAULT_ANSWER_NODE_OUTPUTS,
  DEFAULT_LLM_NODE_OUTPUTS,
  DEFAULT_START_NODE_CONFIG,
  FLOW_SCHEMA_VERSION,
  classifyDocumentChange,
  createDefaultAgentFlowDocument,
  type FlowAuthoringDocument
} from '@1flowbase/flow-schema';

import {
  buildDefaultAgentFlowDocument,
  createNodeDocument,
  createNextNodeId
} from '../../lib/default-agent-flow-document';
import { getBuiltinNodeRuntimeContract } from '../../lib/node-definitions/contracts';

describe('agent flow document helpers', () => {
  test('seeds the default start -> llm -> answer graph', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    expect(document.schemaVersion).toBe(FLOW_SCHEMA_VERSION);
    expect(document.graph.nodes.map((node) => node.type)).toEqual([
      'start',
      'llm',
      'answer'
    ]);
    expect(
      document.graph.edges.map((edge) => [edge.source, edge.target])
    ).toEqual([
      ['node-start', 'node-llm'],
      ['node-llm', 'node-answer']
    ]);
    expect(
      document.graph.nodes.find((node) => node.id === 'node-answer')?.bindings
        .answer_template
    ).toEqual({
      kind: 'templated_text',
      value: '{{node-llm.text}}'
    });
    expect(document.graph.nodes.every((node) => node.description === '')).toBe(
      true
    );
  });

  test('seeds default LLM prompt messages with Start query user context', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const llmNode = document.graph.nodes.find((node) => node.id === 'node-llm');

    expect(llmNode?.bindings.prompt_messages).toEqual({
      kind: 'prompt_messages',
      value: [
        {
          id: 'system-1',
          role: 'system',
          content: { kind: 'templated_text', value: '' }
        },
        {
          id: 'user-1',
          role: 'user',
          content: {
            kind: 'templated_text',
            value: '{{node-start.query}}'
          }
        }
      ]
    });
    expect(llmNode?.bindings).not.toHaveProperty('user_prompt');
  });

  test('treats viewport-only edits as layout changes', () => {
    const before = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const viewportOnly = {
      ...before,
      editor: {
        ...before.editor,
        viewport: { x: 120, y: 48, zoom: 0.85 }
      }
    };
    const logicalChange: FlowAuthoringDocument = {
      ...before,
      graph: {
        ...before.graph,
        nodes: before.graph.nodes.map((node) =>
          node.id === 'node-llm'
            ? {
                ...node,
                bindings: {
                  ...node.bindings,
                  system_prompt: {
                    kind: 'templated_text' as const,
                    value: 'You are a support agent.'
                  }
                }
              }
            : node
        )
      }
    };

    expect(classifyDocumentChange(before, viewportOnly)).toBe('layout');
    expect(classifyDocumentChange(before, logicalChange)).toBe('logical');
  });

  test('keeps the local document helper facade aligned with flow-schema defaults', () => {
    const document = buildDefaultAgentFlowDocument('flow-1');

    expect(document.graph.nodes.map((node) => node.id)).toEqual([
      'node-start',
      'node-llm',
      'node-answer'
    ]);
    expect(createNextNodeId(document, 'llm')).toBe('node-llm-1');
  });

  test('models the start node as configurable input fields with Dify-compatible system variables', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const startNode = document.graph.nodes.find(
      (node) => node.type === 'start'
    );

    expect(startNode?.config.input_fields).toEqual([]);
    expect(startNode?.outputs).toEqual([]);
  });

  test('keeps default document node defaults aligned with node factory and runtime contracts', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const startNode = document.graph.nodes.find(
      (node) => node.id === 'node-start'
    );
    const llmNode = document.graph.nodes.find((node) => node.id === 'node-llm');
    const answerNode = document.graph.nodes.find(
      (node) => node.id === 'node-answer'
    );

    expect(startNode?.config).toEqual(DEFAULT_START_NODE_CONFIG);
    expect(startNode?.config).toEqual(
      createNodeDocument('start', 'node-start').config
    );
    expect(startNode?.config).toEqual(
      getBuiltinNodeRuntimeContract('start')?.defaults.config
    );
    expect(startNode?.outputs).toEqual([]);

    expect(llmNode?.outputs).toEqual(DEFAULT_LLM_NODE_OUTPUTS);
    expect(llmNode?.outputs).toEqual(
      createNodeDocument('llm', 'node-llm').outputs
    );
    expect(llmNode?.outputs).toEqual(
      getBuiltinNodeRuntimeContract('llm')?.defaults.outputs
    );
    expect(llmNode?.config.external_model_parameter_policy).toEqual({
      follow_external_max_output_tokens: false
    });
    expect(
      createNodeDocument('llm', 'node-llm').config
        .external_model_parameter_policy
    ).toEqual({
      follow_external_max_output_tokens: false
    });
    expect(
      getBuiltinNodeRuntimeContract('llm')?.defaults.config
        .external_model_parameter_policy
    ).toEqual({
      follow_external_max_output_tokens: false
    });

    expect(answerNode?.outputs).toEqual(DEFAULT_ANSWER_NODE_OUTPUTS);
    expect(answerNode?.outputs).toEqual(
      createNodeDocument('answer', 'node-answer').outputs
    );
    expect(answerNode?.outputs).toEqual(
      getBuiltinNodeRuntimeContract('answer')?.defaults.outputs
    );
  });
});
