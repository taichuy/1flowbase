import '@xyflow/react/dist/style.css';

import {
  AimOutlined,
  ApartmentOutlined,
  MinusOutlined,
  PlusOutlined
} from '@ant-design/icons';
import { Button } from 'antd';
import {
  Background,
  Panel,
  ReactFlow,
  ReactFlowProvider,
  useOnViewportChange,
  useReactFlow,
  useViewport,
  type Edge
} from '@xyflow/react';
import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';
import { useEffect, useMemo } from 'react';
import { useRef } from 'react';

import { useCanvasInteractions } from '../../hooks/interactions/use-canvas-interactions';
import { useEdgeInteractions } from '../../hooks/interactions/use-edge-interactions';
import { useNodeInteractions } from '../../hooks/interactions/use-node-interactions';
import { useSelectionInteractions } from '../../hooks/interactions/use-selection-interactions';
import { toCanvasEdges } from '../../lib/adapters/to-canvas-edges';
import { toCanvasNodes } from '../../lib/adapters/to-canvas-nodes';
import { BUILTIN_NODE_PICKER_OPTIONS } from '../../lib/plugin-node-definitions';
import { useAgentFlowEditorStore } from '../../store/editor/provider';
import {
  selectActiveContainerId,
  selectSelectedNodeId,
  selectWorkingDocument
} from '../../store/editor/selectors';
import { AgentFlowCustomConnectionLine } from '../canvas/custom-connection-line';
import { NodePickerPopover } from '../node-picker/NodePickerPopover';
import { agentFlowEdgeTypes, agentFlowNodeTypes } from '../canvas/node-types';
import type { NodePickerOption } from '../../lib/plugin-node-definitions';
import type { WorkflowTriggerContext } from '../../lib/workflow-trigger-context';
import { i18nText } from '../../../../shared/i18n/text';

interface AgentFlowCanvasProps {
  issueCountByNodeId: Record<string, number>;
  nodePickerOptions?: NodePickerOption[];
  workflowTriggerContext?: WorkflowTriggerContext | null;
  onRunNode?: (nodeId: string) => void;
  onViewportSnapshotChange?: (
    viewport: FlowAuthoringDocument['editor']['viewport']
  ) => void;
  onViewportGetterReady?: (
    getter: (() => FlowAuthoringDocument['editor']['viewport']) | null
  ) => void;
}

function ZoomToolbar({ onArrangeCanvas }: { onArrangeCanvas: () => void }) {
  const reactFlow = useReactFlow();
  const { zoom } = useViewport();

  return (
    <Panel position="bottom-left" style={{ left: 0, bottom: 0 }}>
      <div className="agent-flow-zoom-toolbar">
        <div
          aria-label={i18nText('agentFlow', 'auto.canvas_zoom_toolbar')}
          className="agent-flow-zoom-toolbar__actions"
          role="toolbar"
        >
          <Button
            aria-label={i18nText('agentFlow', 'auto.reduce_canvas')}
            className="agent-flow-zoom-toolbar__button"
            icon={<MinusOutlined />}
            onClick={() => {
              void reactFlow.zoomOut({ duration: 160 });
            }}
            size="small"
            type="text"
          />
          <Button
            aria-label={i18nText('agentFlow', 'auto.enlarge_canvas')}
            className="agent-flow-zoom-toolbar__button"
            icon={<PlusOutlined />}
            onClick={() => {
              void reactFlow.zoomIn({ duration: 160 });
            }}
            size="small"
            type="text"
          />
          <Button
            aria-label={i18nText('agentFlow', 'auto.adapt_to_canvas')}
            className="agent-flow-zoom-toolbar__button"
            icon={<AimOutlined />}
            onClick={() => {
              void reactFlow.fitView({ duration: 160, padding: 0.16 });
            }}
            size="small"
            type="text"
          />
          <Button
            aria-label={i18nText('agentFlow', 'auto.arrange_canvas')}
            className="agent-flow-zoom-toolbar__button"
            icon={<ApartmentOutlined />}
            onClick={onArrangeCanvas}
            size="small"
            type="text"
          />
        </div>
        <div
          aria-label={i18nText('agentFlow', 'auto.current_zoom')}
          className="agent-flow-zoom-display"
        >
          {Math.round(zoom * 100)}%
        </div>
      </div>
    </Panel>
  );
}

