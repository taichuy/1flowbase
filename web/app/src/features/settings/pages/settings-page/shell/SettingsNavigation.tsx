import type { ReactNode } from 'react';

import type { SectionNavItem } from '../../../../../shared/ui/section-page-layout/SectionPageLayout';
import { SectionPageLayout } from '../../../../../shared/ui/section-page-layout/SectionPageLayout';
import { i18nText } from '../../../../../shared/i18n/text';

export const SETTINGS_PAGE_TITLE = i18nText("settings", "auto.settings");
export const SETTINGS_PAGE_DESCRIPTION =
  i18nText("settings", "auto.system_management_domain_contains_configurations_related_documents_members_permissions");

export function SettingsNavigation({
  activeKey,
  navItems,
  children,
  heightMode = 'natural'
}: {
  activeKey: string;
  navItems: SectionNavItem[];
  children: ReactNode;
  heightMode?: 'natural' | 'viewport';
}) {
  return (
    <SectionPageLayout
      pageDescription={SETTINGS_PAGE_DESCRIPTION}
      navItems={navItems}
      activeKey={activeKey}
      contentWidth="wide"
      heightMode={heightMode}
      hideCompactNav
    >
      {children}
    </SectionPageLayout>
  );
}
