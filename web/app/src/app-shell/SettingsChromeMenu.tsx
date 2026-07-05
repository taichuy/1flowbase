import { Menu } from 'antd';
import { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';

import {
  fetchSettingsConsoleNavigation,
  settingsConsoleNavigationQueryKey
} from '../features/settings/api/console-navigation';
import { settingsSectionItemsFromConsoleNavigation } from '../features/settings/lib/settings-sections';
import { createSettingsChromeMenuItems } from './settings-chrome-menu-items';

export function SettingsChromeMenu({
  pathname,
  useRouterLinks
}: {
  pathname: string;
  useRouterLinks: boolean;
}) {
  const consoleNavigationQuery = useQuery({
    queryKey: settingsConsoleNavigationQueryKey,
    queryFn: fetchSettingsConsoleNavigation
  });
  const sections = useMemo(() => {
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

  return (
    <Menu
      className="app-shell-settings-menu"
      mode="horizontal"
      selectable={false}
      selectedKeys={
        pathname === '/settings' || pathname.startsWith('/settings/')
          ? ['settings']
          : []
      }
      items={createSettingsChromeMenuItems({
        pathname,
        useRouterLinks,
        sections,
        registryState
      })}
      disabledOverflow
    />
  );
}
