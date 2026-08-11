import { describe, expect, test } from 'vitest';
import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { arrangeCanvasLeftToRight, shiftDownstreamNodesBFS } from '../layout';
import { createLlmToolSourceHandleId } from '../../../llm-node-config';
import { createEdgeDocument } from '../../edge-factory';
import { createNodeDocument } from '../../node-factory';

describe('Topological BFS Downstream Shifting', () => {
  test('shifts downstream nodes when parent moves forward', () => {
    // 1. Initial document: start -> llm -> answer
    // Default positions might be arbitrary. Let's explicitly set positions for test predictability:
    // start (x: 100, y: 100) -> llm (x: 380, y: 100) -> answer (x: 660, y: 100)
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    document.graph.nodes = document.graph.nodes.map((node) => {
      if (node.id === 'node-start') {
        return { ...node, position: { x: 100, y: 100 } };
      }
      if (node.id === 'node-llm') {
        return { ...node, position: { x: 380, y: 100 } };
      }
      if (node.id === 'node-answer') {
        return { ...node, position: { x: 660, y: 100 } };
      }
      return node;
    });

    // Move start node forward to x: 200.
    // The gap threshold is 280.
    // llm's minimum x = 200 + 280 = 480.
    // Since llm's current x is 380 < 480, llm should shift to x: 480.
    // answer's minimum x = 480 + 280 = 760.
    // Since answer's current x is 660 < 760, answer should shift to x: 760.
    document.graph.nodes = document.graph.nodes.map((node) =>
      node.id === 'node-start'
        ? { ...node, position: { x: 200, y: 100 } }
        : node
    );

    const updatedDoc = shiftDownstreamNodesBFS(document, 'node-start', 280);

    const startNode = updatedDoc.graph.nodes.find((n) => n.id === 'node-start');
    const llmNode = updatedDoc.graph.nodes.find((n) => n.id === 'node-llm');
    const answerNode = updatedDoc.graph.nodes.find(
      (n) => n.id === 'node-answer'
    );

    expect(startNode?.position.x).toBe(200);
    expect(llmNode?.position.x).toBe(480);
    expect(answerNode?.position.x).toBe(760);
  });

  test('does not shift parent nodes when downstream node is shifted', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    document.graph.nodes = document.graph.nodes.map((node) => {
      if (node.id === 'node-start') {
        return { ...node, position: { x: 100, y: 100 } };
      }
      if (node.id === 'node-llm') {
        return { ...node, position: { x: 380, y: 100 } };
      }
      if (node.id === 'node-answer') {
        return { ...node, position: { x: 660, y: 100 } };
      }
      return node;
    });

    // Move llm node forward to x: 500.
    // answer's minimum x = 500 + 280 = 780.
    // Since answer's current x is 660 < 780, answer shifts to 780.
    // start is a parent of llm, it should NOT shift. It stays at 100.
    document.graph.nodes = document.graph.nodes.map((node) =>
      node.id === 'node-llm' ? { ...node, position: { x: 500, y: 100 } } : node
    );

    const updatedDoc = shiftDownstreamNodesBFS(document, 'node-llm', 280);

    const startNode = updatedDoc.graph.nodes.find((n) => n.id === 'node-start');
    const llmNode = updatedDoc.graph.nodes.find((n) => n.id === 'node-llm');
    const answerNode = updatedDoc.graph.nodes.find(
      (n) => n.id === 'node-answer'
    );

    expect(startNode?.position.x).toBe(100);
    expect(llmNode?.position.x).toBe(500);
    expect(answerNode?.position.x).toBe(780);
  });

  test('does not shift parallel branches that are not connected downstream', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    // Add a parallel node 'node-parallel' with position (x: 400, y: 300)
    // No edges connect 'node-llm' to 'node-parallel'
    document.graph.nodes.push({
      id: 'node-parallel',
      type: 'llm',
      alias: 'Parallel',
      containerId: null,
      position: { x: 400, y: 300 },
      configVersion: 1,
      config: {},
      bindings: {},
      outputs: []
    });

    // Set other nodes
    document.graph.nodes = document.graph.nodes.map((node) => {
      if (node.id === 'node-start') {
        return { ...node, position: { x: 100, y: 100 } };
      }
      if (node.id === 'node-llm') {
        return { ...node, position: { x: 380, y: 100 } };
      }
      if (node.id === 'node-answer') {
        return { ...node, position: { x: 660, y: 100 } };
      }
      return node;
    });

    // Shift start node to x: 200, which shifts llm to 480 and answer to 760.
    // 'node-parallel' is NOT a downstream outgoer of start or llm, so it should stay at 400.
    document.graph.nodes = document.graph.nodes.map((node) =>
      node.id === 'node-start'
        ? { ...node, position: { x: 200, y: 100 } }
        : node
    );

    const updatedDoc = shiftDownstreamNodesBFS(document, 'node-start', 280);

    const parallelNode = updatedDoc.graph.nodes.find(
      (n) => n.id === 'node-parallel'
    );
    expect(parallelNode?.position.x).toBe(400);
  });

  test('resolves vertical overlapping of sibling nodes using AABB MTV', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    // Connect node-start to node-llm and a new sibling node-sibling
    document.graph.nodes.push({
      id: 'node-sibling',
      type: 'llm',
      alias: 'Sibling',
      containerId: null,
      position: { x: 380, y: 110 }, // Overlaps in Y with node-llm (x: 380, y: 100)
      configVersion: 1,
      config: {},
      bindings: {},
      outputs: []
    });

    document.graph.edges.push({
      id: 'edge-start-sibling',
      source: 'node-start',
      target: 'node-sibling',
      sourceHandle: null,
      targetHandle: null,
      containerId: null,
      points: []
    });

    // Explicitly set positions
    document.graph.nodes = document.graph.nodes.map((node) => {
      if (node.id === 'node-start') {
        return { ...node, position: { x: 100, y: 100 } };
      }
      if (node.id === 'node-llm') {
        return { ...node, position: { x: 380, y: 100 } };
      }
      return node;
    });

    // Move start node forward to trigger shifting.
    // This will trigger BFS which shifts both node-llm and node-sibling.
    // They are both active, and they overlap vertically at x: 480.
    document.graph.nodes = document.graph.nodes.map((node) =>
      node.id === 'node-start'
        ? { ...node, position: { x: 200, y: 100 } }
        : node
    );

    const updatedDoc = shiftDownstreamNodesBFS(document, 'node-start', 280);

    const llmNode = updatedDoc.graph.nodes.find((n) => n.id === 'node-llm');
    const siblingNode = updatedDoc.graph.nodes.find(
      (n) => n.id === 'node-sibling'
    );

    if (!llmNode || !siblingNode) {
      throw new Error('expected shifted downstream nodes to exist');
    }

    // Verify they are separated vertically: difference in Y must be at least NODE_HEIGHT (96) + gapY (40) = 136px
    const yDiff = Math.abs(llmNode.position.y - siblingNode.position.y);
    expect(yDiff).toBeGreaterThanOrEqual(136);
  });
});

