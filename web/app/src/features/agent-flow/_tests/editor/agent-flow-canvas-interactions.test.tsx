import type { ReactNode } from 'react';
import { useEffect } from 'react';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import {
  createDefaultAgentFlowDocument,
  type FlowNodeType
} from '@1flowbase/flow-schema';
import { AgentFlowCanvas } from '../../components/editor/AgentFlowCanvas';
import { createNodeDocument } from '../../lib/document/node-factory';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import { useAgentFlowEditorStore } from '../../store/editor/provider';
import { selectWorkingDocument } from '../../store/editor/selectors';

type MockNodeChange = {
  id: string;
  type: string;
  dragging?: boolean;
  position?: { x: number; y: number };
};

type MockViewport = {
  x: number;
  y: number;
  zoom: number;
};

type MockReactFlowProps = {
  children?: ReactNode;
  defaultViewport?: MockViewport;
  nodes?: Array<{
    id: string;
    height?: number;
    measured?: {
      height?: number;
      width?: number;
    };
    position?: {
      x: number;
      y: number;
    };
    width?: number;
    data?: {
      compactSourceHandle?: { id: string; title: string } | null;
      compactNodePickerOptions?: Array<{ type?: string }>;
      onRunNode?: (nodeId: string) => void;
      onSelectNode?: (nodeId: string) => void;
      showSourceHandle?: boolean;
    };
  }>;
  edges?: Array<{
    id: string;
    selected?: boolean;
    source?: string;
    target?: string;
    sourceHandle?: string | null;
    targetHandle?: string | null;
    data?: {
      onInsertNode?: (edgeId: string, nodeType: FlowNodeType) => void;
    };
  }>;
  onConnect?: (connection: {
    source: string;
    target: string;
    sourceHandle?: string | null;
    targetHandle?: string | null;
  }) => void;
  onConnectEnd?: (event: {
    clientX: number;
    clientY: number;
    target: EventTarget | null;
  }) => void;
  onConnectStart?: (
    event: unknown,
    payload: {
      nodeId: string | null;
      handleType: 'source' | 'target' | null;
      handleId: string | null;
    }
  ) => void;
  onEdgeClick?: (
    event: unknown,
    edge: {
      id: string;
    }
  ) => void;
  onPaneClick?: () => void;
  onNodesChange?: (changes: MockNodeChange[]) => void;
  onReconnect?: (
    oldEdge: {
      id: string;
      source: string;
      target: string;
      sourceHandle?: string | null;
      targetHandle?: string | null;
    },
    connection: {
      source: string;
      target: string;
      sourceHandle?: string | null;
      targetHandle?: string | null;
    }
  ) => void;
  onViewportChange?: (viewport: MockViewport) => void;
  viewport?: MockViewport;
};

let latestReactFlowProps: MockReactFlowProps | null = null;
let mockViewport: MockViewport = { x: 0, y: 0, zoom: 1 };
let latestViewportChangeOptions: {
  onStart?: (viewport: MockViewport) => void;
  onChange?: (viewport: MockViewport) => void;
  onEnd?: (viewport: MockViewport) => void;
} | null = null;

function createInitialState(
  document = createDefaultAgentFlowDocument({ flowId: 'flow-1' })
) {
  return {
    flow_id: 'flow-1',
    messages: [],
    draft: {
      id: 'draft-1',
      flow_id: 'flow-1',
      updated_at: '2026-04-16T10:00:00Z',
      document
    },
    autosave_interval_seconds: 30,
    user_protection_limit: 10,
    versions: []
  };
}

type ObservedEditorState = {
  nodePickerState: {
    open: boolean;
    anchorNodeId: string | null;
    anchorEdgeId: string | null;
    anchorCanvasPosition: { x: number; y: number } | null;
  };
  selectedEdgeId: string | null;
  selectedNodeId: string | null;
  workingDocument: ReturnType<typeof createDefaultAgentFlowDocument>;
};

function StoreObserver({
  onChange
}: {
  onChange: (state: ObservedEditorState) => void;
}) {
  const workingDocument = useAgentFlowEditorStore(selectWorkingDocument);
  const nodePickerState = useAgentFlowEditorStore(
    (state) => state.nodePickerState
  );
  const selectedEdgeId = useAgentFlowEditorStore(
    (state) => state.selectedEdgeId
  );
  const selectedNodeId = useAgentFlowEditorStore(
    (state) => state.selectedNodeId
  );

  useEffect(() => {
    onChange({
      nodePickerState,
      selectedEdgeId,
      selectedNodeId,
      workingDocument
    });
  }, [
    nodePickerState,
    onChange,
    selectedEdgeId,
    selectedNodeId,
    workingDocument
  ]);

  return null;
}

