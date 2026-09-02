import CloseOutlined from '@ant-design/icons/es/icons/CloseOutlined';
import CopyOutlined from '@ant-design/icons/es/icons/CopyOutlined';
import QuestionCircleOutlined from '@ant-design/icons/es/icons/QuestionCircleOutlined';
import { App, Button, Tooltip, Typography } from 'antd';
import type { MouseEvent as ReactMouseEvent } from 'react';

import {
  fetchRuntimeDebugArtifact,
  type AgentFlowVariableGroup
} from '../../api/runtime';
import { copyTextToClipboard } from '../../../../shared/ui/clipboard/copy-text';
import { i18nText } from '../../../../shared/i18n/text';
import {
  DebugVariablesPane,
  type SelectedVariableInfo
} from '../debug-console/variables/DebugVariablesPane';

export type { SelectedVariableInfo };

interface AgentFlowVariableCachePanelProps {
  applicationId: string;
  groups: AgentFlowVariableGroup[];
  height: number;
  isResizing: boolean;
  isSidebarResizing: boolean;
  onClose: () => void;
  onReset: () => void;
  onResizeStart: (event: ReactMouseEvent<HTMLDivElement>) => void;
  onSelectedChange: (value: SelectedVariableInfo | null) => void;
  onSelectedValueChange: (key: string, value: unknown) => void;
  onSidebarResizeStart: (event: ReactMouseEvent<HTMLDivElement>) => void;
  rightOffset: number;
  selectedVariable: SelectedVariableInfo | null;
  sidebarMaxWidth: number;
  sidebarMinWidth: number;
  sidebarWidth: number;
}

export function AgentFlowVariableCachePanel({
  applicationId,
  groups,
  height,
  isResizing,
  isSidebarResizing,
  onClose,
  onReset,
  onResizeStart,
  onSelectedChange,
  onSelectedValueChange,
  onSidebarResizeStart,
  rightOffset,
  selectedVariable,
  sidebarMaxWidth,
  sidebarMinWidth,
  sidebarWidth
}: AgentFlowVariableCachePanelProps) {
  const { message } = App.useApp();

  return (
    <section
      aria-label={i18nText('agentFlow', 'auto.variable_cache')}
      className="agent-flow-editor__variable-cache-panel"
      data-resizing={isResizing ? 'true' : 'false'}
      data-sidebar-resizing={isSidebarResizing ? 'true' : 'false'}
      style={{
        right: rightOffset,
        height
      }}
    >
      <div
        aria-label={i18nText('agentFlow', 'auto.adjust_variable_cache_height')}
        aria-orientation="horizontal"
        className="agent-flow-editor__variable-cache-resize-handle"
        onMouseDown={onResizeStart}
        role="separator"
      />
      <header className="agent-flow-editor__variable-cache-header">
        <div className="agent-flow-editor__variable-cache-title-line">
          <Typography.Text strong>
            {i18nText('agentFlow', 'auto.variable_cache')}
          </Typography.Text>
          <Tooltip
            title={i18nText(
              'agentFlow',
              'auto.trial_run_variable_memory_layout_page'
            )}
          >
            <QuestionCircleOutlined
              aria-label={i18nText(
                'agentFlow',
                'auto.variable_cache_description'
              )}
              className="agent-flow-editor__variable-cache-help-icon"
            />
          </Tooltip>
        </div>
        <div className="agent-flow-editor__variable-cache-header-right">
          {selectedVariable ? (
            <div className="agent-flow-editor__variable-cache-header-center">
              <Typography.Text className="agent-flow-editor__variable-cache-header-variable-name">
                {selectedVariable.label}
              </Typography.Text>
              <Button
                aria-label={i18nText('agentFlow', 'auto.copy_variable_value')}
                icon={<CopyOutlined />}
                size="small"
                type="text"
                onClick={() => {
                  const text =
                    typeof selectedVariable.value === 'string'
                      ? selectedVariable.value
                      : JSON.stringify(selectedVariable.value, null, 2);
                  copyTextToClipboard(text).then(
                    () => message.success(i18nText('agentFlow', 'auto.copied')),
                    () =>
                      message.error(i18nText('agentFlow', 'auto.copy_failed'))
                  );
                }}
              >
                {i18nText('agentFlow', 'auto.copy')}
              </Button>
            </div>
          ) : null}
          <Button
            aria-label={i18nText('agentFlow', 'auto.reset_all_variable_caches')}
            size="small"
            type="text"
            onClick={onReset}
          >
            {i18nText('agentFlow', 'auto.reset_all')}
          </Button>
          <Button
            aria-label={i18nText('agentFlow', 'auto.turn_off_variable_caching')}
            icon={<CloseOutlined />}
            type="text"
            onClick={onClose}
          />
        </div>
      </header>
      <div className="agent-flow-editor__variable-cache-body">
        <DebugVariablesPane
          onSelectedValueChange={onSelectedValueChange}
          onLoadFullValue={(artifactRef) =>
            fetchRuntimeDebugArtifact(applicationId, artifactRef)
          }
          groups={groups}
          onSelectedChange={onSelectedChange}
          sidebarWidth={sidebarWidth}
          sidebarMinWidth={sidebarMinWidth}
          sidebarMaxWidth={sidebarMaxWidth}
          onSidebarResizeStart={onSidebarResizeStart}
        />
      </div>
    </section>
  );
}
