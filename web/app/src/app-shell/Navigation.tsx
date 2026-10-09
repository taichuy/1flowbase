import {
  navigationQueryStaleTime,
  selectNavigationQueryScope
} from '../state/navigation-query-scope';
import { Link } from '@tanstack/react-router';
import MenuOutlined from '@ant-design/icons/es/icons/MenuOutlined';
import { Button, Drawer, Menu, Typography } from 'antd';
import type { MenuProps } from 'antd';
import type { ItemType } from 'antd/es/menu/interface';
import { lazy, Suspense, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import type { ConsoleNavigation } from '@1flowbase/api-client';

import {
  fetchSettingsConsoleNavigation,
  settingsConsoleNavigationQueryKey
} from '../features/settings/api/console-navigation';
import { useBlockI18n } from '../features/frontstage/hooks/runtime-i18n/use-block-i18n';
import { i18nText } from '../shared/i18n/text';
import { getSelectedRouteId } from '../routes/route-config';
import {
  fetchFrontstagePageTree,
  frontstagePageTreeQueryKey,
  type FrontstagePageTreeNode
} from '../features/frontstage/api/page-tree';
import { useAuthStore } from '../state/auth-store';
import { useFrontstageDesignModeStore } from '../state/frontstage-design-mode-store';
import { loadTopbarNavigationDesigner } from './design-mode-demand';

const TopbarNavigationDesigner = lazy(() =>
  loadTopbarNavigationDesigner().then((module) => ({
    default: module.TopbarNavigationDesigner
  }))
);
const TopbarNavigationItemLabel = lazy(() =>
  loadTopbarNavigationDesigner().then((module) => ({
    default: module.TopbarNavigationItemLabel
  }))
);
const TopbarNavigationMoveDialog = lazy(() =>
  loadTopbarNavigationDesigner().then((module) => ({
    default: module.TopbarNavigationMoveDialog
  }))
);

interface ConsolePrimaryNavigationRoute {
  id: string;
  path: string;
  label_key: string;
}

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
        label_key: node.title?.trim() || i18nText('frontstage', 'auto.unnamed_page')
      }
    ];
  });
}

