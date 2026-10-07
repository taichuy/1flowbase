import type { ReactNode } from 'react';

import CodeOutlined from '@ant-design/icons/es/icons/CodeOutlined';
import ApiOutlined from '@ant-design/icons/es/icons/ApiOutlined';
import BarChartOutlined from '@ant-design/icons/es/icons/BarChartOutlined';
import DeploymentUnitOutlined from '@ant-design/icons/es/icons/DeploymentUnitOutlined';
import FundOutlined from '@ant-design/icons/es/icons/FundOutlined';
import UnorderedListOutlined from '@ant-design/icons/es/icons/UnorderedListOutlined';
import type { ConsoleApplicationDetail } from '@1flowbase/api-client';

import type { SectionNavItem } from '../../../shared/ui/section-page-layout/SectionPageLayout';

export type ApplicationSectionKey =
  | 'orchestration'
  | 'api'
  | 'logs'
  | 'monitoring'
  | 'statistics'
  | 'collector';

const SECTION_DEFINITIONS: Array<{
  key: ApplicationSectionKey;
  labelKey: string;
  icon: ReactNode;
}> = [
  {
    key: 'orchestration',
    labelKey: 'auto.orchestration',
    icon: <DeploymentUnitOutlined />
  },
  {
    key: 'api',
    labelKey: 'auto.api',
    icon: <ApiOutlined />
  },
  {
    key: 'logs',
    labelKey: 'auto.logs',
    icon: <UnorderedListOutlined />
  },
  {
    key: 'collector',
    labelKey: 'agent_logs.collector',
    icon: <CodeOutlined />
  },
  {
    key: 'monitoring',
    labelKey: 'auto.monitoring',
    icon: <FundOutlined />
  },
  {
    key: 'statistics',
    labelKey: 'auto.statistics',
    icon: <BarChartOutlined />
  }
];

export function getApplicationSections(
  applicationId: string,
  t: (key: string) => string,
  application: Pick<ConsoleApplicationDetail, 'application_type' | 'sections'>
): SectionNavItem[] {
  return SECTION_DEFINITIONS.filter((section) =>
    application.application_type === 'agent_logs'
      ? ['logs', 'api', 'collector', 'statistics'].includes(section.key)
      : section.key !== 'collector' &&
        (section.key !== 'api' ||
          application.sections.api.status !== 'unavailable')
  )
    .sort((left, right) =>
      application.application_type === 'agent_logs'
        ? ['logs', 'api', 'collector', 'statistics'].indexOf(left.key) -
          ['logs', 'api', 'collector', 'statistics'].indexOf(right.key)
        : 0
    )
    .map((section) => ({
      key: section.key,
      label:
        application.application_type === 'workflow' &&
        section.key === 'orchestration'
          ? t('auto.workflow_section')
          : t(section.labelKey),
      icon: section.icon,
      to: `/applications/${applicationId}/${section.key}`
    }));
}

export function getApplicationDefaultSection(
  application_type: ConsoleApplicationDetail['application_type']
): 'logs' | 'orchestration' {
  return application_type === 'agent_logs' ? 'logs' : 'orchestration';
}
