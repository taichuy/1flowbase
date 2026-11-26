import { useAuthStore } from '../state/auth-store';
import {
  navigationQueryStaleTime,
  selectNavigationQueryScope
} from '../state/navigation-query-scope';
import { Menu } from 'antd';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';

import {
  fetchSettingsConsoleNavigation,
  settingsConsoleNavigationQueryKey
} from '../features/settings/api/console-navigation';
import { settingsSectionItemsFromConsoleNavigation } from '../features/settings/lib/settings-sections';
import { appI18n, applicationI18nLoader } from '../shared/i18n/app-i18n';
import { resolveAppLocale } from '../shared/i18n/locales';
import { createSettingsChromeMenuItems } from './settings-chrome-menu-items';

export function SettingsChromeMenu({
  pathname,
  useRouterLinks
}: {
  pathname: string;
  useRouterLinks: boolean;
}) {
  const navigationScope = useAuthStore(selectNavigationQueryScope);
  const [, setSettingsNamespaceTick] = useState(0);
  const ensureSettingsNamespace = useCallback(() => {
    const locale = resolveAppLocale(appI18n.resolvedLanguage ?? appI18n.language);
    if (applicationI18nLoader.readyPrimary(locale, ['settings'])) return;
    void applicationI18nLoader
      .activatePrimary(locale, ['settings'])
      .then(() => {
        setSettingsNamespaceTick((current) => current + 1);
      })
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    if (import.meta.env.VITEST) return undefined;
    const timer = setTimeout(ensureSettingsNamespace, 2_500);
    return () => clearTimeout(timer);
  }, [ensureSettingsNamespace]);

  const consoleNavigationQuery = useQuery({
    queryKey: [...settingsConsoleNavigationQueryKey, navigationScope],
    staleTime: navigationQueryStaleTime,
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
    <span
      onPointerEnter={ensureSettingsNamespace}
      onFocus={ensureSettingsNamespace}
    >
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
    </span>
  );
}
