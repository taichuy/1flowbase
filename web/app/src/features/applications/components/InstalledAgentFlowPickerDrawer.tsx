import { useQuery } from '@tanstack/react-query';
import { Button, Drawer, Empty, Space, Spin, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import '../../../shared/ui/structured-list/structured-list.css';

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
      size={520}
      onClose={onClose}
      extra={
        <Typography.Link href="/templates">
          {t('auto.manage_agent_flow_templates')}
        </Typography.Link>
      }
    >
      <Spin spinning={installedQuery.isPending}>
        {templates.length > 0 ? (
          <ul className="structured-list__items">
            {templates.map((template) => (
              <li className="structured-list__item" key={template.id}>
                <div className="structured-list__meta">
                  <Typography.Text>{template.artifact_id}</Typography.Text>
                  <Space size="small">
                    <Typography.Text type="secondary">
                      {template.version}
                    </Typography.Text>
                    <Typography.Text type="secondary">
                      {template.organization}
                    </Typography.Text>
                  </Space>
                </div>
                <div className="structured-list__actions">
                  <Button type="link" onClick={() => onSelect(template.id)}>
                    {t('auto.import_template')}
                  </Button>
                </div>
              </li>
            ))}
          </ul>
        ) : (
          <Empty description={t('auto.no_installed_agent_flow_templates')}>
            <Typography.Link href="/settings/extension-center/agent-flow">
              {t('auto.go_to_agent_flow_extension_center')}
            </Typography.Link>
          </Empty>
        )}
      </Spin>
    </Drawer>
  );
}
