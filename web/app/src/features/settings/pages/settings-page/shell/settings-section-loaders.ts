import { lazy } from 'react';

import type { SettingsSectionKey } from '../../../lib/settings-sections';

export const settingsSections = {
  system: () =>
    import('../../system-settings/SystemSettingsPanel').then(
      ({ SystemSettingsPanel }) => ({ default: SystemSettingsPanel })
    ),
  events: () =>
    import('../../events/EventsPage').then(({ EventsPage }) => ({
      default: EventsPage
    })),
  communications: () =>
    import('../../notifications/NotificationsPage').then(
      ({ NotificationsPage }) => ({ default: NotificationsPage })
    ),
  members: () =>
    import('../MemberSettingsTabs').then(({ MemberSettingsTabs }) => ({
      default: MemberSettingsTabs
    })),
  roles: () =>
    import('../../../components/RolePermissionPanel').then(
      ({ RolePermissionPanel }) => ({ default: RolePermissionPanel })
    ),
  'system-runtime': () =>
    import('../../../components/SystemRuntimePanel').then(
      ({ SystemRuntimePanel }) => ({ default: SystemRuntimePanel })
    ),
  'auth-center': () =>
    import('../SettingsAuthCenterSection').then(
      ({ SettingsAuthCenterSection }) => ({
        default: SettingsAuthCenterSection
      })
    ),
  'data-models': () =>
    import('../SettingsDataModelsSection').then(
      ({ SettingsDataModelsSection }) => ({
        default: SettingsDataModelsSection
      })
    ),
  files: () =>
    import('../SettingsFilesSection').then(({ SettingsFilesSection }) => ({
      default: SettingsFilesSection
    })),
  docs: () =>
    import('../../../components/ApiDocsPanel').then(({ ApiDocsPanel }) => ({
      default: ApiDocsPanel
    })),
  'api-key-authentication': () =>
    import('../../../components/PersonalAccessTokensPanel').then(
      ({ PersonalAccessTokensPanel }) => ({
        default: PersonalAccessTokensPanel
      })
    ),
  'model-providers': () =>
    import('../ModelProviderSettingsTabs').then(
      ({ ModelProviderSettingsTabs }) => ({
        default: ModelProviderSettingsTabs
      })
    ),
  'extension-center': () =>
    import('../SettingsExtensionCenterSection').then(
      ({ SettingsExtensionCenterSection }) => ({
        default: SettingsExtensionCenterSection
      })
    ),
  'mcp-management': () =>
    import('../SettingsMcpManagementSection').then(
      ({ SettingsMcpManagementSection }) => ({
        default: SettingsMcpManagementSection
      })
    ),
  'memory-observation': () =>
    import('../../../components/host-infrastructure/HostInfrastructureMemoryObservationPanel').then(
      ({ HostInfrastructureMemoryObservationPanel }) => ({
        default: HostInfrastructureMemoryObservationPanel
      })
    ),
  applications: () =>
    import('../../../components/application-management/ApplicationManagementPanel').then(
      ({ ApplicationManagementPanel }) => ({
        default: ApplicationManagementPanel
      })
    ),
  backups: () =>
    import('../../../components/system-backups/SystemBackupsPanel').then(
      ({ SystemBackupsPanel }) => ({ default: SystemBackupsPanel })
    ),
  i18n: () =>
    import('../../i18n-catalog/I18nCatalogPage').then(
      ({ I18nCatalogPage }) => ({ default: I18nCatalogPage })
    ),
  'ui-management': () =>
    import('../../../components/ui-management/UiManagementPanel').then(
      ({ UiManagementPanel }) => ({ default: UiManagementPanel })
    ),
  'network-center': () =>
    import('../../network-center/NetworkCenterSection').then(
      ({ NetworkCenterSection }) => ({ default: NetworkCenterSection })
    )
};

const loadSettingsPage = () =>
  import('../../SettingsPage').then(({ SettingsPage }) => ({
    default: SettingsPage
  }));

export const SettingsRoutePage = lazy(loadSettingsPage);

export async function preloadSettingsRouteByPathname(pathname: string) {
  const [, surface, key] = pathname.split('/');
  if (surface !== 'settings') return;

  await Promise.all([
    loadSettingsPage(),
    key && Object.hasOwn(settingsSections, key)
      ? settingsSections[key as SettingsSectionKey]()
      : undefined
  ]);
}
