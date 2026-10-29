import {
  Handle,
  NodeResizeControl,
  Position,
  type NodeProps
} from '@xyflow/react';
import HomeFilled from '@ant-design/icons/es/icons/HomeFilled';
import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import { Button, theme } from 'antd';
import type { CSSProperties } from 'react';
import type {
  AgentFlowCanvasNode,
  AgentFlowCanvasNodeData
} from '../../canvas/node-types';
import { NodePickerPopover } from '../../node-picker/NodePickerPopover';
import { getAgentFlowNodeTypeIcon } from '../../../lib/node-type-icons';
import { MAIN_SOURCE_HANDLE_ID } from '../../../lib/canvas/handle-ids';
import { i18nText } from '../../../../../shared/i18n/text';
import './container.css';

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
  return (
    <>
      <Handle
        type="source"
        position={Position.Right}
        id={MAIN_SOURCE_HANDLE_ID}
        style={top ? { top } : undefined}
        className="agent-flow-node-handle"
      />
      <div
        className="agent-flow-container__add nodrag nopan"
        style={top ? { top } : undefined}
        onClick={(event) => event.stopPropagation()}
      >
        <NodePickerPopover
          ariaLabel={i18nText(
            'agentFlow',
            'auto.insert_node_connection_position'
          )}
          open={data.pickerOpen}
          options={data.nodePickerOptions}
          onOpenChange={(open) =>
            open ? data.onOpenPicker(data.nodeId) : data.onClosePicker()
          }
          onPickNode={(option) => data.onInsertNode(data.nodeId, option)}
        />
      </div>
    </>
  );
}

export function AgentFlowContainer({
  data,
  selected
}: NodeProps<AgentFlowCanvasNode>) {
  const style = useContainerTheme();
  return (
    <div
      className={`agent-flow-container${selected ? ' agent-flow-container--selected' : ''}`}
      data-container-id={data.nodeId}
      style={style}
    >
      <NodeResizeControl
        position="bottom-right"
        minWidth={data.minimumContainerSize?.width}
        minHeight={data.minimumContainerSize?.height}
        className="agent-flow-container__resize nodrag"
        style={{ visibility: selected ? 'visible' : 'hidden' }}
        onResizeEnd={(_, size) => data.onResizeContainer?.(data.nodeId, size)}
      />
      <Handle
        type="target"
        position={Position.Left}
        style={{ top: 28 }}
        className="agent-flow-node-handle"
      />
      <div
        className="agent-flow-container__header"
        role="button"
        tabIndex={0}
        onClick={(event) => {
          event.stopPropagation();
          data.onSelectNode(data.nodeId);
        }}
        onKeyDown={(event) => {
          if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault();
            data.onSelectNode(data.nodeId);
          }
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
        <Button
          type="text"
          size="small"
          className="nodrag"
          icon={<DeleteOutlined />}
          aria-label={i18nText('agentFlow', 'auto.delete_node')}
          onClick={(event) => {
            event.stopPropagation();
            data.onDeleteNode(data.nodeId);
          }}
        />
      </div>
      <div
        className="agent-flow-container__body"
        onClick={(event) => {
          event.stopPropagation();
          data.onSelectNode(data.nodeId);
        }}
      />
      <Source data={data} top={28} />
    </div>
  );
}

export function ContainerStart({
  data,
  selected
}: NodeProps<AgentFlowCanvasNode>) {
  const style = useContainerTheme();
  return (
    <div
      className={`agent-flow-container-start${selected ? ' agent-flow-container-start--selected' : ''}`}
      role="button"
      style={style}
      tabIndex={0}
      aria-label={data.alias}
      onClick={(event) => {
        event.stopPropagation();
        data.onSelectNode(data.nodeId);
      }}
      onKeyDown={(event) => {
        if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault();
          data.onSelectNode(data.nodeId);
        }
      }}
    >
      <span className="agent-flow-container-start__icon">
        <HomeFilled />
      </span>
      <Source data={data} />
    </div>
  );
}
