import { Link } from '@tanstack/react-router';
import { Menu } from 'antd';
import type { MenuProps } from 'antd';
import type { ItemType } from 'antd/es/menu/interface';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import type { ConsoleNavigation } from '@1flowbase/api-client';

import {
  fetchSettingsConsoleNavigation,
  settingsConsoleNavigationQueryKey
} from '../features/settings/api/console-navigation';
import { getSelectedRouteId } from '../routes/route-config';
import {
  fetchFrontstagePageTree,
  frontstagePageTreeQueryKey,
  type FrontstagePageTreeNode
} from '../features/frontstage/api/page-tree';
import { useAuthStore } from '../state/auth-store';

interface ConsolePrimaryNavigationRoute {
  id: string;
  path: string;
  label_key: string;
}

function topbarPageRoutes(nodes: FrontstagePageTreeNode[]): ConsolePrimaryNavigationRoute[] {
  return nodes.flatMap((node) => {
    if (node.placement !== 'topbar') {
      return [];
    }

    const descendants = topbarPageRoutes(node.children);
    if (node.kind !== 'page') {
      return descendants;
    }

    return [
      {
        id: node.id,
        path: `/frontstage/pages/${node.id}`,
        label_key: node.title?.trim() || '未命名页面'
      },
      ...descendants
    ];
  });
}

function topbarNavigationItems({
  nodes,
  pathname,
  useRouterLinks
}: {
  nodes: FrontstagePageTreeNode[];
  pathname: string;
  useRouterLinks: boolean;
}): ItemType[] {
  return nodes.reduce<ItemType[]>((items, node) => {
    if (node.placement !== 'topbar') {
      return items;
    }

    const label = node.title?.trim() || '未命名页面';
    if (node.kind === 'group') {
      const children = topbarNavigationItems({
        nodes: node.children,
        pathname,
        useRouterLinks
      });
      if (children.length > 0) {
        items.push({ key: node.id, label, children });
      }
      return items;
    }

    const path = `/frontstage/pages/${node.id}`;
    items.push({
      key: node.id,
      label: renderNavigationLink(
        path,
        label,
        useRouterLinks,
        pathname === path || pathname.startsWith(`${path}/`)
      )
    });
    return items;
  }, []);
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
  const workspaceId = useAuthStore((state) => state.actor?.current_workspace_id);
  const selectedKey = getSelectedRouteId(pathname);
  const consoleNavigationQuery = useQuery({
    queryKey: settingsConsoleNavigationQueryKey,
    queryFn: fetchSettingsConsoleNavigation
  });
  const frontstageNavigationQuery = useQuery({
    queryKey: frontstagePageTreeQueryKey(workspaceId ?? ''),
    queryFn: () => fetchFrontstagePageTree(workspaceId ?? ''),
    enabled: Boolean(workspaceId),
    retry: false
  });
  const routes = useMemo<ConsolePrimaryNavigationRoute[]>(() => {
    if (consoleNavigationQuery.data) {
      return [
        ...primaryRoutesFromConsoleNavigation(consoleNavigationQuery.data),
        ...topbarPageRoutes(frontstageNavigationQuery.data ?? [])
      ];
    }

    return [];
  }, [consoleNavigationQuery.data, frontstageNavigationQuery.data]);
  const hasSelectedDynamicPage = routes.some(
    (candidate) =>
      candidate.path.startsWith('/frontstage/pages/') &&
      (pathname === candidate.path || pathname.startsWith(`${candidate.path}/`))
  );
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
      : [
          ...primaryRoutesFromConsoleNavigation(consoleNavigationQuery.data).map(
            (route) => ({
              key: route.id,
              label: renderNavigationLink(
                route.path,
                t(route.label_key),
                useRouterLinks,
                route.id === selectedKey && !hasSelectedDynamicPage
              )
            })
          ),
          ...topbarNavigationItems({
            nodes: frontstageNavigationQuery.data ?? [],
            pathname,
            useRouterLinks
          })
        ];

  return (
    <nav className="app-shell-navigation" aria-label="Primary">
      <Menu
        className="app-shell-menu"
        mode="horizontal"
        selectedKeys={
          routes.some(
            (route) =>
              route.id === selectedKey ||
              (route.path.startsWith('/frontstage/pages/') &&
                (pathname === route.path || pathname.startsWith(`${route.path}/`)))
          )
            ? [
                routes.find(
                  (route) =>
                    route.path.startsWith('/frontstage/pages/') &&
                    (pathname === route.path || pathname.startsWith(`${route.path}/`))
                )?.id ?? selectedKey
              ]
            : []
        }
        items={items}
        disabledOverflow
      />
    </nav>
  );
}
