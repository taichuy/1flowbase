import {
  navigationQueryStaleTime,
  selectNavigationQueryScope
} from '../../../state/navigation-query-scope';
import { lazy, Suspense, useMemo } from 'react';

import { useQuery } from '@tanstack/react-query';
import { Navigate } from '@tanstack/react-router';
import { Alert, Result } from 'antd';
import { useTranslation } from 'react-i18next';

import { useAuthStore } from '../../../state/auth-store';
import { PermissionDeniedState } from '../../../shared/ui/PermissionDeniedState';
import {
  fetchSettingsConsoleNavigation,
  settingsConsoleNavigationQueryKey
} from '../api/console-navigation';
import {
  isSettingsSectionKey,
  settingsSectionItemsFromConsoleNavigation
} from '../lib/settings-sections';
import { LoadingState } from '../../../shared/ui/loading-state/LoadingState';
import { SettingsRouteShell } from './settings-page/shell/SettingsRouteShell';
import { SettingsSectionBody } from './settings-page/shell/SettingsSectionBody';
import { useSettingsSections } from './settings-page/shell/use-settings-sections';
import type { RolePermissionTab } from '../components/RolePermissionPanel';
import type { SettingsExtensionCenterCategory } from '../api/extensions/extensions';
import type { NetworkCenterPage } from './network-center/NetworkCenterSection';

const PluginSettingsPage = lazy(() =>
  import('./settings-page/PluginSettingsPage').then(({ PluginSettingsPage }) => ({
    default: PluginSettingsPage
  }))
);

export function SettingsPage({
  requestedSectionKey,
  modelProviderTab,
  rolePermissionTab,
  networkCenterPage,
  extensionCenterCategory,
  extensionCenterCursor,
  extensionCenterQ
}: {
  requestedSectionKey?: string;
  modelProviderTab?: 'providers' | 'pricing' | 'request-logs';
  rolePermissionTab?: RolePermissionTab;
  networkCenterPage?: NetworkCenterPage;
  extensionCenterCategory?: SettingsExtensionCenterCategory;
  extensionCenterCursor?: string;
  extensionCenterQ?: string;
}) {
  const navigationScope = useAuthStore(selectNavigationQueryScope);
  const { t } = useTranslation('settings');
  const actor = useAuthStore((state) => state.actor);
  const me = useAuthStore((state) => state.me);
  const consoleNavigationQuery = useQuery({
    queryKey: [...settingsConsoleNavigationQueryKey, navigationScope],
    staleTime: navigationQueryStaleTime,
    queryFn: fetchSettingsConsoleNavigation
  });
  const permissions = useMemo(() => me?.permissions ?? [], [me?.permissions]);
  const permissionSet = useMemo(() => new Set(permissions), [permissions]);
  const isRoot = actor?.effective_display_role === 'root';
  const canManageMembers = isRoot || permissionSet.has('user.manage.all');
  const canManageRoles =
    isRoot || permissionSet.has('role_permission.manage.all');
  const canManageHostInfrastructure =
    isRoot || permissionSet.has('plugin_config.configure.all');
  const sectionAccess = useMemo(
    () => ({
      isRoot,
      permissions,
      canManageMembers,
      canManageRoles,
      canManageHostInfrastructure
    }),
    [
      canManageHostInfrastructure,
      canManageMembers,
      canManageRoles,
      isRoot,
      permissions
    ]
  );
  const registrySections = useMemo(() => {
    if (consoleNavigationQuery.data) {
      return settingsSectionItemsFromConsoleNavigation(
        consoleNavigationQuery.data
      );
    }

    return [];
  }, [consoleNavigationQuery.data]);
  const registryState =
    consoleNavigationQuery.data === undefined
      ? consoleNavigationQuery.isError
        ? 'error'
        : 'loading'
      : 'ready';
  const { activeSection, redirectSection, visibleSections } =
    useSettingsSections({
      requestedSectionKey,
      sections: registrySections
    });

  const registeredPage = registrySections.find(
    (section) => section.key === activeSection?.key
  );

  if (registryState === 'loading') {
    return (
      <SettingsRouteShell
        visibleSections={[]}
        activeSectionKey=""
        emptyState={<Result status="info" title={t('auto.loading')} />}
      >
        {null}
      </SettingsRouteShell>
    );
  }

  if (registryState === 'error') {
    return (
      <SettingsRouteShell
        visibleSections={[]}
        activeSectionKey=""
        emptyState={
          <Alert
            type="error"
            showIcon
            title={t('auto.settings_navigation_load_failed')}
          />
        }
      >
        {null}
      </SettingsRouteShell>
    );
  }

  if (redirectSection) {
    return <Navigate to={redirectSection.to} replace />;
  }

  if (requestedSectionKey && !activeSection) {
    return (
      <SettingsRouteShell
        visibleSections={visibleSections}
        activeSectionKey=""
        emptyState={<PermissionDeniedState />}
      >
        {null}
      </SettingsRouteShell>
    );
  }

  return (
    <SettingsRouteShell
      visibleSections={visibleSections}
      activeSectionKey={activeSection?.key ?? ''}
    >
      {activeSection && isSettingsSectionKey(activeSection.key) ? (
        <SettingsSectionBody
          sectionKey={activeSection.key}
          access={sectionAccess}
          modelProviderTab={modelProviderTab}
          rolePermissionTab={rolePermissionTab}
          networkCenterPage={networkCenterPage}
          extensionCenterCategory={extensionCenterCategory}
          extensionCenterCursor={extensionCenterCursor}
          extensionCenterQ={extensionCenterQ}
        />
      ) : registeredPage ? (
        <Suspense fallback={<LoadingState compact />}>
          <PluginSettingsPage
            key={registeredPage.route_id}
            route_id={registeredPage.route_id}
          />
        </Suspense>
      ) : null}
    </SettingsRouteShell>
  );
}
