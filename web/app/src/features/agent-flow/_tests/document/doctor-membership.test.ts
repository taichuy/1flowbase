import { describe, expect, it } from 'vitest';
import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import '../../../workflow/register';
import {
  createNextNodeId,
  createNodeDocument
} from '../../lib/document/node-factory';
import { createEdgeDocument } from '../../lib/document/edge-factory';
import {
  duplicateNodeSubgraph,
  getDuplicatedNodeId
} from '../../lib/document/transforms/duplicate';
import { removeNodeSubgraph } from '../../lib/document/transforms/node';

describe('authoring membership and collision behavior', () => {
  it('retains the first available suffix without changing caller input', () => {
    const ids = [
      'node-llm-1',
      'node-llm-3',
      'node-llm-copy',
      'node-llm-copy-2'
    ];
    expect(createNextNodeId(ids, 'llm')).toBe('node-llm-2');
    expect(getDuplicatedNodeId(ids, 'node-llm')).toBe('node-llm-copy-3');
    expect(ids).toHaveLength(4);
  });

  it('preserves document order and remaps descendants through copy collisions', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'membership' });
    const root = createNodeDocument('iteration', 'container');
    const child = {
      ...createNodeDocument('answer', 'child'),
      containerId: root.id
    };
    document.graph.nodes = [
      child,
      root,
      createNodeDocument('iteration', 'container-copy')
    ];
    document.graph.edges = [
      createEdgeDocument({
        id: 'inside',
        source: root.id,
        target: child.id,
        containerId: root.id
      }),
      createEdgeDocument({
        id: 'inside-copy',
        source: 'container-copy',
        target: 'container-copy',
        containerId: null
      })
    ];
    const copy = duplicateNodeSubgraph(document, { nodeId: root.id });
    expect(copy.graph.nodes.map((node) => node.id)).toEqual([
      'child',
      'container',
      'container-copy',
      'child-copy',
      'container-copy-2'
    ]);
    expect(copy.graph.nodes.at(-2)?.containerId).toBe('container-copy-2');
    expect(copy.graph.edges.at(-1)).toMatchObject({
      id: 'inside-copy-2',
      source: 'container-copy-2',
      target: 'child-copy',
      containerId: 'container-copy-2'
    });
    const removed = removeNodeSubgraph(document, { nodeId: root.id });
    expect(removed.graph.nodes.map((node) => node.id)).toEqual([
      'container-copy'
    ]);
    expect(document.graph.nodes.map((node) => node.id)).toEqual([
      'child',
      'container',
      'container-copy'
    ]);
  });
});
