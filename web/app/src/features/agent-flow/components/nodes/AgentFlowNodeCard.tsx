import {
  DeleteOutlined,
  MergeOutlined,
  MoreOutlined,
  PlayCircleOutlined,
  SwapOutlined
} from '@ant-design/icons';
import { Button, Dropdown, Tooltip, type MenuProps } from 'antd';
import {
  Position,
  useUpdateNodeInternals,
  type NodeProps
} from '@xyflow/react';
import { useEffect, useRef, useState, type SyntheticEvent } from 'react';

import { SchemaRenderer } from '../../../../shared/schema-ui/v1/runtime/SchemaRenderer';
import { CanvasHandle } from '../canvas/CanvasHandle';
import { ConnectorAddIcon } from '../canvas/ConnectorAddIcon';
import { NodePickerPopover } from '../node-picker/NodePickerPopover';
import type { AgentFlowCanvasNode } from '../canvas/node-types';
import { agentFlowRendererRegistry } from '../../schema/agent-flow-renderer-registry';
import {
  COMPACT_SOURCE_HANDLE_ID,
  MAIN_SOURCE_HANDLE_ID
} from '../../lib/canvas/handle-ids';
import { getNodeDefinitionMeta } from '../../lib/node-definitions';
import { getCommonErrorBranchSourceHandle } from '../../lib/policy/node-error-policy';
import {
  getNodePickerOptionDescription,
  getNodePickerOptionKey
} from '../../lib/plugin-node-definitions';
import { i18nText } from '../../../../shared/i18n/text';

const QUICK_ACTION_HIDE_DELAY_MS = 1000;

type SourceHandle = {
  id: string | null;
  title: string | null;
  disabled?: boolean;
};

