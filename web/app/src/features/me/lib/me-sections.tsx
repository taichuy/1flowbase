import KeyOutlined from '@ant-design/icons/es/icons/KeyOutlined';
import UserOutlined from '@ant-design/icons/es/icons/UserOutlined';

import type { SectionNavItem } from '../../../shared/ui/section-page-layout/SectionPageLayout';
import { i18nText } from '../../../shared/i18n/text';

export type MeSectionKey = 'profile' | 'security';

const ME_SECTIONS: SectionNavItem[] = [
  {
    key: 'profile',
    label: i18nText('me', 'profile.title'),
    to: '/me/profile',
    icon: <UserOutlined />
  },
  {
    key: 'security',
    label: i18nText('me', 'auto.security_settings'),
    to: '/me/security',
    icon: <KeyOutlined />
  }
];

export function getMeSections(): SectionNavItem[] {
  return ME_SECTIONS;
}
