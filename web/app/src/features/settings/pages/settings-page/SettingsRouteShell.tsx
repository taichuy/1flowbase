import type { ReactNode } from 'react';

import { SectionPageLayout } from '../../../../shared/ui/section-page-layout/SectionPageLayout';
import type { SettingsSectionNavItem } from '../../lib/settings-sections';
import { SettingsEmptyState } from './SettingsEmptyState';
import {
  SETTINGS_PAGE_DESCRIPTION,
  SETTINGS_PAGE_TITLE,
  SettingsNavigation
} from './SettingsNavigation';

export function SettingsRouteShell({
  visibleSections,
  activeSectionKey,
  emptyState,
  children
}: {
  visibleSections: SettingsSectionNavItem[];
  activeSectionKey: string;
  emptyState?: ReactNode;
  children: ReactNode;
}) {
  if (visibleSections.length === 0) {
    return (
      <SectionPageLayout
        pageTitle={SETTINGS_PAGE_TITLE}
        pageDescription={SETTINGS_PAGE_DESCRIPTION}
        navItems={[]}
        activeKey=""
        contentWidth="wide"
        emptyState={emptyState ?? <SettingsEmptyState />}
      >
        {null}
      </SectionPageLayout>
    );
  }

  return (
    <SettingsNavigation
      activeKey={activeSectionKey}
      navItems={visibleSections}
      heightMode="viewport"
    >
      {children}
    </SettingsNavigation>
  );
}
