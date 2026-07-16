import {
  CodeOutlined,
  DownOutlined,
  HistoryOutlined,
  IssuesCloseOutlined,
  SaveOutlined
} from '@ant-design/icons';
import {
  Badge,
  Button,
  Dropdown,
  Space,
  Tag,
  Tooltip,
  Typography
} from 'antd';
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
  published: boolean;
  publishDisabled: boolean;
  publishLoading: boolean;
  revertLoading: boolean;
  onOpenEnvironmentVariables: () => void;
  onOpenHistory: () => void;
  onOpenIssues: () => void;
  onSaveDraft: () => void;
  onPublish: () => void;
  onRevertToDraft: () => void;
}

export function WorkflowOverlay({
  applicationName,
  autosaveLabel,
  autosaveStatus,
  issueErrorCount,
  saveDisabled,
  saveLoading,
  testRunAction,
  published,
  publishDisabled,
  publishLoading,
  revertLoading,
  onOpenEnvironmentVariables,
  onOpenHistory,
  onOpenIssues,
  onSaveDraft,
  onPublish,
  onRevertToDraft
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
        <Space.Compact>
          <Button
            autoInsertSpace={false}
            type="primary"
            disabled={publishDisabled}
            loading={publishLoading}
            onClick={onPublish}
          >
            {i18nText('agentFlow', 'auto.publish')}
          </Button>
          <Dropdown
            trigger={['click']}
            menu={{
              items: [
                {
                  key: 'revert_to_draft',
                  label: i18nText('applications', 'auto.revert_to_draft'),
                  disabled: !published,
                  onClick: onRevertToDraft
                }
              ]
            }}
          >
            <Button
              aria-label={i18nText('workflow', 'auto.more_publish_actions')}
              autoInsertSpace={false}
              type="primary"
              icon={<DownOutlined />}
              loading={revertLoading}
            />
          </Dropdown>
        </Space.Compact>
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
