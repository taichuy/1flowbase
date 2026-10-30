import {
  createDefaultAgentFlowDocument,
  type FlowAuthoringDocument
} from '@1flowbase/flow-schema';
import { describe, expect, test, vi } from 'vitest';
import { createNodeDocument } from '../../node-factory';
import { getNodeById } from '../../selectors';
import {
  initializeContainer,
  getNodeAbsolutePosition,
  getNodeSize,
  resizeContainer
} from '../container-geometry';
import { insertNodeAfter, moveNodes, removeNodeSubgraph } from '../node';
import { duplicateNodeSubgraph } from '../duplicate';
import { makeRoomForContainerAncestors } from '../layout';
import { connectNodes, validateConnection } from '../edge';
import { toCanvasNodes } from '../../../adapters/to-canvas-nodes';
import { toCanvasEdges } from '../../../adapters/to-canvas-edges';
import { createAgentFlowEditorStore } from '../../../../store/editor';

function fixture() {
  const document = createDefaultAgentFlowDocument({ flowId: 'nested' });
  document.graph.nodes = [createNodeDocument('loop', 'outer', 100, 200)];
  document.graph.edges = [];
  let next = initializeContainer(document, 'outer');
  next = insertNodeAfter(
    next,
    'outer-start',
    createNodeDocument('iteration', 'middle')
  );
  next = insertNodeAfter(
    next,
    'middle-start',
    createNodeDocument('loop', 'inner')
  );
  next = insertNodeAfter(
    next,
    'inner-start',
    createNodeDocument('code', 'leaf')
  );
  next = insertNodeAfter(next, 'leaf', createNodeDocument('code', 'tail'));
  const leaf = getNodeById(next, 'leaf')!;
  leaf.bindings = {
    internal: { kind: 'selector', value: ['inner', 'index'] },
    external: { kind: 'selector', value: ['external', 'value'] }
  };
  return next;
}

function state(document: FlowAuthoringDocument) {
  return {
    flow_id: 'nested',
    messages: [],
    draft: {
      id: 'draft',
      flow_id: 'nested',
      updated_at: '2026-10-29T00:00:00Z',
      document
    },
    versions: [],
    autosave_interval_seconds: 30,
    user_protection_limit: 10
  };
}

function expectContained(document: FlowAuthoringDocument) {
  for (const node of document.graph.nodes) {
    if (!node.containerId) continue;
    const parent = getNodeById(document, node.containerId)!;
    expect(node.position.x).toBeGreaterThanOrEqual(32);
    expect(node.position.y).toBeGreaterThanOrEqual(88);
    expect(node.position.x + getNodeSize(node).width + 32).toBeLessThanOrEqual(
      parent.width!
    );
    expect(node.position.y + getNodeSize(node).height + 32).toBeLessThanOrEqual(
      parent.height!
    );
  }
}

const actions = {
  nodePickerOptions: [],
  onOpenPicker: vi.fn(),
  onClosePicker: vi.fn(),
  onOpenContainer: vi.fn(),
  onSelectNode: vi.fn(),
  onInsertNode: vi.fn(),
  onReplaceNode: vi.fn(),
  onDeleteNode: vi.fn()
};