function stopActionEvent(event: SyntheticEvent<HTMLElement>) {
  event.stopPropagation();
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function unresolvedNodeInfo(config: Record<string, unknown>) {
  const unresolved = isRecord(config.unresolved) ? config.unresolved : {};
  const reason =
    typeof unresolved.reason === 'string' && unresolved.reason.trim().length > 0
      ? unresolved.reason
      : 'missing_dependency';
  const originalType =
    typeof unresolved.original_type === 'string' &&
    unresolved.original_type.trim().length > 0
      ? unresolved.original_type
      : 'unknown';

  return { reason, originalType };
}

export function AgentFlowNodeCard({
  data,
  selected
}: NodeProps<AgentFlowCanvasNode>) {
  const [quickActionsVisible, setQuickActionsVisible] = useState(false);
  const updateNodeInternals = useUpdateNodeInternals();
  const hideQuickActionsTimerRef = useRef<ReturnType<typeof setTimeout> | null>(
    null
  );
  const clearHideQuickActionsTimer = () => {
    if (hideQuickActionsTimerRef.current === null) {
      return;
    }

    clearTimeout(hideQuickActionsTimerRef.current);
    hideQuickActionsTimerRef.current = null;
  };
  const showQuickActions = () => {
    clearHideQuickActionsTimer();
    setQuickActionsVisible(true);
  };
  const scheduleHideQuickActions = () => {
    clearHideQuickActionsTimer();
    hideQuickActionsTimerRef.current = setTimeout(() => {
      setQuickActionsVisible(false);
      hideQuickActionsTimerRef.current = null;
    }, QUICK_ACTION_HIDE_DELAY_MS);
  };

  useEffect(() => {
    const timerRef = hideQuickActionsTimerRef;

    return () => {
      const timer = timerRef.current;
      if (timer !== null) {
        clearTimeout(timer);
      }
    };
  }, []);
  const nodePickerOptions = data.nodePickerOptions ?? [];
  const isUnresolvedNode = data.nodeType === 'unresolved_node';
  const isCompactResponseNode = data.nodeType === 'compact_response';
  const unresolvedInfo = unresolvedNodeInfo(data.config);
  const replaceItems: MenuProps['items'] = nodePickerOptions.map((option) => ({
    key: getNodePickerOptionKey(option),
    label: getNodePickerOptionDescription(option)
      ? `${option.label} · ${getNodePickerOptionDescription(option)}`
      : option.label,
    disabled: option.kind === 'plugin_contribution' && option.disabled,
    onClick: ({ domEvent }) => {
      domEvent.stopPropagation();
      data.onReplaceNode(data.nodeId, option);
    }
  }));
  const runNode = data.onRunNode;
  const runMenuItems: NonNullable<MenuProps['items']> =
    !isUnresolvedNode && !isCompactResponseNode && runNode
      ? [
          {
            key: 'run',
            icon: <PlayCircleOutlined />,
            label: i18nText('agentFlow', 'auto.execute_this_node'),
            onClick: ({ domEvent }) => {
              domEvent.stopPropagation();
              data.onSelectNode(data.nodeId);
              runNode(data.nodeId);
            }
          }
        ]
      : [];
  const menuItems: MenuProps['items'] = [
    ...runMenuItems,
    ...(isCompactResponseNode
      ? []
      : [
          {
            key: 'replace',
            icon: <SwapOutlined />,
            label: i18nText('agentFlow', 'auto.replace_node'),
            children: replaceItems
          }
        ]),
    {
      key: 'delete',
      icon: <DeleteOutlined />,
      label: i18nText('agentFlow', 'auto.delete_node'),
      danger: true,
      onClick: ({ domEvent }) => {
        domEvent.stopPropagation();
        data.onDeleteNode(data.nodeId);
      }
    }
  ];
  const cardAdapter = {
    getValue(path: string) {
      if (path === 'alias') {
        return data.alias;
      }

      if (path === 'description') {
        return data.description;
      }

      if (path.startsWith('config.')) {
        return data.config[path.slice('config.'.length)];
      }

      return null;
    },
    setValue: () => undefined,
    getDerived(key: string) {
      if (key === 'node') {
        return {
          id: data.nodeId,
          type: data.nodeType,
          alias: data.alias,
          description: data.description,
          config: data.config,
          outputs: []
        };
      }

      if (key === 'typeLabel') {
        return data.typeLabel;
      }

      if (key === 'definitionMeta') {
        return getNodeDefinitionMeta(data.nodeType);
      }

      if (key === 'workflowTriggerContext') {
        return data.workflowTriggerContext ?? null;
      }

      return null;
    },
    dispatch: () => undefined
  } as const;
  const branchSourceHandles = data.branchSourceHandles ?? [];
  const commonErrorBranchSourceHandle = getCommonErrorBranchSourceHandle({
    type: data.nodeType,
    config: data.config
  });
  const branchHandleSignature = [
    ...branchSourceHandles,
    ...(data.compactSourceHandle ? [data.compactSourceHandle] : []),
    ...(commonErrorBranchSourceHandle ? [commonErrorBranchSourceHandle] : [])
  ]
    .map((handle) => handle.id)
    .join('|');
  const toolSourceHandles = data.toolSourceHandles ?? [];
  const toolHandleSignature = toolSourceHandles
    .map((handle) => handle.id)
    .join('|');
  const primarySourceHandles: SourceHandle[] =
    branchSourceHandles.length > 0
      ? branchSourceHandles
      : [
          { id: null, title: null },
          ...(data.compactSourceHandle
            ? [
                {
                  ...data.compactSourceHandle,
                  disabled: data.compactSourceHandleOccupied
                }
              ]
            : [])
        ];
  const sourceHandles = [
    ...primarySourceHandles,
    ...(commonErrorBranchSourceHandle &&
    !primarySourceHandles.some(
      (handle) => handle.id === commonErrorBranchSourceHandle.id
    )
      ? [commonErrorBranchSourceHandle]
      : [])
  ];

  useEffect(() => {
    updateNodeInternals(data.nodeId);
  }, [
    branchHandleSignature,
    data.compactSourceHandleOccupied,
    data.nodeId,
    toolHandleSignature,
    updateNodeInternals
  ]);

  function renderSourceHandle(
    handle: SourceHandle,
    index: number
  ) {
    const sourceHandleId = handle.id ?? MAIN_SOURCE_HANDLE_ID;
    const pickerSourceHandleId = data.pickerSourceHandleId ?? null;
    const pickerOpen = data.pickerOpen && pickerSourceHandleId === handle.id;
    const nodePickerOptions =
      handle.id === COMPACT_SOURCE_HANDLE_ID
        ? data.compactNodePickerOptions
        : data.nodePickerOptions;
    const ariaLabel = handle.title
      ? i18nText('agentFlow', 'auto.add_node_after_branch', {
          value1: data.alias,
          value2: handle.title
        })
      : i18nText('agentFlow', 'auto.add_node_after', { value1: data.alias });
    const top =
      sourceHandles.length > 1
        ? `${((index + 1) / (sourceHandles.length + 1)) * 100}%`
        : undefined;
    const style = top ? { top } : undefined;
    const tooltipTitle = handle.title ? (
      <div style={{ textAlign: 'center', fontSize: 12, padding: '2px 0' }}>
        <div>{handle.title}</div>
        <div>{i18nText('agentFlow', 'auto.click_add_node')}</div>
      </div>
    ) : (
      <div style={{ textAlign: 'center', fontSize: 12, padding: '2px 0' }}>
        <div>{i18nText('agentFlow', 'auto.click_add_node')}</div>
        <div>{i18nText('agentFlow', 'auto.drag_drop_connect_nodes')}</div>
      </div>
    );

    return (
      <div key={handle.id ?? 'default'}>
        <NodePickerPopover
          ariaLabel={ariaLabel}
          open={pickerOpen && !handle.disabled}
          options={nodePickerOptions}
          onOpenChange={(open) => {
            if (handle.disabled) {
              data.onClosePicker();
              return;
            }

            if (open) {
              if (handle.id) {
                data.onOpenPicker(data.nodeId, handle.id);
              } else {
                data.onOpenPicker(data.nodeId);
              }
              return;
            }

            data.onClosePicker();
          }}
          onPickNode={(option) => {
            if (handle.disabled) {
              return;
            }

            if (handle.id) {
              data.onInsertNode(data.nodeId, option, handle.id);
            } else {
              data.onInsertNode(data.nodeId, option);
            }
          }}
        >
          <Tooltip
            title={tooltipTitle}
            placement="top"
            color="#ffffff"
            styles={{
              body: {
                color: '#333',
                borderRadius: 8,
                boxShadow: '0 4px 12px rgba(0,0,0,0.1)'
              }
            }}
            open={!pickerOpen ? undefined : false}
          >
            <CanvasHandle
              id={sourceHandleId}
              type="source"
              position={Position.Right}
              aria-expanded={pickerOpen}
              aria-disabled={handle.disabled}
              aria-haspopup="menu"
              aria-label={ariaLabel}
              className={[
                'agent-flow-node-handle',
                'agent-flow-node-handle--source',
                handle.id && handle.id !== COMPACT_SOURCE_HANDLE_ID
                  ? 'agent-flow-node-handle--branch'
                  : null,
                handle.id === COMPACT_SOURCE_HANDLE_ID
                  ? 'agent-flow-node-handle--compact'
                  : null
              ]
                .filter(Boolean)
                .join(' ')}
              isConnectable={!handle.disabled}
              role="button"
              style={style}
              tabIndex={0}
              onClick={(event) => {
                event.stopPropagation();
              }}
              onKeyDown={(event) => {
                if (event.key !== 'Enter' && event.key !== ' ') {
                  return;
                }

                event.preventDefault();
                event.stopPropagation();

                if (handle.disabled) {
                  return;
                }

                if (pickerOpen) {
                  data.onClosePicker();
                  return;
                }

                if (handle.id) {
                  data.onOpenPicker(data.nodeId, handle.id);
                } else {
                  data.onOpenPicker(data.nodeId);
                }
              }}
            >
              <ConnectorAddIcon className="agent-flow-node-handle__icon" />
            </CanvasHandle>
          </Tooltip>
        </NodePickerPopover>
      </div>
    );
  }

  function getToolConnectorLeft(index: number) {
    return toolSourceHandles.length > 1
      ? `${((index + 1) / (toolSourceHandles.length + 1)) * 100}%`
      : '50%';
  }

  function renderToolHandle(
    handle: { id: string; title: string },
    index: number
  ) {
    return (
      <div
        className="agent-flow-node-card__tool-handle"
        data-testid={`agent-flow-node-tool-handle-${index}`}
        key={handle.id}
        style={{ left: getToolConnectorLeft(index) }}
      >
        <Tooltip
          title={handle.title}
          placement="bottom"
          color="#ffffff"
          styles={{
            body: {
              color: '#333',
              borderRadius: 8,
              boxShadow: '0 4px 12px rgba(0,0,0,0.1)'
            }
          }}
        >
          <CanvasHandle
            id={handle.id}
            type="source"
            position={Position.Bottom}
            isConnectable
            aria-label={`${handle.title} ${i18nText('agentFlow', 'auto.tool_connector')}`}
            className="agent-flow-node-handle agent-flow-node-handle--tool"
          >
            <MergeOutlined
              aria-hidden="true"
              className="agent-flow-node-handle__tool-icon"
              data-testid="agent-flow-node-tool-connector-icon"
            />
          </CanvasHandle>
        </Tooltip>
      </div>
    );
  }

  return (
    <>
      {data.showTargetHandle ? (
        <CanvasHandle
          type="target"
          position={Position.Left}
          className="agent-flow-node-handle agent-flow-node-handle--target"
        />
      ) : null}
      <div
        className={`agent-flow-node-card agent-flow-node-card--theme-unified agent-flow-node-card--type-${data.nodeType}${selected ? ' agent-flow-node-card--selected' : ''}`}
        role="button"
        tabIndex={0}
        onClick={() => data.onSelectNode(data.nodeId)}
        onMouseEnter={showQuickActions}
        onMouseLeave={scheduleHideQuickActions}
        onFocus={showQuickActions}
        onBlur={scheduleHideQuickActions}
        onDoubleClick={() => {
          if (data.canEnterContainer) {
            data.onOpenContainer(data.nodeId);
          }
        }}
        onKeyDown={(event) => {
          if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault();
            data.onSelectNode(data.nodeId);
          }
        }}
      >
        <div
          className="agent-flow-node-card__content"
          data-testid={`agent-flow-node-content-${data.nodeId}`}
        >
          {isUnresolvedNode ? (
            <div className="agent-flow-node-card__unresolved">
              <div className="agent-flow-node-card__unresolved-title">
                {i18nText('agentFlow', 'auto.unresolved_node')}
              </div>
              <div className="agent-flow-node-card__unresolved-alias">
                {data.alias}
              </div>
              <div className="agent-flow-node-card__unresolved-detail">
                {i18nText('agentFlow', 'auto.original_node_type', {
                  value1: unresolvedInfo.originalType
                })}
              </div>
              <div className="agent-flow-node-card__unresolved-detail">
                {i18nText('agentFlow', 'auto.unresolved_node_reason', {
                  value1: unresolvedInfo.reason
                })}
              </div>
            </div>
          ) : (
            <SchemaRenderer
              adapter={cardAdapter}
              blocks={data.nodeSchema.card.blocks}
              registry={agentFlowRendererRegistry}
            />
          )}
        </div>
        <div
          className={`agent-flow-node-card__quick-actions${quickActionsVisible ? ' agent-flow-node-card__quick-actions--visible' : ''}`}
          data-testid={`agent-flow-node-quick-actions-${data.nodeId}`}
          onMouseEnter={showQuickActions}
          onMouseLeave={scheduleHideQuickActions}
          onPointerDown={stopActionEvent}
        >
          {!isUnresolvedNode && !isCompactResponseNode && runNode ? (
            <Tooltip title={i18nText('agentFlow', 'auto.execute_this_node')}>
              <Button
                aria-label={i18nText('agentFlow', 'auto.execute', {
                  value1: data.alias
                })}
                className="agent-flow-node-card__quick-action"
                icon={<PlayCircleOutlined />}
                shape="circle"
                size="small"
                type="text"
                onClick={(event) => {
                  stopActionEvent(event);
                  data.onSelectNode(data.nodeId);
                  runNode(data.nodeId);
                }}
              />
            </Tooltip>
          ) : null}
          <Dropdown menu={{ items: menuItems }} trigger={['click']}>
            <Button
              aria-label={i18nText('agentFlow', 'auto.more_actions', {
                value1: data.alias
              })}
              className="agent-flow-node-card__quick-action"
              icon={<MoreOutlined />}
              shape="circle"
              size="small"
              type="text"
              onClick={stopActionEvent}
            />
          </Dropdown>
        </div>
        {toolSourceHandles.map((handle, index) =>
          renderToolHandle(handle, index)
        )}
      </div>
      {data.showSourceHandle
        ? sourceHandles.map((handle, index) =>
            renderSourceHandle(handle, index)
          )
        : null}
    </>
  );
}
