import type { ReactNode } from 'react';

import {
  ApiOutlined,
  DeploymentUnitOutlined,
  FundOutlined,
  UnorderedListOutlined
} from '@ant-design/icons';
import type { ConsoleApplicationType } from '@1flowbase/api-client';

import type { SectionNavItem } from '../../../shared/ui/section-page-layout/SectionPageLayout';

export type ApplicationSectionKey = 'orchestration' | 'api' | 'logs' | 'monitoring';

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
    key: 'monitoring',
    labelKey: 'auto.monitoring',
    icon: <FundOutlined />
  }
];

export function getApplicationSections(
  applicationId: string,
  t: (key: string) => string,
  applicationType: ConsoleApplicationType = 'agent_flow'
): SectionNavItem[] {
  return SECTION_DEFINITIONS.filter(
    (section) => applicationType !== 'workflow' || section.key !== 'api'
  ).map((section) => ({
    key: section.key,
    label:
      applicationType === 'workflow' && section.key === 'orchestration'
        ? t('auto.workflow_section')
        : t(section.labelKey),
    icon: section.icon,
    to: `/applications/${applicationId}/${section.key}`
  }));
}