function topbarNavigationItems({
  nodes,
  pathname,
  useRouterLinks,
  workspaceId,
  isDesignMode,
  onOpenMoveNode,
  translateText
}: {
  nodes: FrontstagePageTreeNode[];
  pathname: string;
  useRouterLinks: boolean;
  workspaceId?: string;
  isDesignMode: boolean;
  onOpenMoveNode: (nodeId: string) => void;
  translateText: (text: string) => string;
}): ItemType[] {
  return nodes.reduce<ItemType[]>((items, node) => {
    if (node.placement !== 'topbar' || !node.slug) {
      return items;
    }
    const path = `/${node.slug}`;
    items.push({
      key: node.id,
      popupClassName:
        node.kind === 'group' ? 'app-shell-scrollable-menu-popup' : undefined,
      children:
        node.kind === 'group' && node.children?.length
          ? frontstagePageItems({
              nodes: node.children,
              slug: node.slug,
              pathname,
              useRouterLinks,
              translateText
            })
          : undefined,
      label:
        isDesignMode && workspaceId ? (
          <Suspense fallback={null}>
            <TopbarNavigationItemLabel
              workspaceId={workspaceId}
              node={node}
              onOpenMoveNode={onOpenMoveNode}
              siblings={nodes.filter(
                (candidate) => candidate.placement === 'topbar'
              )}
            >
              {renderNavigationLink(
                path,
                node.title?.trim()
                  ? translateText(node.title.trim())
                  : i18nText('frontstage', 'auto.unnamed_page'),
                useRouterLinks,
                pathname === path || pathname.startsWith(`${path}/`)
              )}
            </TopbarNavigationItemLabel>
          </Suspense>
        ) : (
          renderNavigationLink(
            path,
            node.title?.trim()
              ? translateText(node.title.trim())
              : i18nText('frontstage', 'auto.unnamed_page'),
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
  isCurrent: boolean,
  onNavigate?: () => void
) {
  if (useRouterLinks) {
    return (
      <Link
        to={pathname}
        className="app-shell-menu-link"
        aria-current={isCurrent ? 'page' : undefined}
        onClick={onNavigate}
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
      onClick={onNavigate}
    >
      {label}
    </a>
  );
}

function frontstagePageItems({
  nodes,
  slug,
  pathname,
  useRouterLinks,
  onNavigate,
  translateText
}: {
  nodes: FrontstagePageTreeNode[];
  slug: string;
  pathname: string;
  useRouterLinks: boolean;
  onNavigate?: () => void;
  translateText: (text: string) => string;
}): ItemType[] {
  return nodes.map((node) => {
    const title = node.title?.trim()
      ? translateText(node.title.trim())
      : i18nText('frontstage', 'auto.unnamed_page');
    if (node.kind === 'group') {
      return {
        key: node.id,
        label: title,
        popupClassName: 'app-shell-scrollable-menu-popup',
        children: frontstagePageItems({
          nodes: node.children ?? [],
          slug,
          pathname,
          useRouterLinks,
          onNavigate,
          translateText
        })
      };
    }

    const path = `/${slug}/pages/${node.id}`;
    return {
      key: node.id,
      label: renderNavigationLink(
        path,
        title,
        useRouterLinks,
        pathname === path || pathname.startsWith(`${path}/`),
        onNavigate
      )
    };
  });
}

function selectedFrontstagePageId(
  nodes: FrontstagePageTreeNode[],
  slug: string,
  pathname: string
): string | undefined {
  for (const node of nodes) {
    if (node.kind === 'page') {
      const path = `/${slug}/pages/${node.id}`;
      if (pathname === path || pathname.startsWith(`${path}/`)) {
        return node.id;
      }
    }

    const selectedChildId = selectedFrontstagePageId(
      node.children ?? [],
      slug,
      pathname
    );
    if (selectedChildId) return selectedChildId;
  }

  return undefined;
}

function frontstageGroupIds(nodes: FrontstagePageTreeNode[]): string[] {
  return nodes.flatMap((node) => [
    ...(node.kind === 'group' ? [node.id] : []),
    ...frontstageGroupIds(node.children ?? [])
  ]);
}

export function Navigation({
  pathname,
  useRouterLinks
}: {
  pathname: string;
  useRouterLinks: boolean;
}) {
  const navigationScope = useAuthStore(selectNavigationQueryScope);
  const { t } = useTranslation('appShell');
  const [isMobileNavigationOpen, setIsMobileNavigationOpen] = useState(false);
  const [moveNodeId, setMoveNodeId] = useState<string | null>(null);
  const workspaceId = useAuthStore(
    (state) => state.actor?.current_workspace_id
  );
  const runtimeI18n = useBlockI18n(workspaceId ?? '', Boolean(workspaceId));
  const isDesignMode = useFrontstageDesignModeStore(
    (state) => state.isDesignMode
  );
  const selectedKey = getSelectedRouteId(pathname);
  const consoleNavigationQuery = useQuery({
    queryKey: [...settingsConsoleNavigationQueryKey, navigationScope],
    staleTime: navigationQueryStaleTime,
    queryFn: fetchSettingsConsoleNavigation
  });
  const frontstageNavigationQuery = useQuery({
    queryKey: frontstagePageTreeQueryKey(workspaceId ?? '', navigationScope),
    staleTime: navigationQueryStaleTime,
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
  const selectedTopbarNode = (frontstageNavigationQuery.data ?? []).find(
    (node) => node.id === selectedDynamicRoute?.id
  );
  const closeMobileNavigation = () => setIsMobileNavigationOpen(false);
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
            isDesignMode,
            onOpenMoveNode: setMoveNodeId,
            translateText: runtimeI18n.t
          })
        ];
  const mobilePrimaryItems: MenuProps['items'] =
    consoleNavigationQuery.data === undefined
      ? items
      : [
          ...primaryRoutesFromConsoleNavigation(
            consoleNavigationQuery.data
          ).map((route) => ({
            key: route.id,
            label: renderNavigationLink(
              route.path,
              t(route.label_key),
              useRouterLinks,
              route.id === selectedKey && !hasSelectedDynamicPage,
              closeMobileNavigation
            )
          })),
          ...(frontstageNavigationQuery.data ?? []).flatMap((node) => {
            if (node.placement !== 'topbar' || !node.slug) return [];
            const path = `/${node.slug}`;
            const childItems = frontstagePageItems({
              nodes: node.children ?? [],
              slug: node.slug,
              pathname,
              useRouterLinks,
              onNavigate: closeMobileNavigation,
              translateText: runtimeI18n.t
            });
            return [
              {
                key: node.id,
                label: renderNavigationLink(
                  path,
                  node.title?.trim()
                    ? runtimeI18n.t(node.title.trim())
                    : i18nText('frontstage', 'auto.unnamed_page'),
                  useRouterLinks,
                  pathname === path || pathname.startsWith(`${path}/`),
                  closeMobileNavigation
                ),
                children: childItems.length > 0 ? childItems : undefined
              }
            ];
          })
        ];
  const mobileSelectedFrontstagePageId = selectedTopbarNode?.slug
    ? selectedFrontstagePageId(
        selectedTopbarNode.children ?? [],
        selectedTopbarNode.slug,
        pathname
      )
    : undefined;

  return (
    <nav className="app-shell-navigation" aria-label="Primary">
      <Button
        aria-label={t('auto.mobile_navigation_open')}
        className="app-shell-mobile-navigation-trigger"
        icon={<MenuOutlined />}
        onClick={() => setIsMobileNavigationOpen(true)}
        type="text"
      />
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
      />
      <Drawer
        className="app-shell-mobile-navigation-drawer"
        destroyOnHidden
        open={isMobileNavigationOpen}
        placement="left"
        title={
          <span className="app-shell-mobile-navigation-brand">
            <img alt="" aria-hidden="true" src="/icon.svg" />
            <Typography.Text>1flowbase</Typography.Text>
          </span>
        }
        onClose={closeMobileNavigation}
      >
        <section aria-label={t('auto.mobile_navigation')}>
          <Menu
            className="app-shell-mobile-navigation-menu"
            mode="inline"
            selectedKeys={
              mobileSelectedFrontstagePageId
                ? [mobileSelectedFrontstagePageId]
                : selectedDynamicRoute
                  ? [selectedDynamicRoute.id]
                  : routes.some((route) => route.id === selectedKey)
                    ? [selectedKey]
                    : []
            }
            defaultOpenKeys={
              selectedTopbarNode
                ? [
                    selectedTopbarNode.id,
                    ...frontstageGroupIds(selectedTopbarNode.children ?? [])
                  ]
                : undefined
            }
            items={mobilePrimaryItems}
          />
          {isDesignMode && workspaceId ? (
            <Suspense fallback={null}>
              <TopbarNavigationDesigner
                workspaceId={workspaceId}
                nodes={frontstageNavigationQuery.data ?? []}
              />
            </Suspense>
          ) : null}
        </section>
      </Drawer>
      {isDesignMode && workspaceId ? (
        <Suspense fallback={null}>
          <TopbarNavigationDesigner
            workspaceId={workspaceId}
            nodes={frontstageNavigationQuery.data ?? []}
          />
        </Suspense>
      ) : null}
      {isDesignMode && workspaceId && moveNodeId ? (
        <Suspense fallback={null}>
          <TopbarNavigationMoveDialog
            key={moveNodeId}
            workspaceId={workspaceId}
            nodeId={moveNodeId}
            nodes={frontstageNavigationQuery.data ?? []}
            onCancel={() => setMoveNodeId(null)}
          />
        </Suspense>
      ) : null}
    </nav>
  );
}
