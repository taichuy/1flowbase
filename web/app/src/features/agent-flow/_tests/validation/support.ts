import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';

import { modelProviderOptionsContract } from '../../../../test/model-provider-contract-fixtures';
import { createNodeDocument } from '../../lib/document/node-factory';

export const primaryProvider = modelProviderOptionsContract.providers[0];
export const primaryGroup = primaryProvider.model_groups[0];
export const primaryModel = primaryGroup.model;

export function createCodeDocumentWithOutputs(
  outputs: Array<{
    key: string;
    title: string;
    valueType:
      | 'string'
      | 'number'
      | 'boolean'
      | 'object'
      | 'array'
      | 'json'
      | 'unknown';
    jsonSchema?: Record<string, unknown>;
  }>
) {
  const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

  document.graph.nodes = document.graph.nodes.map((node) =>
    node.id === 'node-llm'
      ? {
          ...createNodeDocument(
            'code',
            'node-code',
            node.position.x,
            node.position.y
          ),
          outputs
        }
      : node
  );
  document.graph.edges = document.graph.edges.map((edge) =>
    edge.source === 'node-llm'
      ? { ...edge, source: 'node-code' }
      : edge.target === 'node-llm'
        ? { ...edge, target: 'node-code' }
        : edge
  );

  return document;
}

export function addSecondLlmNode(
  document: ReturnType<typeof createDefaultAgentFlowDocument>,
  dependsOnFirst: boolean
) {
  const firstLlm = document.graph.nodes.find((node) => node.id === 'node-llm');
  const answerNode = document.graph.nodes.find(
    (node) => node.id === 'node-answer'
  );

  if (!firstLlm || !answerNode) {
    throw new Error('expected default LLM and Answer nodes');
  }

  const secondLlm = {
    ...createNodeDocument(
      'llm',
      'node-llm-2',
      firstLlm.position.x + 240,
      firstLlm.position.y
    ),
    alias: 'LLM 2',
    config: firstLlm.config,
    bindings: dependsOnFirst
      ? {
          prompt_messages: {
            kind: 'prompt_messages' as const,
            value: [
              {
                id: 'user-2',
                role: 'user' as const,
                content: {
                  kind: 'templated_text' as const,
                  value: '{{node-llm.text}}'
                }
              }
            ]
          }
        }
      : firstLlm.bindings,
    outputs: firstLlm.outputs
  };

  document.graph.nodes = [
    ...document.graph.nodes.filter((node) => node.id !== 'node-answer'),
    secondLlm,
    answerNode
  ];
  document.graph.edges = document.graph.edges.filter(
    (edge) => edge.id !== 'edge-llm-answer'
  );

  if (dependsOnFirst) {
    document.graph.edges.push(
      {
        id: 'edge-llm-llm2',
        source: 'node-llm',
        target: 'node-llm-2',
        sourceHandle: null,
        targetHandle: null,
        containerId: null,
        points: []
      },
      {
        id: 'edge-llm2-answer',
        source: 'node-llm-2',
        target: 'node-answer',
        sourceHandle: null,
        targetHandle: null,
        containerId: null,
        points: []
      }
    );
    return;
  }

  document.graph.edges.push(
    {
      id: 'edge-start-llm2',
      source: 'node-start',
      target: 'node-llm-2',
      sourceHandle: null,
      targetHandle: null,
      containerId: null,
      points: []
    },
    {
      id: 'edge-llm-answer',
      source: 'node-llm',
      target: 'node-answer',
      sourceHandle: null,
      targetHandle: null,
      containerId: null,
      points: []
    },
    {
      id: 'edge-llm2-answer',
      source: 'node-llm-2',
      target: 'node-answer',
      sourceHandle: null,
      targetHandle: null,
      containerId: null,
      points: []
    }
  );
}

export {
  createDefaultAgentFlowDocument,
  createNodeDocument,
  modelProviderOptionsContract
};
