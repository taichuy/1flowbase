import { COMPACT_SOURCE_HANDLE_ID } from '@1flowbase/flow-schema';
import { describe, expect, test } from 'vitest';

import { validateDocument } from '../../../lib/validate-document';
import {
  createDefaultAgentFlowDocument,
  createNodeDocument
} from '../support';

function createApplicationFlowCompactDocument() {
  const document = createDefaultAgentFlowDocument({ flowId: 'flow-compact' });
  const startNode = document.graph.nodes.find(
    (node) => node.id === 'node-start'
  );

  if (!startNode) {
    throw new Error('expected default Start node');
  }

  startNode.config.compact_dispatch = 'application_flow';
  document.graph.nodes.push(
    createNodeDocument('compact_response', 'node-compact-response', 920, 360)
  );
  document.graph.edges.push({
    id: 'edge-start-compact-response',
    source: 'node-start',
    target: 'node-compact-response',
    sourceHandle: COMPACT_SOURCE_HANDLE_ID,
    targetHandle: null,
    containerId: null,
    points: []
  });

  return document;
}

describe('validateDocument compact response topology', () => {
  test('treats old Start documents with no compact_dispatch field as transparent', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-legacy' });
    const startNode = document.graph.nodes.find(
      (node) => node.id === 'node-start'
    );

    if (!startNode) {
      throw new Error('expected default Start node');
    }

    delete startNode.config.compact_dispatch;

    expect(
      validateDocument(document).some(
        (issue) => issue.fieldKey === 'config.compact_dispatch'
      )
    ).toBe(false);
  });

  test('requires exactly one direct Compact Response edge for application_flow dispatch', () => {
    const document = createApplicationFlowCompactDocument();

    const missingEdgeDocument = createApplicationFlowCompactDocument();
    missingEdgeDocument.graph.edges = missingEdgeDocument.graph.edges.filter(
      (edge) => edge.id !== 'edge-start-compact-response'
    );

    expect(
      validateDocument(missingEdgeDocument).some(
        (issue) =>
          issue.nodeId === 'node-start' &&
          issue.fieldKey === 'config.compact_dispatch' &&
          issue.level === 'error'
      )
    ).toBe(true);

    expect(
      validateDocument(document).some(
        (issue) =>
          issue.nodeId === 'node-start' &&
          issue.fieldKey === 'config.compact_dispatch'
      )
    ).toBe(false);

    document.graph.edges.push({
      id: 'edge-start-compact-response-duplicate',
      source: 'node-start',
      target: 'node-compact-response',
      sourceHandle: COMPACT_SOURCE_HANDLE_ID,
      targetHandle: null,
      containerId: null,
      points: []
    });

    expect(
      validateDocument(document).some(
        (issue) =>
          issue.nodeId === 'node-start' &&
          issue.fieldKey === 'config.compact_dispatch' &&
          issue.level === 'error'
      )
    ).toBe(true);
  });

  test('rejects a dangling compact handle and a Code-node compact target', () => {
    const danglingDocument = createApplicationFlowCompactDocument();
    danglingDocument.graph.edges[2] = {
      ...danglingDocument.graph.edges[2]!,
      target: 'node-deleted-compact-response'
    };

    expect(
      validateDocument(danglingDocument).some(
        (issue) =>
          issue.nodeId === 'node-start' &&
          issue.fieldKey === 'config.compact_dispatch' &&
          issue.message === '压缩响应连线必须指向存在的节点。'
      )
    ).toBe(true);

    const codeDocument = createApplicationFlowCompactDocument();
    codeDocument.graph.nodes.push(
      createNodeDocument('code', 'node-code-compact', 920, 360)
    );
    codeDocument.graph.edges[2] = {
      ...codeDocument.graph.edges[2]!,
      target: 'node-code-compact'
    };

    expect(
      validateDocument(codeDocument).some(
        (issue) =>
          issue.nodeId === 'node-start' &&
          issue.fieldKey === 'config.compact_dispatch' &&
          issue.message === '压缩出口必须直接连接到压缩响应节点。'
      )
    ).toBe(true);
  });

  test('rejects terminal and raw-contract attempts on Compact Response', () => {
    const document = createApplicationFlowCompactDocument();
    const compactResponseNode = document.graph.nodes.find(
      (node) => node.id === 'node-compact-response'
    );

    if (!compactResponseNode) {
      throw new Error('expected Compact Response node');
    }

    compactResponseNode.config = { body: '{"fake":"v2"}' };
    document.graph.edges.push({
      id: 'edge-compact-response-answer',
      source: 'node-compact-response',
      target: 'node-answer',
      sourceHandle: null,
      targetHandle: null,
      containerId: null,
      points: []
    });
    document.graph.edges.push({
      id: 'edge-answer-compact-response',
      source: 'node-answer',
      target: 'node-compact-response',
      sourceHandle: null,
      targetHandle: null,
      containerId: null,
      points: []
    });

    const issues = validateDocument(document);

    expect(issues).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'node-compact-response-compact-response-contract',
          level: 'error'
        }),
        expect.objectContaining({
          id: 'node-compact-response-terminal-outgoing-edge',
          level: 'error'
        }),
        expect.objectContaining({
          id: 'node-answer-terminal-outgoing-edge',
          level: 'error'
        })
      ])
    );
  });
});
