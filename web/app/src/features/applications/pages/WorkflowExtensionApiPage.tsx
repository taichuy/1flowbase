import { Descriptions, Space, Tag, Typography } from 'antd';
import { useTranslation } from 'react-i18next';

import type { ApplicationDetail } from '../api/applications';

export function WorkflowExtensionApiPage({
  application
}: {
  application: ApplicationDetail;
}) {
  const { t } = useTranslation('applications');
  const capability = application.sections.api;

  return (
    <Space direction="vertical" size="middle">
      <Typography.Title level={4}>
        {t('auto.workflow_extension_api')}
      </Typography.Title>
      <Typography.Paragraph>
        {t('auto.workflow_extension_api_description')}
      </Typography.Paragraph>
      <Descriptions
        bordered
        column={1}
        items={[
          {
            key: 'status',
            label: t('auto.capability_status'),
            children: <Tag>{capability.status}</Tag>
          },
          {
            key: 'credential_kind',
            label: t('auto.access_mode'),
            children: capability.credential_kind
          },
          {
            key: 'routing_mode',
            label: t('auto.operation_mode'),
            children: capability.invoke_routing_mode
          },
          {
            key: 'path_template',
            label: t('auto.call_path_template'),
            children: capability.invoke_path_template
          }
        ]}
      />
    </Space>
  );
}