function ViewportObserver({
  onViewportEnd,
  onViewportSnapshotChange,
  onViewportGetterReady
}: {
  onViewportEnd: (
    viewport: FlowAuthoringDocument['editor']['viewport']
  ) => void;
  onViewportSnapshotChange?: (
    viewport: FlowAuthoringDocument['editor']['viewport']
  ) => void;
  onViewportGetterReady?: (
    getter: (() => FlowAuthoringDocument['editor']['viewport']) | null
  ) => void;
}) {
  const reactFlow = useReactFlow();
  const viewport = useViewport();

  useOnViewportChange({
    onEnd: onViewportEnd
  });

  useEffect(() => {
    onViewportGetterReady?.(() => reactFlow.getViewport());

    return () => onViewportGetterReady?.(null);
  }, [onViewportGetterReady, reactFlow]);

  useEffect(() => {
    onViewportSnapshotChange?.({
      x: viewport.x,
      y: viewport.y,
      zoom: viewport.zoom
    });
  }, [onViewportSnapshotChange, viewport.x, viewport.y, viewport.zoom]);

  return null;
}

function PendingLocateNodeEffect() {
  const reactFlow = useReactFlow();
  const pendingLocateNodeId = useAgentFlowEditorStore(
    (state) => state.pendingLocateNodeId
  );
  const setInteractionState = useAgentFlowEditorStore(
    (state) => state.setInteractionState
  );

  useEffect(() => {
    if (!pendingLocateNodeId) {
      return;
    }

    void reactFlow.fitView({
      nodes: [{ id: pendingLocateNodeId }],
      duration: 240,
      padding: 0.24
    });
    setInteractionState({ pendingLocateNodeId: null });
  }, [pendingLocateNodeId, reactFlow, setInteractionState]);

  return null;
}

