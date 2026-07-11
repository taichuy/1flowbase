import { Suspense, lazy, type ReactNode } from 'react';

import { LoadingState } from '../../../../shared/ui/loading-state/LoadingState';
import { MemberManagementPanel } from '../../components/MemberManagementPanel';
import { RolePermissionPanel } from '../../components/RolePermissionPanel';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import { SystemRuntimePanel } from '../../components/SystemRuntimePanel';
import type { SettingsSectionKey } from '../../lib/settings-sections';
import { SettingsAuthCenterSection } from './SettingsAuthCenterSection';
import { SettingsDataModelsSection } from './SettingsDataModelsSection';
import { SettingsFilesSection } from './SettingsFilesSection';
import { i18nText } from '../../../../shared/i18n/text';

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
  modelProviderTab = 'providers'
}: {
  sectionKey: SettingsSectionKey;
  access: SettingsSectionAccess;
  modelProviderTab?: 'providers' | 'request-logs';
}) {
  switch (sectionKey) {
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
          <SettingsSectionSurface
            title={i18nText('settings', 'auto.memory_observation')}
            hideHeader
            heightMode="fill"
          >
            <HostInfrastructureMemoryObservationPanel
              canManage={access.canManageHostInfrastructure}
            />
          </SettingsSectionSurface>
        </SettingsSectionBoundary>
      );
    case 'roles':
      return <RolePermissionPanel canManageRoles={access.canManageRoles} />;
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
