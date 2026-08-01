import { Suspense, lazy, type ReactNode } from 'react';

import { useNavigate } from '@tanstack/react-router';

import { LoadingState } from '../../../../shared/ui/loading-state/LoadingState';
import { MemberManagementPanel } from '../../components/MemberManagementPanel';
import {
  RolePermissionPanel,
  type RolePermissionTab
} from '../../components/RolePermissionPanel';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import { SystemRuntimePanel } from '../../components/SystemRuntimePanel';
import type { SettingsSectionKey } from '../../lib/settings-sections';
import type { SettingsExtensionCenterCategory } from '../../api/extensions';
import { SettingsAuthCenterSection } from './SettingsAuthCenterSection';
import { SettingsDataModelsSection } from './SettingsDataModelsSection';
import { SettingsFilesSection } from './SettingsFilesSection';

const ApiDocsPanel = lazy(() =>
  import('../../components/ApiDocsPanel').then((module) => ({
    default: module.ApiDocsPanel
  }))
);
const PersonalAccessTokensPanel = lazy(() =>
  import('../../components/PersonalAccessTokensPanel').then((module) => ({
    default: module.PersonalAccessTokensPanel
  }))
);
const ModelProviderSettingsTabs = lazy(() =>
  import('./ModelProviderSettingsTabs').then((module) => ({
    default: module.ModelProviderSettingsTabs
  }))
);
const SettingsExtensionCenterSection = lazy(() =>
  import('./SettingsExtensionCenterSection').then((module) => ({
    default: module.SettingsExtensionCenterSection
  }))
);
const SettingsMcpManagementSection = lazy(() =>
  import('./SettingsMcpManagementSection').then((module) => ({
    default: module.SettingsMcpManagementSection
  }))
);
const HostInfrastructurePanel = lazy(() =>
  import('../../components/host-infrastructure/HostInfrastructurePanel').then(
    (module) => ({
      default: module.HostInfrastructurePanel
    })
  )
);
const HostInfrastructureMemoryObservationPanel = lazy(() =>
  import('../../components/host-infrastructure/HostInfrastructureMemoryObservationPanel').then(
    (module) => ({
      default: module.HostInfrastructureMemoryObservationPanel
    })
  )
);
const ApplicationManagementPanel = lazy(() =>
  import('../../components/application-management/ApplicationManagementPanel').then(
    (module) => ({
      default: module.ApplicationManagementPanel
    })
  )
);
const I18nCatalogPage = lazy(() =>
  import('../i18n-catalog/I18nCatalogPage').then((module) => ({
    default: module.I18nCatalogPage
  }))
);

function SettingsSectionFallback() {
  return <LoadingState compact />;
}

function SettingsSectionBoundary({ children }: { children: ReactNode }) {
  return <Suspense fallback={<SettingsSectionFallback />}>{children}</Suspense>;
}

interface SettingsSectionAccess {
  isRoot: boolean;
  permissions: string[];
  canManageMembers: boolean;
  canManageRoles: boolean;
  canManageDataModels: boolean;
  canManageModelProviders: boolean;
  canManageHostInfrastructure: boolean;
  canManageMcpManagement: boolean;
}

export function SettingsSectionBody({
  sectionKey,
  access,
  modelProviderTab = 'providers',
  rolePermissionTab = 'console-policy',
  extensionCenterCategory = 'installed',
  extensionCenterCursor
}: {
  sectionKey: SettingsSectionKey;
  access: SettingsSectionAccess;
  modelProviderTab?: 'providers' | 'request-logs';
  rolePermissionTab?: RolePermissionTab;
  extensionCenterCategory?: SettingsExtensionCenterCategory;
  extensionCenterCursor?: string;
}) {
  const navigate = useNavigate();

  switch (sectionKey) {
    case 'applications':
      return (
        <SettingsSectionBoundary>
          <ApplicationManagementPanel />
        </SettingsSectionBoundary>
      );
    case 'extension-center':
      return (
        <SettingsSectionBoundary>
          <SettingsExtensionCenterSection
            category={extensionCenterCategory}
            cursor={extensionCenterCursor}
          />
        </SettingsSectionBoundary>
      );
    case 'members':
      return (
        <MemberManagementPanel
          canManageMembers={access.canManageMembers}
          canManageRoleBindings={access.canManageRoles}
        />
      );
    case 'system-runtime':
      return <SystemRuntimePanel />;
    case 'files':
      return (
        <SettingsFilesSection
          isRoot={access.isRoot}
          permissions={access.permissions}
        />
      );
    case 'model-providers':
      return (
        <SettingsSectionBoundary>
          <ModelProviderSettingsTabs
            activeTab={modelProviderTab}
            canManage={access.canManageModelProviders}
          />
        </SettingsSectionBoundary>
      );
    case 'data-models':
      return (
        <SettingsDataModelsSection canManage={access.canManageDataModels} />
      );
    case 'mcp-management':
      return (
        <SettingsSectionBoundary>
          <SettingsMcpManagementSection
            canManage={access.canManageMcpManagement}
          />
        </SettingsSectionBoundary>
      );
    case 'host-infrastructure':
      return (
        <SettingsSectionBoundary>
          <HostInfrastructurePanel
            canManage={access.canManageHostInfrastructure}
          />
        </SettingsSectionBoundary>
      );
    case 'memory-observation':
      return (
        <SettingsSectionBoundary>
          <SettingsSectionSurface heightMode="fill">
            <HostInfrastructureMemoryObservationPanel
              canManage={access.canManageHostInfrastructure}
            />
          </SettingsSectionSurface>
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