function renderCanvas(
  document = createDefaultAgentFlowDocument({ flowId: 'flow-1' })
) {
  let latestState: ObservedEditorState | null = null;

  render(
    <AgentFlowEditorStoreProvider initialState={createInitialState(document)}>
      <StoreObserver
        onChange={(state) => {
          latestState = state;
        }}
      />
      <AgentFlowCanvas issueCountByNodeId={{}} />
    </AgentFlowEditorStoreProvider>
  );

  return {
    getState() {
      if (!latestState) {
        throw new Error('editor state not observed');
      }

      return latestState;
    }
  };
}

vi.mock('@xyflow/react', () => ({
  Background: () => null,
  Controls: () => null,
  EdgeLabelRenderer: ({ children }: { children?: ReactNode }) =>
    children ?? null,
  Handle: () => null,
  MarkerType: {
    ArrowClosed: 'arrowclosed'
  },
  Panel: ({ children }: { children?: ReactNode }) => children ?? null,
  Position: {
    Left: 'left',
    Right: 'right'
  },
  ReactFlow: (props: MockReactFlowProps) => {
    latestReactFlowProps = props;
    mockViewport = props.viewport ?? props.defaultViewport ?? mockViewport;

    return (
      <div data-testid="mock-react-flow">
        <button
          type="button"
          onClick={() =>
            props.onNodesChange?.([
              {
                id: 'node-llm',
                type: 'position',
                dragging: true,
                position: { x: 480, y: 250 }
              }
            ])
          }
        >
          trigger node drag move
        </button>
        <button
          type="button"
          onClick={() =>
            props.onNodesChange?.([
              {
                id: 'node-llm',
                type: 'position',
                dragging: false,
                position: { x: 520, y: 260 }
              }
            ])
          }
        >
          trigger node drag
        </button>
        <button
          type="button"
          onClick={() => {
            mockViewport = { x: 120, y: 48, zoom: 0.85 };
            props.onViewportChange?.(mockViewport);
          }}
        >
          trigger viewport change
        </button>
        {props.children}
      </div>
    );
  },
  ReactFlowProvider: ({ children }: { children?: ReactNode }) =>
    children ?? null,
  getBezierPath: () => ['M0,0', 0, 0],
  useOnViewportChange: (options: {
    onStart?: (viewport: MockViewport) => void;
    onChange?: (viewport: MockViewport) => void;
    onEnd?: (viewport: MockViewport) => void;
  }) => {
    latestViewportChangeOptions = options;
  },
  useReactFlow: () => ({
    fitView: vi.fn(),
    screenToFlowPosition: ({ x, y }: { x: number; y: number }) => ({ x, y }),
    zoomIn: vi.fn(),
    zoomOut: vi.fn()
  }),
  useViewport: () => mockViewport
}));

