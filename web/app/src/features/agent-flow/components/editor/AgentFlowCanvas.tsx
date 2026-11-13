import { useConsoleOperationAccess } from '../../../../state/console-operation-access';
import { containerBodyPickerOptions } from '../../lib/node-definitions/nodes/containers/body-policy';
import '@xyflow/react/dist/style.css';

import AimOutlined from '@ant-design/icons/es/icons/AimOutlined';
import ApartmentOutlined from '@ant-design/icons/es/icons/ApartmentOutlined';
import MinusOutlined from '@ant-design/icons/es/icons/MinusOutlined';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
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
import { memo, useEffect, useLayoutEffect, useMemo } from 'react';
import { useRef } from 'react';

import { useCanvasInteractions } from '../../hooks/interactions/use-canvas-interactions';
import { useEdgeInteractions } from '../../hooks/interactions/use-edge-interactions';
import { useNodeInteractions } from '../../hooks/interactions/use-node-interactions';
import { useSelectionInteractions } from '../../hooks/interactions/use-selection-interactions';
import { toCanvasEdges } from '../../lib/adapters/to-canvas-edges';
import { toCanvasNodes } from '../../lib/adapters/to-canvas-nodes';
import {
  syncCanvasEdges,
  syncCanvasNodes
} from '../../lib/adapters/sync-canvas-elements';
import { useAgentFlowEditorStore } from '../../store/editor/provider';
import {
  selectActiveContainerId,
  selectSelectedNodeId,
  selectWorkingDocument
} from '../../store/editor/selectors';
import { AgentFlowCustomConnectionLine } from '../canvas/custom-connection-line';
import { NodePickerPopover } from '../node-picker/NodePickerPopover';
import {
  agentFlowEdgeTypes,
  agentFlowNodeTypes,
  type AgentFlowCanvasEdge,
  type AgentFlowCanvasNode
} from '../canvas/node-types';
import type { NodePickerOption } from '../../lib/plugin-node-definitions';
import { i18nText } from '../../../../shared/i18n/text';

interface AgentFlowCanvasProps {
  issueCountByNodeId: Record<string, number>;
  interactionLocked?: boolean;
  nodeRunStatuses?: Record<string, 'running' | 'succeeded' | 'failed'>;
  workflowTriggerContext?: AgentFlowCanvasNode['data']['workflowTriggerContext'];
  nodePickerOptions?: NodePickerOption[];
  onRunNode?: (nodeId: string) => void;
  onViewportSnapshotChange?: (
    viewport: FlowAuthoringDocument['editor']['viewport']
  ) => void;
  onViewportGetterReady?: (
    getter: (() => FlowAuthoringDocument['editor']['viewport']) | null
  ) => void;
}

const EMPTY_NODE_PICKER_OPTIONS: NodePickerOption[] = [];

