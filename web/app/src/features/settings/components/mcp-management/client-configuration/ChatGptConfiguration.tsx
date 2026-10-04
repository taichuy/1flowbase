import { useQuery } from '@tanstack/react-query';
import { Alert, Button, Descriptions, Space, Spin, Typography } from 'antd';

import { fetchMcpOAuthConfiguration } from '../../../api/mcp-oauth';
import { i18nText } from '../../../../../shared/i18n/text';

export function ChatGptConfiguration({ instanceId }: { instanceId: string }) {
  const config = useQuery({
    queryKey: ['settings', 'mcp-management', 'oauth-configuration', instanceId],
    queryFn: () => fetchMcpOAuthConfiguration(instanceId),
    retry: false
  });

  if (config.isPending) return <Spin />;
  if (config.isError) {
    return (
      <Alert
        type="error"
        showIcon
        title={i18nText('settingsMcpManagement', 'chatgpt.load_error')}
        action={
          <Button onClick={() => void config.refetch()}>
            {i18nText('settingsMcpManagement', 'chatgpt.retry')}
          </Button>
        }
      />
    );
  }
  if (!config.data.enabled || !config.data.server_url) {
    return (
      <Alert
        type="warning"
        showIcon
        title={i18nText('settingsMcpManagement', 'chatgpt.not_enabled')}
        description={i18nText(
          'settingsMcpManagement',
          'chatgpt.not_enabled_help'
        )}
      />
    );
  }
  return (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      <Alert
        type="success"
        showIcon
        title={i18nText('settingsMcpManagement', 'chatgpt.ready')}
      />
      <Descriptions
        column={1}
        size="small"
        items={[
          {
            key: 'url',
            label: i18nText('settingsMcpManagement', 'chatgpt.server_url'),
            children: (
              <Typography.Text copyable style={{ overflowWrap: 'anywhere' }}>
                {config.data.server_url}
              </Typography.Text>
            )
          },
          {
            key: 'auth',
            label: i18nText('settingsMcpManagement', 'chatgpt.authentication'),
            children: 'OAuth'
          },
          {
            key: 'registration',
            label: i18nText('settingsMcpManagement', 'chatgpt.registration'),
            children: i18nText(
              'settingsMcpManagement',
              'chatgpt.dynamic_registration'
            )
          }
        ]}
      />
      <ol style={{ paddingInlineStart: 20, margin: 0 }}>
        <li>{i18nText('settingsMcpManagement', 'chatgpt.step_create')}</li>
        <li>{i18nText('settingsMcpManagement', 'chatgpt.step_auth')}</li>
        <li>{i18nText('settingsMcpManagement', 'chatgpt.step_key')}</li>
        <li>{i18nText('settingsMcpManagement', 'chatgpt.step_consent')}</li>
      </ol>
      <Typography.Paragraph type="secondary" style={{ margin: 0 }}>
        {i18nText('settingsMcpManagement', 'chatgpt.lifecycle')}
      </Typography.Paragraph>
    </Space>
  );
}