describe('same-canvas nested containers', () => {
  test('inserts a single internal start per level and projects all nodes parent-first', () => {
    const document = fixture();
    expectContained(document);
    expect(
      document.graph.nodes.filter((node) => node.type === 'container_start')
    ).toHaveLength(3);
    expect(initializeContainer(document, 'inner').graph.nodes).toHaveLength(
      document.graph.nodes.length
    );
    const reversed = {
      ...document,
      graph: { ...document.graph, nodes: [...document.graph.nodes].reverse() }
    };
    const nodes = toCanvasNodes(
      reversed,
      'inner',
      'leaf',
      null,
      null,
      {},
      actions
    );
    expect(nodes).toHaveLength(document.graph.nodes.length);
    for (const node of nodes) {
      if (node.parentId)
        expect(
          nodes.findIndex((parent) => parent.id === node.parentId)
        ).toBeLessThan(nodes.indexOf(node));
    }
    expect(
      nodes.filter((node) => node.selected).map((node) => node.id)
    ).toEqual(['leaf']);
    expect(nodes.find((node) => node.id === 'leaf')).toMatchObject({
      parentId: 'inner',
      position: getNodeById(document, 'leaf')!.position
    });
    expect(nodes.find((node) => node.id === 'outer-start')).toMatchObject({
      type: 'containerStart',
      deletable: false,
      data: { showTargetHandle: false }
    });
    expect(
      toCanvasEdges(document, 'inner', null, {
        nodePickerOptions: [],
        onInsertNode: vi.fn()
      })
    ).toHaveLength(document.graph.edges.length);
  });

  test('moving a parent moves descendant absolute positions exactly once and resizing contains every level', () => {
    const document = fixture();
    const before = getNodeAbsolutePosition(document, 'leaf');
    const moved = moveNodes(document, { outer: { x: 500, y: 600 } });
    expect(getNodeById(moved, 'leaf')!.position).toEqual(
      getNodeById(document, 'leaf')!.position
    );
    expect(getNodeAbsolutePosition(moved, 'leaf')).toEqual({
      x: before.x + 400,
      y: before.y + 400
    });
    const resized = resizeContainer(moved, 'inner', {
      width: 1800,
      height: 900
    });
    expectContained(resized);
    expect(getNodeById(resized, 'inner')!.width).toBe(1800);
    expectContained(resizeContainer(resized, 'outer', { width: 1, height: 1 }));
    expectContained(moveNodes(resized, { leaf: { x: -100, y: -100 } }));
  });

  test('growing an inner body makes room for outer successor nodes without shifting child coordinates', () => {
    let document = fixture();
    document = insertNodeAfter(
      document,
      'outer',
      createNodeDocument('code', 'after-outer')
    );
    const before = getNodeById(document, 'leaf')!.position;
    const next = makeRoomForContainerAncestors(
      resizeContainer(document, 'inner', { width: 2400, height: 700 }),
      'inner'
    );
    const outer = getNodeById(next, 'outer')!;
    expect(getNodeById(next, 'after-outer')!.position.x).toBeGreaterThan(
      outer.position.x + outer.width!
    );
    expect(getNodeById(next, 'leaf')!.position).toEqual(before);
    expectContained(next);
  });

  test('copies the entire tree with one root offset, local references remapped and external references retained', () => {
    const document = fixture();
    const copied = duplicateNodeSubgraph(document, { nodeId: 'outer' });
    expect(getNodeById(copied, 'outer-copy')!.position).toEqual({
      x: 148,
      y: 248
    });
    for (const id of ['middle', 'inner', 'leaf', 'outer-start', 'tail']) {
      expect(getNodeById(copied, `${id}-copy`)!.position).toEqual(
        getNodeById(document, id)!.position
      );
    }
    expect(getNodeById(copied, 'leaf-copy')!.bindings).toEqual({
      internal: { kind: 'selector', value: ['inner-copy', 'index'] },
      external: { kind: 'selector', value: ['external', 'value'] }
    });
    expect(
      copied.graph.edges.find((edge) => edge.source === 'leaf-copy')
    ).toMatchObject({ target: 'tail-copy', containerId: 'inner-copy' });
    expectContained(copied);
  });

  test('rejects cross-layer edges, incoming start edges and cycles while preserving valid sibling connections', () => {
    const document = fixture();
    expect(
      validateConnection(document, { source: 'leaf', target: 'middle' })
    ).toBe(false);
    expect(
      validateConnection(document, { source: 'leaf', target: 'inner-start' })
    ).toBe(false);
    expect(
      validateConnection(document, { source: 'tail', target: 'leaf' })
    ).toBe(false);
    const disconnected = {
      ...document,
      graph: {
        ...document.graph,
        edges: document.graph.edges.filter((edge) => edge.target !== 'tail')
      }
    };
    expect(
      connectNodes(disconnected, {
        connection: { source: 'leaf', target: 'tail' }
      }).graph.edges.at(-1)
    ).toMatchObject({ containerId: 'inner' });
    expect(removeNodeSubgraph(document, { nodeId: 'inner-start' })).toBe(
      document
    );
    expect(duplicateNodeSubgraph(document, { nodeId: 'inner-start' })).toBe(
      document
    );
  });

  test('deletes, undoes, redoes and reloads the complete persisted tree as one history step', () => {
    const document = fixture();
    const store = createAgentFlowEditorStore(state(document));
    store
      .getState()
      .setWorkingDocument(removeNodeSubgraph(document, { nodeId: 'middle' }));
    expect(
      store.getState().workingDocument.graph.nodes.map((node) => node.id)
    ).toEqual(['outer', 'outer-start']);
    store.getState().restorePreviousDocument();
    expect(store.getState().workingDocument).toEqual(document);
    store.getState().restoreNextDocument();
    expect(store.getState().workingDocument.graph.nodes).toHaveLength(2);
    store.getState().restorePreviousDocument();
    const reloaded = createAgentFlowEditorStore(
      state(JSON.parse(JSON.stringify(store.getState().workingDocument)))
    );
    expect(reloaded.getState().workingDocument).toEqual(document);
    expectContained(reloaded.getState().workingDocument);
    store
      .getState()
      .setWorkingDocument(moveNodes(document, { outer: { x: 800, y: 900 } }));
    expect(store.getState().redoStack).toHaveLength(0);
  });
});
