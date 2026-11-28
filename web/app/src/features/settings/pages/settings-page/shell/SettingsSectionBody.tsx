import { lazy, Suspense, type ReactNode } from 'react';

import { useNavigate } from '@tanstack/react-router';

import { LoadingState } from '../../../../../shared/ui/loading-state/LoadingState';
import type { RolePermissionTab } from '../../../components/RolePermissionPanel';
import type { SettingsSectionKey } from '../../../lib/settings-sections';
import type { SettingsExtensionCenterCategory } from '../../../api/extensions/extensions';
import type { NetworkCenterPage } from '../../network-center/NetworkCenterSection';

import { settingsSections } from './settings-section-loaders';

const SystemSettingsPanel = lazy(settingsSections.system);
const EventsPage = lazy(settingsSections.events);
const NotificationsPage = lazy(settingsSections.communications);
const MemberSettingsTabs = lazy(settingsSections.members);
const RolePermissionPanel = lazy(settingsSections.roles);
const SystemRuntimePanel = lazy(settingsSections['system-runtime']);
const SettingsAuthCenterSection = lazy(settingsSections['auth-center']);
const SettingsDataModelsSection = lazy(settingsSections['data-models']);
const SettingsFilesSection = lazy(settingsSections.files);
const ApiDocsPanel = lazy(settingsSections.docs);
const PersonalAccessTokensPanel = lazy(settingsSections['api-key-authentication']);
const ModelProviderSettingsTabs = lazy(settingsSections['model-providers']);
const SettingsExtensionCenterSection = lazy(settingsSections['extension-center']);
const SettingsMcpManagementSection = lazy(settingsSections['mcp-management']);
const HostInfrastructureMemoryObservationPanel = lazy(settingsSections['memory-observation']);
const ApplicationManagementPanel = lazy(settingsSections.applications);
const SystemBackupsPanel = lazy(settingsSections.backups);
const I18nCatalogPage = lazy(settingsSections.i18n);
const UiManagementPanel = lazy(settingsSections['ui-management']);
const NetworkCenterSection = lazy(settingsSections['network-center']);

function SettingsSectionFallback() {
  return <LoadingState compact />;
}

function SettingsSectionBoundary({ children }: { children: ReactNode }) {
  return <Suspense fallback={<SettingsSectionFallback />}>{children}</Suspense>;
}

interface SettingsSectionAccess {
  isRoot: boolean;
  canManageMembers: boolean;
  canManageRoles: boolean;
  canManageHostInfrastructure: boolean;
}

export function SettingsSectionBody({
  sectionKey,
  access,
  modelProviderTab = 'providers',
  rolePermissionTab = 'console-policy',
  networkCenterPage = 'proxy-types',
  extensionCenterCategory = 'installed',
  extensionCenterCursor,
  extensionCenterQ
}: {
  sectionKey: SettingsSectionKey;
  access: SettingsSectionAccess;
  modelProviderTab?: 'providers' | 'pricing' | 'request-logs';
  rolePermissionTab?: RolePermissionTab;
  networkCenterPage?: NetworkCenterPage;
  extensionCenterCategory?: SettingsExtensionCenterCategory;
  extensionCenterCursor?: string;
  extensionCenterQ?: string;
}) {
  const navigate = useNavigate();

  switch (sectionKey) {
    case 'system':
      return (
        <SettingsSectionBoundary>
          <SystemSettingsPanel />
        </SettingsSectionBoundary>
      );
    case 'events':
      return (
        <SettingsSectionBoundary>
          <EventsPage />
        </SettingsSectionBoundary>
      );
    case 'communications':
      return (
        <SettingsSectionBoundary>
          <NotificationsPage />
        </SettingsSectionBoundary>
      );
    case 'applications':
      return (
        <SettingsSectionBoundary>
          <ApplicationManagementPanel />
        </SettingsSectionBoundary>
      );
    case 'backups':
      return (
        <SettingsSectionBoundary>
          <SystemBackupsPanel />
        </SettingsSectionBoundary>
      );
    case 'extension-center':
      return (
        <SettingsSectionBoundary>
          <SettingsExtensionCenterSection
            category={extensionCenterCategory}
            cursor={extensionCenterCursor}
            q={extensionCenterQ}
          />
        </SettingsSectionBoundary>
      );
    case 'members':
      return (
        <SettingsSectionBoundary>
          <MemberSettingsTabs canManageMembers={access.canManageMembers} />
        </SettingsSectionBoundary>
      );
    case 'system-runtime':
      return (
        <SettingsSectionBoundary>
          <SystemRuntimePanel />
        </SettingsSectionBoundary>
      );
    case 'files':
      return (
        <SettingsSectionBoundary>
          <SettingsFilesSection
            isRoot={access.isRoot}
          />
        </SettingsSectionBoundary>
      );
    case 'model-providers':
      return (
        <SettingsSectionBoundary>
          <ModelProviderSettingsTabs
            activeTab={modelProviderTab}
          />
        </SettingsSectionBoundary>
      );
    case 'data-models':
      return (
        <SettingsSectionBoundary>
          <SettingsDataModelsSection />
        </SettingsSectionBoundary>
      );
    case 'network-center':
      return (
        <SettingsSectionBoundary>
          <NetworkCenterSection page={networkCenterPage} />
        </SettingsSectionBoundary>
      );
    case 'mcp-management':
      return (
        <SettingsSectionBoundary>
          <SettingsMcpManagementSection />
        </SettingsSectionBoundary>
      );
    case 'ui-management':
      return (
        <SettingsSectionBoundary>
          <UiManagementPanel />
        </SettingsSectionBoundary>
      );
    case 'memory-observation':
      return (
        <SettingsSectionBoundary>
          <HostInfrastructureMemoryObservationPanel
            canManage={access.canManageHostInfrastructure}
          />
        </SettingsSectionBoundary>
      );
    case 'i18n':
      return (
        <SettingsSectionBoundary>
          <I18nCatalogPage />
        </SettingsSectionBoundary>
      );
    case 'roles':
      return (
        <SettingsSectionBoundary>
          <RolePermissionPanel
            canManageRoles={access.canManageRoles}
            activePermissionTab={rolePermissionTab}
            onPermissionTabChange={(tab, navigationMode) =>
              navigate({
                to: `/settings/roles/${tab}`,
                replace: navigationMode === 'replace'
              })
            }
          />
        </SettingsSectionBoundary>
      );
    case 'api-key-authentication':
      return (
        <SettingsSectionBoundary>
          <PersonalAccessTokensPanel />
        </SettingsSectionBoundary>
      );
    case 'auth-center':
      return (
        <SettingsSectionBoundary>
          <SettingsAuthCenterSection />
        </SettingsSectionBoundary>
      );
    case 'docs':
    default:
      return (
        <SettingsSectionBoundary>
          <ApiDocsPanel />
        </SettingsSectionBoundary>
      );
  }
}
