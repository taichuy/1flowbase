import { useQuery } from '@tanstack/react-query';
import { Button, Drawer, Empty, List, Space, Typography } from 'antd';
import { useTranslation } from 'react-i18next';

import {
  fetchInstalledAgentFlows,
  installedAgentFlowsQueryKey
} from '../api/applications';

export function InstalledAgentFlowPickerDrawer({
  open,
  onClose,
  onSelect
}: {
  open: boolean;
  onClose: () => void;
  onSelect: (installationId: string) => void;
}) {
  const { t } = useTranslation('applications');
  const installedQuery = useQuery({
    queryKey: installedAgentFlowsQueryKey,
    queryFn: fetchInstalledAgentFlows,
    enabled: open,
    retry: false
  });
  const templates =
    installedQuery.data?.entries.filter(
      (entry) =>
        entry.status === 'installed' &&
        entry.application_action === 'import_agent_flow'
    ) ?? [];

  return (
    <Drawer
      open={open}
      title={t('auto.select_installed_agent_flow')}
      width={520}
      onClose={onClose}
      extra={
        <Typography.Link href="/templates">
          {t('auto.manage_agent_flow_templates')}
        </Typography.Link>
      }
    >
      <List
        loading={installedQuery.isPending}
        dataSource={templates}
        locale={{
          emptyText: (
            <Empty description={t('auto.no_installed_agent_flow_templates')}>
              <Typography.Link href="/settings/extension-center/agent-flow">
                {t('auto.go_to_agent_flow_extension_center')}
              </Typography.Link>
            </Empty>
          )
        }}
        renderItem={(template) => (
          <List.Item
            actions={[
              <Button
                key="import"
                type="link"
                onClick={() => onSelect(template.id)}
              >
                {t('auto.import_template')}
              </Button>
            ]}
          >
            <List.Item.Meta
              title={template.artifact_id}
              description={
                <Space size="small">
                  <Typography.Text type="secondary">
                    {template.version}
                  </Typography.Text>
                  <Typography.Text type="secondary">
                    {template.organization}
                  </Typography.Text>
                </Space>
              }
            />
          </List.Item>
        )}
      />
    </Drawer>
  );
}
