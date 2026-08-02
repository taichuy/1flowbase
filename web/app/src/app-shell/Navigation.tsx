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
import { APP_ROUTES, getSelectedRouteId } from '../routes/route-config';
import {
  fetchFrontstagePageTree,
  frontstagePageTreeQueryKey,
  type FrontstagePageTreeNode
} from '../features/frontstage/api/page-tree';
import { useAuthStore } from '../state/auth-store';
import { useFrontstageDesignModeStore } from '../state/frontstage-design-mode-store';
import {
  TopbarNavigationDesigner,
  TopbarNavigationItemLabel
} from './TopbarNavigationDesigner';

interface ConsolePrimaryNavigationRoute {
  id: string;
  path: string;
  label_key: string;
}

const primaryRoutePathsById = new Map<string, string>(
  APP_ROUTES.filter((route) => route.chromeSlot === 'primary').map((route) => [
    route.id,
    route.path
  ])
);

function topbarPageRoutes(
  nodes: FrontstagePageTreeNode[]
): ConsolePrimaryNavigationRoute[] {
  return nodes.flatMap((node) => {
    if (node.placement !== 'topbar' || !node.slug) {
      return [];
    }
    return [
      {
        id: node.id,
        path: `/${node.slug}`,
        label_key: node.title?.trim() || '未命名页面'
      }
    ];
  });
}

function topbarNavigationItems({
  nodes,
  pathname,
  useRouterLinks,
  workspaceId,
  isDesignMode
}: {
  nodes: FrontstagePageTreeNode[];
  pathname: string;
  useRouterLinks: boolean;
  workspaceId?: string;
  isDesignMode: boolean;
}): ItemType[] {
  return nodes.reduce<ItemType[]>((items, node) => {
    if (node.placement !== 'topbar' || !node.slug) {
      return items;
    }
    const path = `/${node.slug}`;
    items.push({
      key: node.id,
      label:
        isDesignMode && workspaceId ? (
          <TopbarNavigationItemLabel
            workspaceId={workspaceId}
            node={node}
            siblings={nodes.filter(
              (candidate) => candidate.placement === 'topbar'
            )}
          >
            {renderNavigationLink(
              path,
              node.title?.trim() || '未命名页面',
              useRouterLinks,
              pathname === path || pathname.startsWith(`${path}/`)
            )}
          </TopbarNavigationItemLabel>
        ) : (
          renderNavigationLink(
            path,
            node.title?.trim() || '未命名页面',
            useRouterLinks,
            pathname === path || pathname.startsWith(`${path}/`)
          )
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
      // Only render backend-authorized routes that this frontend build can resolve.
      if (!route || primaryRoutePathsById.get(route.route_id) !== route.path) {
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
  const workspaceId = useAuthStore(
    (state) => state.actor?.current_workspace_id
  );
  const isDesignMode = useFrontstageDesignModeStore(
    (state) => state.isDesignMode
  );
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
  const dynamicRoutes = useMemo(
    () => topbarPageRoutes(frontstageNavigationQuery.data ?? []),
    [frontstageNavigationQuery.data]
  );
  const routes = useMemo<ConsolePrimaryNavigationRoute[]>(() => {
    if (!consoleNavigationQuery.data) return [];
    return [
      ...primaryRoutesFromConsoleNavigation(consoleNavigationQuery.data),
      ...dynamicRoutes
    ];
  }, [consoleNavigationQuery.data, dynamicRoutes]);
  const selectedDynamicRoute = dynamicRoutes.find(
    (candidate) =>
      pathname === candidate.path || pathname.startsWith(`${candidate.path}/`)
  );
  const hasSelectedDynamicPage = Boolean(selectedDynamicRoute);
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
          ...primaryRoutesFromConsoleNavigation(
            consoleNavigationQuery.data
          ).map((route) => ({
            key: route.id,
            label: renderNavigationLink(
              route.path,
              t(route.label_key),
              useRouterLinks,
              route.id === selectedKey && !hasSelectedDynamicPage
            )
          })),
          ...topbarNavigationItems({
            nodes: frontstageNavigationQuery.data ?? [],
            pathname,
            useRouterLinks,
            workspaceId,
            isDesignMode
          })
        ];

  return (
    <nav className="app-shell-navigation" aria-label="Primary">
      <Menu
        className="app-shell-menu"
        mode="horizontal"
        selectedKeys={
          selectedDynamicRoute
            ? [selectedDynamicRoute.id]
            : routes.some((route) => route.id === selectedKey)
              ? [selectedKey]
              : []
        }
        items={items}
        disabledOverflow
      />
      {isDesignMode && workspaceId ? (
        <TopbarNavigationDesigner
          workspaceId={workspaceId}
          nodes={frontstageNavigationQuery.data ?? []}
        />
      ) : null}
    </nav>
  );
}
