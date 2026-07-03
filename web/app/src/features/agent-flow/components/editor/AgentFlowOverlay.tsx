import {
  CodeOutlined,
  ExportOutlined,
  GlobalOutlined,
  HistoryOutlined,
  IssuesCloseOutlined,
  MessageOutlined,
  PlayCircleOutlined,
  SaveOutlined
} from '@ant-design/icons';
import { Badge, Button, Space, Tag, Tooltip, Typography } from 'antd';
import { i18nText } from '../../../../shared/i18n/text';

interface AgentFlowOverlayProps {
  applicationName: string;
  autosaveLabel: string;
  autosaveStatus: 'idle' | 'saving' | 'saved' | 'error';
  onSaveDraft: () => void;
  saveDisabled: boolean;
  saveLoading: boolean;
  onOpenDebugConsole: () => void;
  onExportTemplate: () => void;
  onOpenIssues: () => void;
  onOpenHistory: () => void;
  onOpenConversationVariables: () => void;
  onOpenEnvironmentVariables: () => void;
  onOpenSystemVariables: () => void;
  onOpenPublish: () => void;
  issueErrorCount: number;
  exportLoading: boolean;
  publishDisabled: boolean;
  showConversationDebug?: boolean;
  showConversationVariables?: boolean;
  showSystemVariables?: boolean;
}

export function AgentFlowOverlay({
  applicationName,
  autosaveLabel,
  autosaveStatus,
  onSaveDraft,
  saveDisabled,
  saveLoading,
  onOpenDebugConsole,
  onExportTemplate,
  onOpenIssues,
  onOpenHistory,
  onOpenConversationVariables,
  onOpenEnvironmentVariables,
  onOpenSystemVariables,
  onOpenPublish,
  issueErrorCount,
  exportLoading,
  publishDisabled,
  showConversationDebug = true,
  showConversationVariables = true,
  showSystemVariables = true
}: AgentFlowOverlayProps) {
  const statusTag = {
    idle: { color: 'default', label: i18nText('agentFlow', 'auto.free') },
    saving: { color: 'blue', label: i18nText('agentFlow', 'auto.saving') },
    saved: { color: 'green', label: i18nText('agentFlow', 'auto.saved') },
    error: { color: 'red', label: i18nText('agentFlow', 'auto.save_failed') }
  }[autosaveStatus];

  return (
    <div
      aria-label={i18nText('agentFlow', 'auto.agent_flow_action_bar')}
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
        {showConversationDebug ? (
          <Button
            aria-label={i18nText('agentFlow', 'auto.preview')}
            autoInsertSpace={false}
            icon={<PlayCircleOutlined />}
            onClick={onOpenDebugConsole}
            title={i18nText('agentFlow', 'auto.preview')}
          >
            {i18nText('agentFlow', 'auto.preview')}
          </Button>
        ) : null}
        <Badge count={issueErrorCount} size="small">
          <Button
            aria-label="Issues"
            icon={<IssuesCloseOutlined />}
            onClick={onOpenIssues}
            title="Issues"
          />
        </Badge>
        <Button
          aria-label={i18nText('agentFlow', 'auto.export_template')}
          autoInsertSpace={false}
          icon={<ExportOutlined />}
          loading={exportLoading}
          onClick={onExportTemplate}
          title={i18nText('agentFlow', 'auto.export_template')}
        />
        {showSystemVariables ? (
          <Button
            aria-label={i18nText('agentFlow', 'auto.system_variables')}
            autoInsertSpace={false}
            icon={<GlobalOutlined />}
            onClick={onOpenSystemVariables}
            title={i18nText('agentFlow', 'auto.system_variables')}
          />
        ) : null}
        <Button
          aria-label={i18nText('agentFlow', 'auto.environment_variables')}
          autoInsertSpace={false}
          icon={<CodeOutlined />}
          onClick={onOpenEnvironmentVariables}
          title={i18nText('agentFlow', 'auto.environment_variables')}
        />
        {showConversationVariables ? (
          <Button
            aria-label={i18nText('agentFlow', 'auto.conversation_variables')}
            autoInsertSpace={false}
            icon={<MessageOutlined />}
            onClick={onOpenConversationVariables}
            title={i18nText('agentFlow', 'auto.conversation_variables')}
          />
        ) : null}
        <Tooltip title={autosaveLabel}>
          <Button
            aria-label={i18nText('agentFlow', 'auto.save')}
            autoInsertSpace={false}
            disabled={saveDisabled}
            icon={<SaveOutlined />}
            loading={saveLoading}
            onClick={onSaveDraft}
          />
        </Tooltip>
        <Button
          autoInsertSpace={false}
          type="primary"
          disabled={publishDisabled}
          onClick={onOpenPublish}
        >
          {i18nText('agentFlow', 'auto.publish')}
        </Button>
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
