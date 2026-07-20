import type { ReactNode } from 'react';

import {
  ApiOutlined,
  DeploymentUnitOutlined,
  FundOutlined,
  UnorderedListOutlined
} from '@ant-design/icons';
import type { ConsoleApplicationDetail } from '@1flowbase/api-client';

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
  application: Pick<ConsoleApplicationDetail, 'application_type' | 'sections'>
): SectionNavItem[] {
  return SECTION_DEFINITIONS.filter(
    (section) =>
      section.key !== 'api' || application.sections.api.status !== 'unavailable'
  ).map((section) => ({
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
