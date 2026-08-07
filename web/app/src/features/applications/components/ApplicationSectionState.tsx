import { Descriptions, Result, Space, Tag, Typography } from 'antd';
import { useTranslation } from 'react-i18next';

import type { ApplicationDetail } from '../api/applications';
import type { ApplicationSectionKey } from '../lib/application-sections';

function renderStatusTag(status: string) {
  return <Tag color={status === 'planned' ? 'gold' : 'default'}>{status}</Tag>;
}

export function ApplicationSectionState({
  application,
  sectionKey
}: {
  application: ApplicationDetail;
  sectionKey: ApplicationSectionKey;
}) {
  const { t } = useTranslation('applications');

  if (sectionKey === 'orchestration') {
    return (
      <Space orientation="vertical" size="middle">
        <Typography.Title level={4}>{t('auto.orchestration')}</Typography.Title>
        <Typography.Paragraph>
          {t('auto.orchestration_section_description')}</Typography.Paragraph>
        <Descriptions
          bordered
          column={1}
          items={[
            {
              key: 'status',
              label: t('auto.capability_status'),
              children: renderStatusTag(application.sections.orchestration.status)
            },
            {
              key: 'subject_kind',
              label: t('auto.subject_type'),
              children: application.sections.orchestration.subject_kind
            },
            {
              key: 'subject_status',
              label: t('auto.subject_state'),
              children: application.sections.orchestration.subject_status
            },
            {
              key: 'subject_id',
              label: t('auto.current_subject_id'),
              children: application.sections.orchestration.current_subject_id ?? t('auto.not_bound')
            },
            {
              key: 'draft_id',
              label: t('auto.current_draft_id'),
              children: application.sections.orchestration.current_draft_id ?? t('auto.not_generated')
            }
          ]}
        />
      </Space>
    );
  }

  if (sectionKey === 'api') {
    return (
      <Space orientation="vertical" size="middle">
        <Typography.Title level={4}>{t('auto.api')}</Typography.Title>
        <Typography.Paragraph>
          {t('auto.public_api_section_description')}</Typography.Paragraph>
        <Descriptions
          bordered
          column={1}
          items={[
            {
              key: 'status',
              label: t('auto.capability_status'),
              children: renderStatusTag(application.sections.api.status)
            },
            {
              key: 'credential_kind',
              label: t('auto.credential_type'),
              children: application.sections.api.credential_kind
            },
            {
              key: 'routing_mode',
              label: t('auto.routing_mode'),
              children: application.sections.api.invoke_routing_mode
            },
            {
              key: 'path_template',
              label: t('auto.call_path_template'),
              children:
                application.sections.api.invoke_path_template ??
                t('auto.frozen_by_application_type')
            },
            {
              key: 'credentials_status',
              label: t('auto.credential_lifecycle'),
              children: application.sections.api.credentials_status
            }
          ]}
        />
      </Space>
    );
  }

  if (sectionKey === 'monitoring') {
    return (
      <Space orientation="vertical" size="middle">
        <Typography.Title level={4}>{t('auto.monitoring')}</Typography.Title>
        <Typography.Paragraph>
          {t('auto.monitoring_section_description')}</Typography.Paragraph>
        <Descriptions
          bordered
          column={1}
          items={[
            {
              key: 'status',
              label: t('auto.capability_status'),
              children: renderStatusTag(application.sections.monitoring.status)
            },
            {
              key: 'metrics_kind',
              label: t('auto.metric_object'),
              children: application.sections.monitoring.metrics_object_kind
            },
            {
              key: 'metrics_status',
              label: t('auto.metric_aggregation_status'),
              children: application.sections.monitoring.metrics_capability_status
            },
            {
              key: 'tracing_status',
              label: t('auto.tracing_configuration_status'),
              children: application.sections.monitoring.tracing_config_status
            }
          ]}
        />
      </Space>
    );
  }

  return <Result status="info" title={t('auto.section_content_not_found')} />;
}
