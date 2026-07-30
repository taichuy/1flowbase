import { useMemo } from 'react';

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
import { SettingsRouteShell } from './settings-page/SettingsRouteShell';
import { SettingsSectionBody } from './settings-page/SettingsSectionBody';
import { useSettingsSections } from './settings-page/use-settings-sections';
import type { RolePermissionTab } from '../components/RolePermissionPanel';

function hasAnyPermission(permissions: string[], candidates: string[]) {
  return candidates.some((permission) => permissions.includes(permission));
}

export function SettingsPage({
  requestedSectionKey,
  modelProviderTab,
  rolePermissionTab
}: {
  requestedSectionKey?: string;
  modelProviderTab?: 'providers' | 'request-logs';
  rolePermissionTab?: RolePermissionTab;
}) {
  const { t } = useTranslation('settings');
  const actor = useAuthStore((state) => state.actor);
  const me = useAuthStore((state) => state.me);
  const consoleNavigationQuery = useQuery({
    queryKey: settingsConsoleNavigationQueryKey,
    queryFn: fetchSettingsConsoleNavigation
  });
  const permissions = useMemo(() => me?.permissions ?? [], [me?.permissions]);
  const permissionSet = useMemo(() => new Set(permissions), [permissions]);
  const isRoot = actor?.effective_display_role === 'root';
  const canManageMembers = isRoot || permissionSet.has('user.manage.all');
  const canManageRoles =
    isRoot || permissionSet.has('role_permission.manage.all');
  const canManageModelProviders =
    isRoot ||
    hasAnyPermission(permissions, [
      'state_model.manage.all',
      'state_model.manage.own'
    ]);
  const canManageDataModels = canManageModelProviders;
  const canManageHostInfrastructure =
    isRoot || permissionSet.has('plugin_config.configure.all');
  const canManageMcpManagement =
    isRoot ||
    permissionSet.has('settings_feature.access.system.mcp-management');
  const sectionAccess = useMemo(
    () => ({
      isRoot,
      permissions,
      canManageMembers,
      canManageRoles,
      canManageDataModels,
      canManageModelProviders,
      canManageHostInfrastructure,
      canManageMcpManagement
    }),
    [
      canManageDataModels,
      canManageHostInfrastructure,
      canManageMcpManagement,
      canManageMembers,
      canManageModelProviders,
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
            message={t('auto.settings_navigation_load_failed')}
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
        />
      ) : null}
    </SettingsRouteShell>
  );
}
