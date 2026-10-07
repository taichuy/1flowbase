import { Link } from '@tanstack/react-router';
import { Space, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import { getApplicationsApiBaseUrl } from '../api/applications';

export function ApplicationCollectorPage({
  applicationId
}: {
  applicationId: string;
}) {
  const { t } = useTranslation('applications');
  const endpoint = `${getApplicationsApiBaseUrl()}/api/logs/v1/events`;
  const command = (mode: 'import' | 'watch') =>
    `node scripts/node/agent-logs-collector.js ${mode} --endpoint "${endpoint}" --source "$HOME/.codex/sessions" --state "$HOME/.codex/agent-logs-state.json" --source-id "codex-local"`;
  return (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      <Typography.Title level={4}>{t('agent_logs.collector')}</Typography.Title>
      <Typography.Paragraph>
        {t('agent_logs.collector_description')}
      </Typography.Paragraph>
      <Typography.Paragraph>
        {t('agent_logs.collector_installation')}
      </Typography.Paragraph>
      <Link to="/applications/$applicationId/api" params={{ applicationId }}>
        {t('agent_logs.open_api_keys')}
      </Link>
      <Typography.Paragraph>
        {t('agent_logs.collector_key')}
      </Typography.Paragraph>
      <Typography.Text code copyable>
        {
          'read -rsp "FLOWBASE_AGENT_LOGS_API_KEY: " FLOWBASE_AGENT_LOGS_API_KEY; export FLOWBASE_AGENT_LOGS_API_KEY'
        }
      </Typography.Text>
      <Typography.Title level={5}>
        {t('agent_logs.import_history')}
      </Typography.Title>
      <Typography.Text code copyable>
        {command('import')}
      </Typography.Text>
      <Typography.Title level={5}>{t('agent_logs.watch')}</Typography.Title>
      <Typography.Text code copyable>
        {command('watch')}
      </Typography.Text>
      <Typography.Paragraph>
        {t('agent_logs.collector_paths')}
      </Typography.Paragraph>
    </Space>
  );
}
