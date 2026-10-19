import type { ReactNode } from 'react';

import type { SectionNavItem } from '../../../../../shared/ui/section-page-layout/SectionPageLayout';
import { SectionPageLayout } from '../../../../../shared/ui/section-page-layout/SectionPageLayout';
import { SETTINGS_PAGE_DESCRIPTION } from './settings-page-copy';

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
