import { Link } from '@tanstack/react-router';
import { Menu } from 'antd';
import type { MenuProps } from 'antd';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import type { ConsoleNavigation } from '@1flowbase/api-client';

import {
  fetchSettingsConsoleNavigation,
  settingsConsoleNavigationQueryKey
} from '../features/settings/api/console-navigation';
import { getSelectedRouteId } from '../routes/route-config';

interface ConsolePrimaryNavigationRoute {
  id: string;
  path: string;
  label_key: string;
}

function primaryRoutesFromConsoleNavigation(
  navigation: ConsoleNavigation
): ConsolePrimaryNavigationRoute[] {
  const routesById = new Map(
    navigation.route_definitions.map((route) => [route.route_id, route])
  );

  return navigation.navigation_items
    .filter((item) => item.navigation_slot === 'primary')
    .sort((left, right) => left.order - right.order)
    .flatMap((item) => {
      const route = routesById.get(item.route_id);
      if (!route) {
        return [];
      }

      return [
        {
          id: item.item_id,
          path: route.path,
          label_key: item.label_key
        }
      ];
    });
}

function renderNavigationLink(
  pathname: string,
  label: string,
  useRouterLinks: boolean,
  isCurrent: boolean
) {
  if (useRouterLinks) {
    return (
      <Link
        to={pathname}
        className="app-shell-menu-link"
        aria-current={isCurrent ? 'page' : undefined}
      >
        {label}
      </Link>
    );
  }

  return (
    <a
      href={pathname}
      className="app-shell-menu-link"
      aria-current={isCurrent ? 'page' : undefined}
    >
      {label}
    </a>
  );
}

export function Navigation({
  pathname,
  useRouterLinks
}: {
  pathname: string;
  useRouterLinks: boolean;
}) {
  const { t } = useTranslation('appShell');
  const selectedKey = getSelectedRouteId(pathname);
  const consoleNavigationQuery = useQuery({
    queryKey: settingsConsoleNavigationQueryKey,
    queryFn: fetchSettingsConsoleNavigation
  });
  const routes = useMemo<ConsolePrimaryNavigationRoute[]>(() => {
    if (consoleNavigationQuery.data) {
      return primaryRoutesFromConsoleNavigation(consoleNavigationQuery.data);
    }

    return [];
  }, [consoleNavigationQuery.data]);
  const items: MenuProps['items'] =
    consoleNavigationQuery.data === undefined
      ? [
          {
            key: consoleNavigationQuery.isError
              ? 'console-navigation-error'
              : 'console-navigation-loading',
            disabled: true,
            label: consoleNavigationQuery.isError
              ? t('auto.console_navigation_load_failed')
              : t('auto.console_navigation_loading')
          }
        ]
      : routes.map((route) => {
          return {
            key: route.id,
            label: renderNavigationLink(
              route.path,
              t(route.label_key),
              useRouterLinks,
              route.id === selectedKey
            )
          };
        });

  return (
    <nav className="app-shell-navigation" aria-label="Primary">
      <Menu
        className="app-shell-menu"
        mode="horizontal"
        selectedKeys={
          routes.some((route) => route.id === selectedKey) ? [selectedKey] : []
        }
        items={items}
        disabledOverflow
      />
    </nav>
  );
}
