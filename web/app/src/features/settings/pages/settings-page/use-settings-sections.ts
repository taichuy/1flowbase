import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import {
  type SettingsSectionNavItem,
  type SettingsSectionRegistryItem
} from '../../lib/settings-sections';

export function useSettingsSections({
  requestedSectionKey,
  sections
}: {
  requestedSectionKey?: string;
  sections: SettingsSectionRegistryItem[];
}) {
  const { t } = useTranslation('settings');
  const visibleSections = useMemo<SettingsSectionNavItem[]>(
    () =>
      sections.map(({ key, label_key, to }) => ({
        key,
        label: t(label_key),
        to
      })),
    [sections, t]
  );
  const fallbackSection = visibleSections[0] ?? null;
  const activeSection = requestedSectionKey
    ? (visibleSections.find((section) => section.key === requestedSectionKey) ??
      null)
    : null;
  const redirectSection =
    !requestedSectionKey || !activeSection ? fallbackSection : null;

  return {
    activeSection,
    fallbackSection,
    redirectSection,
    visibleSections
  };
}
