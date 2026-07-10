import {
  CodeOutlined,
  HistoryOutlined,
  IssuesCloseOutlined,
  SaveOutlined
} from '@ant-design/icons';
import { Badge, Button, Space, Tag, Tooltip, Typography } from 'antd';
import type { ReactNode } from 'react';

import { i18nText } from '../../../shared/i18n/text';

interface WorkflowOverlayProps {
  applicationName: string;
  autosaveLabel: string;
  autosaveStatus: 'idle' | 'saving' | 'saved' | 'error';
  issueErrorCount: number;
  saveDisabled: boolean;
  saveLoading: boolean;
  testRunAction: ReactNode;
  onOpenEnvironmentVariables: () => void;
  onOpenHistory: () => void;
  onOpenIssues: () => void;
  onSaveDraft: () => void;
}

export function WorkflowOverlay({
  applicationName,
  autosaveLabel,
  autosaveStatus,
  issueErrorCount,
  saveDisabled,
  saveLoading,
  testRunAction,
  onOpenEnvironmentVariables,
  onOpenHistory,
  onOpenIssues,
  onSaveDraft
}: WorkflowOverlayProps) {
  const statusTag = {
    idle: { color: 'default', label: i18nText('agentFlow', 'auto.free') },
    saving: { color: 'blue', label: i18nText('agentFlow', 'auto.saving') },
    saved: { color: 'green', label: i18nText('agentFlow', 'auto.saved') },
    error: { color: 'red', label: i18nText('agentFlow', 'auto.save_failed') }
  }[autosaveStatus];

  return (
    <div
      aria-label={i18nText('workflow', 'auto.workflow_action_bar')}
      className="agent-flow-editor__overlay"
      role="region"
    >
      <Space className="agent-flow-editor__overlay-status" size="small">
        <Typography.Text strong>{applicationName}</Typography.Text>
        <Tag color={statusTag.color} bordered={false}>
          {statusTag.label}
        </Tag>
      </Space>
      <Space size="small">
        {testRunAction}
        <Badge count={issueErrorCount} size="small">
          <Button
            aria-label="Issues"
            icon={<IssuesCloseOutlined />}
            onClick={onOpenIssues}
            title="Issues"
          />
        </Badge>
        <Button
          aria-label={i18nText('agentFlow', 'auto.environment_variables')}
          icon={<CodeOutlined />}
          onClick={onOpenEnvironmentVariables}
          title={i18nText('agentFlow', 'auto.environment_variables')}
        />
        <Tooltip title={autosaveLabel}>
          <Button
            aria-label={i18nText('agentFlow', 'auto.save')}
            disabled={saveDisabled}
            icon={<SaveOutlined />}
            loading={saveLoading}
            onClick={onSaveDraft}
          />
        </Tooltip>
        <Button
          aria-label={i18nText('agentFlow', 'auto.historical_version')}
          icon={<HistoryOutlined />}
          onClick={onOpenHistory}
          title={i18nText('agentFlow', 'auto.historical_version')}
        />
      </Space>
    </div>
  );
}
