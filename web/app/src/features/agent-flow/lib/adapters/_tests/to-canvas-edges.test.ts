import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { describe, expect, test, vi } from 'vitest';

import { MAIN_SOURCE_HANDLE_ID } from '../../canvas/handle-ids';
import { toCanvasEdges } from '../to-canvas-edges';

describe('toCanvasEdges', () => {
  test('preserves edge order, handles, selection and insertion actions', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'edges' });
    const [first, second] = document.graph.edges;
    document.graph.edges = [
      { ...second, sourceHandle: 'output', targetHandle: 'input' },
      { ...first, sourceHandle: null },
      { ...first, id: 'answer-out', source: 'node-answer' }
    ];
    const actions = { nodePickerOptions: [], onInsertNode: vi.fn() };

    const edges = toCanvasEdges(document, null, first.id, actions);

    expect(edges.map((edge) => edge.id)).toEqual([
      second.id,
      first.id,
      'answer-out'
    ]);
    expect(edges.map((edge) => edge.selected)).toEqual([false, true, false]);
    expect(edges[0]).toMatchObject({
      sourceHandle: 'output',
      targetHandle: 'input',
      animated: false,
      type: 'agentFlowEdge'
    });
    expect(edges[1].sourceHandle).toBe(MAIN_SOURCE_HANDLE_ID);
    expect(edges[0].data?.nodePickerOptions).toBe(actions.nodePickerOptions);
    expect(edges[1].data?.onInsertNode).toBe(actions.onInsertNode);
    expect(edges[2].data?.onInsertNode).toBeUndefined();
  });

  test('excludes dangling and cross-container edges in both canvas views', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'containers' });
    const [first] = document.graph.edges;
    document.graph.nodes.push({
      ...document.graph.nodes[0],
      id: 'nested',
      containerId: 'loop'
    });
    document.graph.edges.push(
      { ...first, id: 'dangling-target', target: 'missing' },
      { ...first, id: 'dangling-source', source: 'missing' },
      { ...first, id: 'cross-container', target: 'nested' },
      { ...first, id: 'wrong-container', containerId: 'loop' },
      {
        ...first,
        id: 'nested-self',
        source: 'nested',
        target: 'nested',
        containerId: 'loop'
      }
    );
    const actions = { nodePickerOptions: [], onInsertNode: vi.fn() };

    expect(
      toCanvasEdges(document, null, null, actions).map((edge) => edge.id)
    ).toEqual(document.graph.edges.slice(0, 2).map((edge) => edge.id));
    expect(
      toCanvasEdges(document, 'loop', null, actions).map((edge) => edge.id)
    ).toEqual(['nested-self']);
  });

  test('retains the first source type when duplicate IDs span containers', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'duplicates' });
    document.graph.nodes.unshift({
      ...document.graph.nodes[0],
      type: 'answer',
      containerId: 'other'
    });
    const actions = { nodePickerOptions: [], onInsertNode: vi.fn() };

    const edges = toCanvasEdges(document, null, null, actions);

    expect(edges).toHaveLength(2);
    expect(edges[0].data?.onInsertNode).toBeUndefined();
    expect(edges[1].data?.onInsertNode).toBe(actions.onInsertNode);
  });
});