function AgentFlowCanvasInner({
  issueCountByNodeId,
  nodePickerOptions = BUILTIN_NODE_PICKER_OPTIONS,
  workflowTriggerContext = null,
  onRunNode,
  onViewportSnapshotChange,
  onViewportGetterReady
}: AgentFlowCanvasProps) {
  const canvasRef = useRef<HTMLDivElement>(null);
  const document = useAgentFlowEditorStore(selectWorkingDocument);
  const activeContainerId = useAgentFlowEditorStore(selectActiveContainerId);
  const selectedEdgeId = useAgentFlowEditorStore(
    (state) => state.selectedEdgeId
  );
  const selectedNodeId = useAgentFlowEditorStore(selectSelectedNodeId);
  const nodePickerState = useAgentFlowEditorStore(
    (state) => state.nodePickerState
  );
  const connectingPayload = useAgentFlowEditorStore(
    (state) => state.connectingPayload
  );
  const canvasInteractions = useCanvasInteractions();
  const nodeInteractions = useNodeInteractions();
  const edgeInteractions = useEdgeInteractions();
  const selectionInteractions = useSelectionInteractions();

  const baseNodes = useMemo(
    () =>
      toCanvasNodes(
        document,
        activeContainerId,
        selectedNodeId,
        nodePickerState.open && !nodePickerState.anchorCanvasPosition
          ? nodePickerState.anchorNodeId
          : null,
        connectingPayload.sourceHandleId,
        issueCountByNodeId,
        {
          onOpenPicker: nodeInteractions.openNodePicker,
          onClosePicker: nodeInteractions.closeNodePicker,
          onOpenContainer: nodeInteractions.openContainer,
          onSelectNode: nodeInteractions.selectNode,
          onInsertNode: nodeInteractions.insertAfterNode,
          onRunNode: onRunNode ?? (() => undefined),
          onReplaceNode: nodeInteractions.replaceNode,
          onDeleteNode: nodeInteractions.deleteNode,
          nodePickerOptions,
          workflowTriggerContext
        }
      ),
    [
      activeContainerId,
      document,
      issueCountByNodeId,
      nodeInteractions,
      nodePickerOptions,
      onRunNode,
      workflowTriggerContext,
      nodePickerState.anchorCanvasPosition,
      nodePickerState.anchorNodeId,
      nodePickerState.open,
      connectingPayload.sourceHandleId,
      selectedNodeId
    ]
  );
  const nodes = useMemo(() => {
    const transientNodePositions = canvasInteractions.transientNodePositions;

    if (Object.keys(transientNodePositions).length === 0) {
      return baseNodes;
    }

    return baseNodes.map((node) =>
      transientNodePositions[node.id]
        ? {
            ...node,
            position: transientNodePositions[node.id]
          }
        : node
    );
  }, [baseNodes, canvasInteractions.transientNodePositions]);
  const edges = useMemo(
    () =>
      toCanvasEdges(document, activeContainerId, selectedEdgeId, {
        nodePickerOptions,
        onInsertNode: edgeInteractions.insertOnEdge
      }),
    [
      activeContainerId,
      document,
      edgeInteractions.insertOnEdge,
      nodePickerOptions,
      selectedEdgeId
    ]
  );

  return (
    <div className="agent-flow-canvas" ref={canvasRef}>
      <ReactFlow
        edges={edges}
        nodes={nodes}
        defaultViewport={document.editor.viewport}
        nodeTypes={agentFlowNodeTypes}
        edgeTypes={agentFlowEdgeTypes}
        connectionLineComponent={AgentFlowCustomConnectionLine}
        nodesDraggable
        onConnect={edgeInteractions.connect}
        onConnectStart={(_, payload) => {
          edgeInteractions.startConnection(payload);
        }}
        onConnectEnd={(event) => {
          if (!event) {
            edgeInteractions.cancelConnection();
            return;
          }

          const target = event.target instanceof Element ? event.target : null;

          if (
            target?.closest('.react-flow__node') ||
            target?.closest('.react-flow__handle')
          ) {
            edgeInteractions.cancelConnection();
            return;
          }

          const bounds = canvasRef.current?.getBoundingClientRect();
          const clientPosition =
            'changedTouches' in event && event.changedTouches.length > 0
              ? {
                  x: event.changedTouches[0].clientX,
                  y: event.changedTouches[0].clientY
                }
              : {
                  x: 'clientX' in event ? event.clientX : 0,
                  y: 'clientY' in event ? event.clientY : 0
                };

          edgeInteractions.finishConnectionOnPane({
            x: clientPosition.x - (bounds?.left ?? 0),
            y: clientPosition.y - (bounds?.top ?? 0)
          });
        }}
        onNodesChange={canvasInteractions.onNodesChange}
        onReconnect={(oldEdge: Edge, connection) => {
          edgeInteractions.reconnect(oldEdge.id, connection);
        }}
        onEdgeClick={(_, edge) => {
          selectionInteractions.selectEdge(edge.id);
        }}
        isValidConnection={edgeInteractions.isValidConnection}
      >
        <Background gap={20} size={1} />
        <PendingLocateNodeEffect />
        <ViewportObserver
          onViewportEnd={canvasInteractions.commitViewportChange}
          onViewportSnapshotChange={onViewportSnapshotChange}
          onViewportGetterReady={onViewportGetterReady}
        />
        <ZoomToolbar onArrangeCanvas={canvasInteractions.arrangeCanvas} />
      </ReactFlow>
      {nodePickerState.open &&
      nodePickerState.anchorNodeId &&
      nodePickerState.anchorCanvasPosition ? (
        <div
          className="agent-flow-floating-picker-anchor"
          style={{
            left: Number.isFinite(nodePickerState.anchorCanvasPosition.x)
              ? nodePickerState.anchorCanvasPosition.x
              : 0,
            top: Number.isFinite(nodePickerState.anchorCanvasPosition.y)
              ? nodePickerState.anchorCanvasPosition.y
              : 0
          }}
        >
          <NodePickerPopover
            ariaLabel={i18nText(
              'agentFlow',
              'auto.insert_node_connection_position'
            )}
            buttonClassName="agent-flow-floating-picker-anchor__button"
            open
            options={nodePickerOptions}
            placement="bottom"
            onOpenChange={(open) => {
              if (!open) {
                nodeInteractions.closeNodePicker();
              }
            }}
            onPickNode={(option) => {
              edgeInteractions.insertFromConnection(option);
            }}
          />
        </div>
      ) : null}
    </div>
  );
}

export function AgentFlowCanvas(props: AgentFlowCanvasProps) {
  return (
    <ReactFlowProvider>
      <AgentFlowCanvasInner {...props} />
    </ReactFlowProvider>
  );
}