describe('AgentFlowCanvas interactions', () => {
  beforeEach(() => {
    latestReactFlowProps = null;
    latestViewportChangeOptions = null;
    mockViewport = { x: 0, y: 0, zoom: 1 };
  });

  test('writes dragged node positions back into the document', () => {
    const { getState } = renderCanvas();

    fireEvent.click(screen.getByRole('button', { name: 'trigger node drag' }));

    expect(getState().workingDocument.graph.nodes).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'node-llm',
          position: { x: 520, y: 260 }
        })
      ])
    );
  });

  test('keeps node drag move positions local until the drag ends', () => {
    const { getState } = renderCanvas();
    const initialPosition = getState().workingDocument.graph.nodes.find(
      (node) => node.id === 'node-llm'
    )?.position;

    fireEvent.click(
      screen.getByRole('button', { name: 'trigger node drag move' })
    );

    expect(getState().workingDocument.graph.nodes).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'node-llm',
          position: initialPosition
        })
      ])
    );
    expect(
      latestReactFlowProps?.nodes?.find((node) => node.id === 'node-llm')
        ?.position
    ).toEqual({ x: 480, y: 250 });
  });

  test('projects stable measured dimensions for controlled React Flow nodes', () => {
    renderCanvas();

    expect(latestReactFlowProps?.nodes).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'node-llm',
          height: 96,
          measured: { height: 96, width: 196 },
          width: 196
        })
      ])
    );
  });

  test('projects default main edges to the right source handle when mounted tool handles exist', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const sourceLlm = document.graph.nodes.find(
      (node) => node.id === 'node-llm'
    );

    if (!sourceLlm) {
      throw new Error('expected default LLM node');
    }

    sourceLlm.config = {
      ...sourceLlm.config,
      visible_internal_llm_tools_enabled: true,
      visible_internal_llm_tools: [
        {
          type: 'visible_internal_llm_tool',
          tool_name: 'inspect_visible_context',
          connector_id: 'inspect_visible_context',
          target_node_id: '',
          input_schema: { type: 'object' }
        }
      ]
    };

    expect(
      document.graph.edges.find((edge) => edge.id === 'edge-llm-answer')
        ?.sourceHandle
    ).toBeNull();

    renderCanvas(document);

    expect(
      latestReactFlowProps?.edges?.find((edge) => edge.id === 'edge-llm-answer')
        ?.sourceHandle
    ).toBe('source-right');
  });

  test('opens with the document viewport and shows a plain percentage label', () => {
    renderCanvas();

    expect(latestReactFlowProps?.defaultViewport).toEqual({
      x: 0,
      y: 0,
      zoom: 1
    });
    expect(latestReactFlowProps?.viewport).toBeUndefined();
    expect(screen.getByLabelText('当前缩放')).toHaveTextContent('100%');
  });

  test('writes viewport changes back into the document after viewport movement ends', () => {
    const { getState } = renderCanvas();

    act(() => {
      latestViewportChangeOptions?.onChange?.({ x: 64, y: 32, zoom: 0.92 });
    });

    expect(getState().workingDocument.editor.viewport).toEqual({
      x: 0,
      y: 0,
      zoom: 1
    });

    act(() => {
      latestViewportChangeOptions?.onEnd?.({ x: 120, y: 48, zoom: 0.85 });
    });

    expect(getState().workingDocument.editor.viewport).toEqual({
      x: 120,
      y: 48,
      zoom: 0.85
    });
  });

  test('arranges the whole canvas from left to right through the canvas toolbar', () => {
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

    const { getState } = renderCanvas(document);

    fireEvent.click(screen.getByRole('button', { name: '自动整理' }));

    expect(getState().workingDocument.graph.nodes).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'node-start',
          position: { x: 120, y: 160 }
        }),
        expect.objectContaining({
          id: 'node-llm',
          position: { x: 400, y: 160 }
        }),
        expect.objectContaining({
          id: 'node-answer',
          position: { x: 680, y: 160 }
        })
      ])
    );
  });

  test('inserts a node through the edge action callback with a counted alias', () => {
    const { getState } = renderCanvas();

    expect(latestReactFlowProps).not.toBeNull();
    const insertOnEdge = latestReactFlowProps?.edges?.find(
      (edge) => edge.id === 'edge-llm-answer'
    )?.data?.onInsertNode;

    expect(insertOnEdge).toBeTypeOf('function');

    act(() => {
      insertOnEdge?.('edge-llm-answer', 'llm');
    });

    expect(getState().workingDocument.graph.nodes).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          type: 'llm',
          alias: 'LLM1'
        })
      ])
    );
    expect(getState().selectedNodeId).toMatch(/^node-llm-/);
  });

  test('rewrites the document edge when an existing line is reconnected', () => {
    const { getState } = renderCanvas();

    expect(latestReactFlowProps?.onReconnect).toBeTypeOf('function');

    act(() => {
      latestReactFlowProps?.onReconnect?.(
        {
          id: 'edge-start-llm',
          source: 'node-start',
          target: 'node-llm',
          sourceHandle: null,
          targetHandle: null
        },
        {
          source: 'node-start',
          target: 'node-answer',
          sourceHandle: 'source-right',
          targetHandle: 'target-left'
        }
      );
    });

    expect(getState().workingDocument.graph.edges).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'edge-start-llm',
          source: 'node-start',
          target: 'node-answer',
          sourceHandle: 'source-right',
          targetHandle: 'target-left'
        })
      ])
    );
  });

  test('creates a new edge when a source handle connects to another node', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    document.graph.edges = document.graph.edges.filter(
      (edge) => edge.id !== 'edge-start-llm'
    );

    const { getState } = renderCanvas(document);

    expect(latestReactFlowProps?.onConnect).toBeTypeOf('function');

    act(() => {
      latestReactFlowProps?.onConnect?.({
        source: 'node-start',
        target: 'node-llm',
        sourceHandle: 'source-right',
        targetHandle: 'target-left'
      });
    });

    expect(getState().workingDocument.graph.edges).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          source: 'node-start',
          target: 'node-llm',
          sourceHandle: 'source-right',
          targetHandle: 'target-left'
        })
      ])
    );
  });

  test('projects one typed Compact handle and only connects it to Compact Response', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-compact' });
    const startNode = document.graph.nodes.find(
      (node) => node.id === 'node-start'
    );

    if (!startNode) {
      throw new Error('expected default Start node');
    }

    startNode.config.compact_dispatch = 'application_flow';
    document.graph.nodes.push(
      createNodeDocument(
        'compact_response',
        'node-compact-response',
        920,
        360
      )
    );

    const { getState } = renderCanvas(document);
    const startCanvasNode = latestReactFlowProps?.nodes?.find(
      (node) => node.id === 'node-start'
    );
    const answerCanvasNode = latestReactFlowProps?.nodes?.find(
      (node) => node.id === 'node-answer'
    );
    const compactCanvasNode = latestReactFlowProps?.nodes?.find(
      (node) => node.id === 'node-compact-response'
    );

    expect(startCanvasNode?.data?.compactSourceHandle).toEqual(
      expect.objectContaining({ id: 'compact' })
    );
    expect(startCanvasNode?.data?.compactNodePickerOptions).toEqual([
      expect.objectContaining({ type: 'compact_response' })
    ]);
    expect(answerCanvasNode?.data?.showSourceHandle).toBe(false);
    expect(compactCanvasNode?.data?.showSourceHandle).toBe(false);
    expect(compactCanvasNode?.data?.onRunNode).toBeUndefined();

    act(() => {
      latestReactFlowProps?.onConnect?.({
        source: 'node-start',
        target: 'node-compact-response',
        sourceHandle: 'compact',
        targetHandle: null
      });
      latestReactFlowProps?.onConnect?.({
        source: 'node-start',
        target: 'node-llm',
        sourceHandle: 'compact',
        targetHandle: null
      });
    });

    const compactEdges = getState().workingDocument.graph.edges.filter(
      (edge) => edge.source === 'node-start' && edge.sourceHandle === 'compact'
    );

    expect(compactEdges).toEqual([
      expect.objectContaining({ target: 'node-compact-response' })
    ]);
  });

  test('creates a composable edge from an LLM tool connector', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const transformNode = createNodeDocument(
      'template_transform',
      'node-tool-transform',
      720,
      240
    );
    const sourceLlm = document.graph.nodes.find(
      (node) => node.id === 'node-llm'
    );

    if (!sourceLlm) {
      throw new Error('expected default LLM node');
    }

    document.graph.nodes.push(transformNode);
    sourceLlm.config = {
      ...sourceLlm.config,
      visible_internal_llm_tools_enabled: true,
      visible_internal_llm_tools: [
        {
          type: 'visible_internal_llm_tool',
          tool_name: 'inspect_visible_context',
          connector_id: 'inspect_visible_context',
          target_node_id: '',
          input_schema: { type: 'object' }
        }
      ]
    };

    const { getState } = renderCanvas(document);

    expect(latestReactFlowProps?.onConnect).toBeTypeOf('function');

    act(() => {
      latestReactFlowProps?.onConnect?.({
        source: 'node-llm',
        target: 'node-tool-transform',
        sourceHandle: 'visible_internal_llm_tool:inspect_visible_context',
        targetHandle: null
      });
    });

    const nextSourceLlm = getState().workingDocument.graph.nodes.find(
      (node) => node.id === 'node-llm'
    );

    expect(nextSourceLlm?.config.visible_internal_llm_tools).toEqual([
      expect.objectContaining({
        tool_name: 'inspect_visible_context',
        target_node_id: ''
      })
    ]);
    expect(getState().workingDocument.graph.edges).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          source: 'node-llm',
          target: 'node-tool-transform',
          sourceHandle: 'visible_internal_llm_tool:inspect_visible_context',
          targetHandle: null
        })
      ])
    );
  });

  test('opens the node picker when an LLM tool connector stops on the pane', async () => {
    const flowDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const sourceLlm = flowDocument.graph.nodes.find(
      (node) => node.id === 'node-llm'
    );

    if (!sourceLlm) {
      throw new Error('expected default LLM node');
    }

    sourceLlm.config = {
      ...sourceLlm.config,
      visible_internal_llm_tools_enabled: true,
      visible_internal_llm_tools: [
        {
          type: 'visible_internal_llm_tool',
          tool_name: 'inspect_visible_context',
          connector_id: 'inspect_visible_context',
          target_node_id: '',
          input_schema: { type: 'object' }
        }
      ]
    };

    const { getState } = renderCanvas(flowDocument);

    act(() => {
      latestReactFlowProps?.onConnectStart?.(null, {
        nodeId: 'node-llm',
        handleType: 'source',
        handleId: 'visible_internal_llm_tool:inspect_visible_context'
      });
      latestReactFlowProps?.onConnectEnd?.({
        clientX: 420,
        clientY: 260,
        target: document.createElement('div')
      });
    });

    expect(getState().nodePickerState).toEqual({
      open: true,
      anchorNodeId: 'node-llm',
      anchorEdgeId: null,
      anchorCanvasPosition: { x: 420, y: 260 }
    });

    fireEvent.click(await screen.findByRole('menuitem', { name: 'LLM' }));

    const insertedNode = getState().workingDocument.graph.nodes.find(
      (node) => node.id !== 'node-llm' && node.type === 'llm'
    );

    expect(insertedNode).toMatchObject({
      type: 'llm',
      alias: 'LLM1',
      position: { x: 609, y: 405 }
    });
    expect(getState().workingDocument.graph.edges).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          source: 'node-llm',
          target: insertedNode?.id,
          sourceHandle: 'visible_internal_llm_tool:inspect_visible_context'
        })
      ])
    );
  });

  test('opens the shared node picker when a dragged connection stops on the pane', () => {
    const { getState } = renderCanvas();

    expect(latestReactFlowProps?.onConnectStart).toBeTypeOf('function');
    expect(latestReactFlowProps?.onConnectEnd).toBeTypeOf('function');

    act(() => {
      latestReactFlowProps?.onConnectStart?.(null, {
        nodeId: 'node-llm',
        handleType: 'source',
        handleId: 'source-right'
      });
      latestReactFlowProps?.onConnectEnd?.({
        clientX: 420,
        clientY: 260,
        target: document.createElement('div')
      });
    });

    expect(getState().nodePickerState).toEqual({
      open: true,
      anchorNodeId: 'node-llm',
      anchorEdgeId: null,
      anchorCanvasPosition: { x: 420, y: 260 }
    });
  });

  test('creates a branched node from the dragged connection without rewriting the original outgoing edge', async () => {
    const { getState } = renderCanvas();

    act(() => {
      latestReactFlowProps?.onConnectStart?.(null, {
        nodeId: 'node-llm',
        handleType: 'source',
        handleId: 'source-right'
      });
      latestReactFlowProps?.onConnectEnd?.({
        clientX: 420,
        clientY: 260,
        target: document.createElement('div')
      });
    });
    fireEvent.click(
      await screen.findByRole('menuitem', { name: 'Template Transform' })
    );

    const insertedNode = getState().workingDocument.graph.nodes.find(
      (node) => node.type === 'template_transform'
    );

    expect(insertedNode).toMatchObject({
      type: 'template_transform',
      position: { x: 609, y: 405 }
    });
    expect(insertedNode?.position).toEqual({ x: 609, y: 405 });
    expect(getState().workingDocument.graph.edges).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'edge-llm-answer',
          source: 'node-llm',
          target: 'node-answer'
        }),
        expect.objectContaining({
          source: 'node-llm',
          target: insertedNode?.id
        })
      ])
    );
    expect(getState().workingDocument.graph.edges).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          source: insertedNode?.id,
          target: 'node-answer'
        })
      ])
    );
  });

  test('selects an edge when it is clicked', () => {
    const { getState } = renderCanvas();

    expect(latestReactFlowProps?.onEdgeClick).toBeTypeOf('function');

    act(() => {
      latestReactFlowProps?.onEdgeClick?.(null, {
        id: 'edge-llm-answer'
      });
    });

    expect(getState().selectedEdgeId).toBe('edge-llm-answer');
    expect(latestReactFlowProps?.edges).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'edge-llm-answer',
          selected: true
        })
      ])
    );
  });

  test('does not close node detail when clicking the pane after a node is selected', () => {
    renderCanvas();

    const llmNode = latestReactFlowProps?.nodes?.find(
      (node) => node.id === 'node-llm'
    );
    expect(llmNode?.data?.onSelectNode).toBeTypeOf('function');

    act(() => {
      llmNode?.data?.onSelectNode?.('node-llm');
    });

    expect(latestReactFlowProps?.onPaneClick).toBeUndefined();
  });
});