describe('Canvas left-to-right arrangement', () => {
  test('arranges the whole root canvas by graph layers', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    document.graph.nodes = document.graph.nodes.map((node) => {
      if (node.id === 'node-start') {
        return { ...node, position: { x: 900, y: 420 } };
      }
      if (node.id === 'node-llm') {
        return { ...node, position: { x: 120, y: 120 } };
      }
      if (node.id === 'node-answer') {
        return { ...node, position: { x: 360, y: 130 } };
      }
      return node;
    });

    const arranged = arrangeCanvasLeftToRight(document, null);

    const startNode = arranged.graph.nodes.find(
      (node) => node.id === 'node-start'
    );
    const llmNode = arranged.graph.nodes.find((node) => node.id === 'node-llm');
    const answerNode = arranged.graph.nodes.find(
      (node) => node.id === 'node-answer'
    );

    expect(startNode?.position).toEqual({ x: 120, y: 160 });
    expect(llmNode?.position).toEqual({ x: 400, y: 160 });
    expect(answerNode?.position).toEqual({ x: 680, y: 160 });
  });

  test('separates sibling nodes and returns a stable layout on repeated arrangement', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    document.graph.nodes.push({
      id: 'node-sibling',
      type: 'llm',
      alias: 'Sibling',
      containerId: null,
      position: { x: 380, y: 110 },
      configVersion: 1,
      config: {},
      bindings: {},
      outputs: []
    });
    document.graph.edges.push({
      id: 'edge-start-sibling',
      source: 'node-start',
      target: 'node-sibling',
      sourceHandle: null,
      targetHandle: null,
      containerId: null,
      points: []
    });

    const arranged = arrangeCanvasLeftToRight(document, null);
    const arrangedAgain = arrangeCanvasLeftToRight(arranged, null);
    const llmNode = arranged.graph.nodes.find((node) => node.id === 'node-llm');
    const siblingNode = arranged.graph.nodes.find(
      (node) => node.id === 'node-sibling'
    );

    if (!llmNode || !siblingNode) {
      throw new Error('expected sibling layer nodes to exist');
    }

    expect(llmNode.position.x).toBe(400);
    expect(siblingNode.position.x).toBe(400);
    expect(
      Math.abs(llmNode.position.y - siblingNode.position.y)
    ).toBeGreaterThanOrEqual(136);
    expect(
      arrangedAgain.graph.nodes.map((node) => [node.id, node.position])
    ).toEqual(arranged.graph.nodes.map((node) => [node.id, node.position]));
  });

  test('uses dynamic branch-node height when separating a dense sibling layer', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const branchNode = createNodeDocument('if_else', 'node-branches', 380, 100);

    branchNode.bindings.branches = {
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
          ...[1, 2, 3].map((index) => ({
            id: `else-if-${index}`,
            kind: 'else_if' as const,
            title: `Else If ${index}`,
            sourceHandle: `else-if-${index}`,
            condition: { operator: 'and' as const, conditions: [] }
          })),
          {
            id: 'else',
            kind: 'else',
            title: 'Else',
            sourceHandle: 'else'
          }
        ]
      }
    };
    document.graph.nodes.push(branchNode);
    document.graph.edges.push(
      createEdgeDocument({
        id: 'edge-start-branches',
        source: 'node-start',
        target: branchNode.id
      })
    );

    const arranged = arrangeCanvasLeftToRight(document, null);
    const llmNode = arranged.graph.nodes.find((node) => node.id === 'node-llm');
    const arrangedBranchNode = arranged.graph.nodes.find(
      (node) => node.id === branchNode.id
    );

    if (!llmNode || !arrangedBranchNode) {
      throw new Error('expected dense branch sibling layer nodes to exist');
    }

    const upperNode =
      llmNode.position.y < arrangedBranchNode.position.y
        ? { node: llmNode, height: 96 }
        : { node: arrangedBranchNode, height: 192 };
    const lowerNode =
      llmNode.position.y < arrangedBranchNode.position.y
        ? arrangedBranchNode
        : llmNode;

    expect(lowerNode.position.y - upperNode.node.position.y).toBeGreaterThanOrEqual(
      upperNode.height + 40
    );
  });

  test('keeps LLM tool-mounted nodes in a vertical mount lane without flattening the main flow', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const toolHandleA = createLlmToolSourceHandleId('search_tool');
    const toolHandleB = createLlmToolSourceHandleId('quote_tool');

    document.graph.nodes.push(
      createNodeDocument('llm', 'node-mounted-llm-a', 40, 600),
      createNodeDocument('llm', 'node-mounted-llm-b', 40, 620),
      createNodeDocument('tool_result', 'node-tool-result', 40, 640)
    );
    document.graph.edges.push(
      createEdgeDocument({
        id: 'edge-llm-mounted-a',
        source: 'node-llm',
        target: 'node-mounted-llm-a',
        sourceHandle: toolHandleA
      }),
      createEdgeDocument({
        id: 'edge-llm-mounted-b',
        source: 'node-llm',
        target: 'node-mounted-llm-b',
        sourceHandle: toolHandleB
      }),
      createEdgeDocument({
        id: 'edge-mounted-a-tool-result',
        source: 'node-mounted-llm-a',
        target: 'node-tool-result'
      })
    );

    const arranged = arrangeCanvasLeftToRight(document, null);
    const arrangedAgain = arrangeCanvasLeftToRight(arranged, null);
    const llmNode = arranged.graph.nodes.find((node) => node.id === 'node-llm');
    const answerNode = arranged.graph.nodes.find(
      (node) => node.id === 'node-answer'
    );
    const mountedNodeA = arranged.graph.nodes.find(
      (node) => node.id === 'node-mounted-llm-a'
    );
    const mountedNodeB = arranged.graph.nodes.find(
      (node) => node.id === 'node-mounted-llm-b'
    );
    const toolResultNode = arranged.graph.nodes.find(
      (node) => node.id === 'node-tool-result'
    );

    if (
      !llmNode ||
      !answerNode ||
      !mountedNodeA ||
      !mountedNodeB ||
      !toolResultNode
    ) {
      throw new Error('expected arranged nodes to exist');
    }

    expect(answerNode.position).toEqual({
      x: llmNode.position.x + 280,
      y: llmNode.position.y
    });
    expect(mountedNodeA.position.x).toBe(llmNode.position.x + 280);
    expect(mountedNodeB.position.x).toBe(llmNode.position.x + 280);
    expect(mountedNodeA.position.y).toBeGreaterThan(llmNode.position.y + 96);
    expect(
      mountedNodeB.position.y - mountedNodeA.position.y
    ).toBeGreaterThanOrEqual(136);
    expect(toolResultNode.position).toEqual({
      x: mountedNodeA.position.x + 280,
      y: mountedNodeA.position.y
    });
    expect(
      arrangedAgain.graph.nodes.map((node) => [node.id, node.position])
    ).toEqual(arranged.graph.nodes.map((node) => [node.id, node.position]));
  });
});
