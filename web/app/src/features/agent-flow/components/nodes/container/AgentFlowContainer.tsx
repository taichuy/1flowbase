import {
  Handle,
  NodeResizeControl,
  Position,
  useUpdateNodeInternals,
  type NodeProps
} from '@xyflow/react';
import HomeFilled from '@ant-design/icons/es/icons/HomeFilled';
import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import { Button, theme } from 'antd';
import { useEffect, type CSSProperties } from 'react';
import type {
  AgentFlowCanvasNode,
  AgentFlowCanvasNodeData
} from '../../canvas/node-types';
import { NodePickerPopover } from '../../node-picker/NodePickerPopover';
import { ConnectorAddIcon } from '../../canvas/ConnectorAddIcon';
import { getAgentFlowNodeTypeIcon } from '../../../lib/node-type-icons';
import { MAIN_SOURCE_HANDLE_ID } from '../../../lib/canvas/handle-ids';
import { i18nText } from '../../../../../shared/i18n/text';
import './container.css';
import { NodeRunStatus } from '../NodeRunStatus';

function useContainerTheme(): CSSProperties {
  const { token } = theme.useToken();
  return {
    '--container-accent': token.cyan6,
    '--container-selection': token.blue6,
    '--container-surface': token.colorBgContainer,
    '--container-body': token.colorFillQuaternary,
    '--container-dots': token.colorBorderSecondary
  } as CSSProperties;
}

function Source({
  data,
  top
}: {
  data: AgentFlowCanvasNodeData;
  top?: number;
}) {
  if (data.readOnly) {
    return (
      <Handle
        type="source"
        position={Position.Right}
        id={MAIN_SOURCE_HANDLE_ID}
        isConnectable={false}
        style={top ? { top } : undefined}
        className="agent-flow-node-handle agent-flow-node-handle--source"
      />
    );
  }
  return (
    <NodePickerPopover
      ariaLabel={i18nText('agentFlow', 'auto.add_node_after', {
        value1: data.alias
      })}
      open={data.pickerOpen}
      options={data.nodePickerOptions}
      onOpenChange={(open) =>
        open ? data.onOpenPicker(data.nodeId) : data.onClosePicker()
      }
      onPickNode={(option) => data.onInsertNode(data.nodeId, option)}
    >
      <Handle
        type="source"
        position={Position.Right}
        id={MAIN_SOURCE_HANDLE_ID}
        style={top ? { top } : undefined}
        className="agent-flow-node-handle agent-flow-node-handle--source nodrag nopan"
        role="button"
        tabIndex={0}
        aria-label={i18nText('agentFlow', 'auto.add_node_after', {
          value1: data.alias
        })}
        aria-expanded={data.pickerOpen}
        aria-haspopup="menu"
        onClick={(event) => event.stopPropagation()}
        onKeyDown={(event) => {
          if (event.key !== 'Enter' && event.key !== ' ') return;
          event.preventDefault();
          event.stopPropagation();
          if (data.pickerOpen) data.onClosePicker();
          else data.onOpenPicker(data.nodeId);
        }}
      >
        <ConnectorAddIcon className="agent-flow-node-handle__icon" />
      </Handle>
    </NodePickerPopover>
  );
}

export function AgentFlowContainer({
  data,
  selected
}: NodeProps<AgentFlowCanvasNode>) {
  const style = useContainerTheme();
  return (
    <div
      className={`agent-flow-run-node agent-flow-container${selected ? ' agent-flow-container--selected' : ''}`}
      data-run-status={data.runStatus}
      data-container-id={data.nodeId}
      style={style}
    >
      <NodeRunStatus status={data.runStatus} />
      <NodeResizeControl
        position="bottom-right"
        minWidth={data.minimumContainerSize?.width}
        minHeight={data.minimumContainerSize?.height}
        className="agent-flow-container__resize nodrag"
        style={{
          visibility: selected && !data.readOnly ? 'visible' : 'hidden',
          width: 12,
          height: 12,
          border: '2px solid var(--container-selection)',
          borderRadius: 3,
          background: 'var(--container-surface)'
        }}
        onResizeEnd={(_, size) => data.onResizeContainer?.(data.nodeId, size)}
      />
      <Handle
        type="target"
        isConnectable={!data.readOnly}
        position={Position.Left}
        style={{ top: 28 }}
        className="agent-flow-node-handle"
      />
      <div className="agent-flow-container__header">
        <button
          type="button"
          className="agent-flow-container__select"
          onClick={(event) => {
            event.stopPropagation();
            data.onSelectNode(data.nodeId);
          }}
        >
          <span className="agent-flow-container__icon">
            {getAgentFlowNodeTypeIcon(data.nodeType)}
          </span>
          <span className="agent-flow-container__title">{data.alias}</span>
          {data.issueCount > 0 ? (
            <span className="agent-flow-container__issues">
              {data.issueCount}
            </span>
          ) : null}
        </button>
        {!data.runStatus ? (
          <Button
            type="text"
            disabled={data.readOnly}
            size="small"
            className="nodrag"
            icon={<DeleteOutlined />}
            aria-label={i18nText('agentFlow', 'auto.delete_node')}
            onClick={(event) => {
              event.stopPropagation();
              data.onDeleteNode(data.nodeId);
            }}
          />
        ) : null}
      </div>
      <div className="agent-flow-container__body" />
      <Source data={data} top={28} />
    </div>
  );
}

export function ContainerStart({
  data,
  selected
}: NodeProps<AgentFlowCanvasNode>) {
  const style = useContainerTheme();
  const updateNodeInternals = useUpdateNodeInternals();
  useEffect(() => {
    updateNodeInternals(data.nodeId);
  }, [data.nodeId, data.emptyContainer, updateNodeInternals]);
  return (
    <div
      className={`agent-flow-run-node agent-flow-container-start${data.emptyContainer ? ' agent-flow-container-start--empty' : ''}${selected ? ' agent-flow-container-start--selected' : ''}`}
      style={style}
      data-run-status={data.runStatus}
    >
      <NodeRunStatus status={data.runStatus} />
      <button
        type="button"
        className="agent-flow-container-start__select"
        aria-label={data.alias}
        onClick={(event) => {
          event.stopPropagation();
          data.onSelectNode(data.nodeId);
        }}
      >
        <span className="agent-flow-container-start__icon">
          <HomeFilled />
        </span>
      </button>
      {data.emptyContainer ? (
        <Source data={data} />
      ) : (
        <Handle
          type="source"
          position={Position.Right}
          id={MAIN_SOURCE_HANDLE_ID}
          isConnectable={!data.readOnly}
          className="agent-flow-container-start__anchor nodrag nopan"
        />
      )}
    </div>
  );
}