function ZoomToolbar({
  onArrangeCanvas,
  locked,
  readOnly
}: {
  onArrangeCanvas: () => void;
  locked: boolean;
  readOnly: boolean;
}) {
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
            disabled={locked}
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
            disabled={locked}
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
            disabled={locked}
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
            disabled={readOnly}
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
  interactionLocked = false,
  nodeRunStatuses,
  workflowTriggerContext,
  nodePickerOptions = EMPTY_NODE_PICKER_OPTIONS,
  onRunNode,
  onViewportSnapshotChange,
  onViewportGetterReady
}: AgentFlowCanvasProps) {
  const can = useConsoleOperationAccess();
  const readOnly = interactionLocked || !can('save_draft');
  const canvasRef = useRef<HTMLDivElement>(null);
  const connectionGestureRef = useRef(false);
  const reactFlow = useReactFlow<AgentFlowCanvasNode, AgentFlowCanvasEdge>();
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
  const actionsRef = useRef({ nodeInteractions, edgeInteractions, onRunNode });
  useLayoutEffect(() => {
    actionsRef.current = { nodeInteractions, edgeInteractions, onRunNode };
  });
  // Canvas objects retain their callbacks while actions read the latest draft.
  const actions = useMemo(
    () => ({
      onOpenPicker: (nodeId: string, handleId?: string | null) =>
        actionsRef.current.nodeInteractions.openNodePicker(nodeId, handleId),
      onClosePicker: () =>
        actionsRef.current.nodeInteractions.closeNodePicker(),
      onOpenContainer: (nodeId: string) =>
        actionsRef.current.nodeInteractions.openContainer(nodeId),
      onSelectNode: (nodeId: string) =>
        actionsRef.current.nodeInteractions.selectNode(nodeId),
      onInsertNode: (
        nodeId: string,
        option: NodePickerOption,
        handleId?: string | null
      ) =>
        actionsRef.current.nodeInteractions.insertAfterNode(
          nodeId,
          option,
          handleId
        ),
      onReplaceNode: (nodeId: string, option: NodePickerOption) =>
        actionsRef.current.nodeInteractions.replaceNode(nodeId, option),
      onDeleteNode: (nodeId: string) =>
        actionsRef.current.nodeInteractions.deleteNode(nodeId),
      onRunNode: (nodeId: string) => actionsRef.current.onRunNode?.(nodeId),
      onInsertOnEdge: (edgeId: string, option: NodePickerOption) =>
        actionsRef.current.edgeInteractions.insertOnEdge(edgeId, option),
      ignore: () => {}
    }),
    []
  );
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
          readOnly,
          onOpenPicker: readOnly ? actions.ignore : actions.onOpenPicker,
          onClosePicker: actions.onClosePicker,
          onOpenContainer: actions.onOpenContainer,
          onSelectNode: actions.onSelectNode,
          onInsertNode: readOnly ? actions.ignore : actions.onInsertNode,
          onRunNode:
            !interactionLocked && onRunNode ? actions.onRunNode : undefined,
          workflowTriggerContext,
          onReplaceNode: readOnly ? actions.ignore : actions.onReplaceNode,
          onDeleteNode: readOnly ? actions.ignore : actions.onDeleteNode,
          onResizeContainer: readOnly
            ? undefined
            : canvasInteractions.commitContainerResize,
          nodePickerOptions
        }
      ).map((node) =>
        nodeRunStatuses
          ? {
              ...node,
              data: { ...node.data, runStatus: nodeRunStatuses[node.id] }
            }
          : node
      ),
    [
      interactionLocked,
      nodeRunStatuses,
      readOnly,
      activeContainerId,
      canvasInteractions.commitContainerResize,
      document,
      issueCountByNodeId,
      actions,
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
  const edges = useMemo(
    () =>
      toCanvasEdges(document, activeContainerId, selectedEdgeId, {
        nodePickerOptions,
        onInsertNode: readOnly ? undefined : actions.onInsertOnEdge
      }).map((edge) => {
        const running =
          nodeRunStatuses?.[edge.target] === 'running' &&
          nodeRunStatuses?.[edge.source] === 'succeeded';
        return nodeRunStatuses
          ? {
              ...edge,
              animated: running,
              style: {
                ...edge.style,
                ...(running ? { stroke: 'var(--status-running)' } : {})
              }
            }
          : edge;
      }),
    [
      nodeRunStatuses,
      readOnly,
      activeContainerId,
      document,
      actions,
      nodePickerOptions,
      selectedEdgeId
    ]
  );

  useEffect(() => {
    reactFlow.setNodes((current) => syncCanvasNodes(current, baseNodes));
  }, [baseNodes, reactFlow]);

  useEffect(() => {
    reactFlow.setEdges((current) => syncCanvasEdges(current, edges));
  }, [edges, reactFlow]);

  return (
    <div
      className="agent-flow-canvas"
      data-interaction-locked={interactionLocked}
      inert={interactionLocked || undefined}
      ref={canvasRef}
      onPointerDownCapture={() => {
        connectionGestureRef.current = false;
      }}
    >
      <ReactFlow
        defaultEdges={edges}
        defaultNodes={baseNodes}
        defaultViewport={document.editor.viewport}
        nodeTypes={agentFlowNodeTypes}
        edgeTypes={agentFlowEdgeTypes}
        connectionLineComponent={AgentFlowCustomConnectionLine}
        deleteKeyCode={null}
        multiSelectionKeyCode={null}
        selectionOnDrag={false}
        selectionKeyCode={null}
        onPaneClick={() => {
          // Mouseup after a connection drag can also produce a pane click.
          // Keep its picker open; the next pointer gesture clears this flag.
          if (connectionGestureRef.current) return;
          selectionInteractions.clearSelection();
        }}
        onNodeClick={(_, node) => nodeInteractions.selectNode(node.id)}
        panOnDrag={!interactionLocked}
        panOnScroll={false}
        zoomOnScroll={!interactionLocked}
        zoomOnPinch={!interactionLocked}
        zoomOnDoubleClick={!interactionLocked}
        autoPanOnNodeDrag={!interactionLocked}
        autoPanOnConnect={!interactionLocked}
        nodesFocusable={!interactionLocked}
        edgesFocusable={!interactionLocked}
        elementsSelectable={!interactionLocked}
        nodesDraggable={!readOnly}
        nodesConnectable={!readOnly}
        edgesReconnectable={!readOnly}
        onConnect={readOnly ? undefined : edgeInteractions.connect}
        onConnectStart={(_, payload) => {
          if (readOnly) return;
          connectionGestureRef.current = true;
          edgeInteractions.startConnection(payload);
        }}
        onConnectEnd={(event) => {
          if (readOnly) return;
          if (!event) {
            edgeInteractions.cancelConnection();
            return;
          }

          const target = event.target instanceof Element ? event.target : null;

          if (
            (target?.closest('.react-flow__node') &&
              !target.closest('.agent-flow-container__body')) ||
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
        onNodesChange={readOnly ? undefined : canvasInteractions.onNodesChange}
        onReconnect={(oldEdge: Edge, connection) => {
          if (readOnly) return;
          edgeInteractions.reconnect(oldEdge.id, connection);
        }}
        onEdgeClick={(_, edge) => {
          selectionInteractions.selectEdge(edge.id);
        }}
        isValidConnection={edgeInteractions.isValidConnection}
        onInit={() => {
          requestAnimationFrame(() => {
            if (
              performance.getEntriesByName('1flowbase:CanvasVisible').length ===
              0
            ) {
              performance.mark('1flowbase:CanvasVisible');
            }
            requestAnimationFrame(() => {
              if (
                performance.getEntriesByName('1flowbase:CanvasInteractive')
                  .length === 0
              ) {
                performance.mark('1flowbase:CanvasInteractive');
              }
            });
          });
        }}
      >
        <Background gap={20} size={1} />
        {!interactionLocked ? <PendingLocateNodeEffect /> : null}
        <ViewportObserver
          onViewportEnd={
            readOnly ? () => {} : canvasInteractions.commitViewportChange
          }
          onViewportSnapshotChange={onViewportSnapshotChange}
          onViewportGetterReady={onViewportGetterReady}
        />
        <ZoomToolbar
          locked={interactionLocked}
          readOnly={readOnly}
          onArrangeCanvas={canvasInteractions.arrangeCanvas}
        />
      </ReactFlow>
      {!readOnly &&
      nodePickerState.open &&
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
            options={containerBodyPickerOptions(
              nodePickerOptions,
              document.graph.nodes.find(
                (node) => node.id === connectingPayload.sourceNodeId
              )?.containerId ?? null
            )}
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

function AgentFlowCanvasComponent(props: AgentFlowCanvasProps) {
  return (
    <ReactFlowProvider>
      <AgentFlowCanvasInner {...props} />
    </ReactFlowProvider>
  );
}

export const AgentFlowCanvas = memo(AgentFlowCanvasComponent);
