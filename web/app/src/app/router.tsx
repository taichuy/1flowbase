import {
  Navigate,
  Outlet,
  RouterProvider,
  useNavigate,
  createRootRoute,
  createRoute,
  createRouter,
  useRouterState
} from '@tanstack/react-router';
import { useQuery } from '@tanstack/react-query';
import { Result } from 'antd';
import { Suspense, lazy, useState, type ReactNode } from 'react';

import { AppShellFrame } from '../app-shell/AppShellFrame';
import { SignInPage } from '../features/auth/pages/SignInPage';
import type { ApplicationSectionKey } from '../features/applications/lib/application-sections';
import { EmbeddedAppsPage } from '../features/embedded-apps/pages/EmbeddedAppsPage';
import {
  fetchFrontstagePageContent,
  frontstagePageContentQueryKey
} from '../features/frontstage/api/page-content';
import {
  fetchFrontstagePageTree,
  frontstagePageTreeQueryKey
} from '../features/frontstage/api/page-tree';
import {
  fetchFrontstagePageTabs,
  frontstagePageTabsQueryKey,
  type FrontstagePageTab
} from '../features/frontstage/api/page-tabs';
import { useFrontstagePageTreeMutations } from '../features/frontstage/hooks/use-frontstage-page-tree-mutations';
import { isForbiddenResponseError } from '../features/frontstage/lib/api-errors';
import { resolveSelectedPageId } from '../features/frontstage/lib/page-tree';
import { HomePage } from '../features/home/pages/HomePage';
import { FrontStagePage } from '../features/frontstage/pages/FrontStagePage';
import type { MeSectionKey } from '../features/me/lib/me-sections';
import { MePage } from '../features/me/pages/MePage';
import { TemplatesPage } from '../features/templates/pages/TemplatesPage';
import { RouteGuard } from '../routes/route-guards';
import { SessionGuard } from '../routes/session-guard';
import {
  FRONTSTAGE_SLUG_PAGE_PATH,
  FRONTSTAGE_SLUG_PAGE_TAB_PATH,
  FRONTSTAGE_SLUG_PATH
} from '../routes/route-config';
import { LoadingState } from '../shared/ui/loading-state/LoadingState';
import { useAuthStore } from '../state/auth-store';
import { i18nText } from '../shared/i18n/text';

const ApplicationDetailPage = lazy(() =>
  import('../features/applications/pages/ApplicationDetailPage').then(
    (module) => ({
      default: module.ApplicationDetailPage
    })
  )
);
const SettingsPage = lazy(() =>
  import('../features/settings/pages/SettingsPage').then((module) => ({
    default: module.SettingsPage
  }))
);

function NotFoundPage() {
  return <Result status="404" title={i18nText('app', 'auto.page_not_found')} />;
}

function RouteLoadingFallback() {
  return <LoadingState fullscreen />;
}

function LazyRouteBoundary({ children }: { children: ReactNode }) {
  return <Suspense fallback={<RouteLoadingFallback />}>{children}</Suspense>;
}

function ShellLayout() {
  const pathname = useRouterState({
    select: (state) => state.location.pathname
  });

  return (
    <AppShellFrame pathname={pathname} useRouterLinks>
      <Outlet />
    </AppShellFrame>
  );
}

function ApplicationIndexRedirect() {
  const { applicationId } = applicationIndexRoute.useParams();

  return (
    <Navigate
      to="/applications/$applicationId/orchestration"
      params={{ applicationId }}
      replace
    />
  );
}

function ApplicationSectionRoute({
  applicationId,
  requestedSectionKey
}: {
  applicationId: string;
  requestedSectionKey: ApplicationSectionKey;
}) {
  return (
    <RouteGuard routeId="application-detail">
      <LazyRouteBoundary>
        <ApplicationDetailPage
          applicationId={applicationId}
          requestedSectionKey={requestedSectionKey}
        />
      </LazyRouteBoundary>
    </RouteGuard>
  );
}

const rootRoute = createRootRoute({
  component: () => <Outlet />,
  notFoundComponent: NotFoundPage
});

const shellRoute = createRoute({
  getParentRoute: () => rootRoute,
  id: 'shell',
  component: ShellLayout,
  notFoundComponent: NotFoundPage
});

const homeRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/',
  component: () => (
    <RouteGuard routeId="home">
      <HomePage />
    </RouteGuard>
  )
});

const applicationIndexRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/applications/$applicationId',
  component: ApplicationIndexRedirect,
  notFoundComponent: NotFoundPage
});

const applicationOrchestrationRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/applications/$applicationId/orchestration',
  notFoundComponent: NotFoundPage,
  component: () => {
    const { applicationId } = applicationOrchestrationRoute.useParams();

    return (
      <ApplicationSectionRoute
        applicationId={applicationId}
        requestedSectionKey="orchestration"
      />
    );
  }
});

const applicationApiRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/applications/$applicationId/api',
  notFoundComponent: NotFoundPage,
  component: () => {
    const { applicationId } = applicationApiRoute.useParams();

    return (
      <ApplicationSectionRoute
        applicationId={applicationId}
        requestedSectionKey="api"
      />
    );
  }
});

const applicationLogsRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/applications/$applicationId/logs',
  validateSearch: (search: Record<string, unknown>) => ({
    run_id:
      typeof search.run_id === 'string' && search.run_id.trim().length > 0
        ? search.run_id
        : undefined,
    view: search.view === 'trace' ? ('trace' as const) : undefined
  }),
  notFoundComponent: NotFoundPage,
  component: () => {
    const { applicationId } = applicationLogsRoute.useParams();

    return (
      <ApplicationSectionRoute
        applicationId={applicationId}
        requestedSectionKey="logs"
      />
    );
  }
});

const applicationMonitoringRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/applications/$applicationId/monitoring',
  notFoundComponent: NotFoundPage,
  component: () => {
    const { applicationId } = applicationMonitoringRoute.useParams();

    return (
      <ApplicationSectionRoute
        applicationId={applicationId}
        requestedSectionKey="monitoring"
      />
    );
  }
});

const embeddedAppsRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/embedded-apps',
  notFoundComponent: NotFoundPage,
  component: () => (
    <RouteGuard routeId="embedded-apps">
      <EmbeddedAppsPage />
    </RouteGuard>
  )
});

const templatesRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/templates',
  notFoundComponent: NotFoundPage,
  component: () => (
    <RouteGuard routeId="templates">
      <TemplatesPage />
    </RouteGuard>
  )
});

function renderSettingsRoute(
  requestedSectionKey?: string,
  modelProviderTab?: 'providers' | 'request-logs'
) {
  return (
    <RouteGuard routeId="settings">
      <LazyRouteBoundary>
        <SettingsPage
          requestedSectionKey={requestedSectionKey}
          modelProviderTab={modelProviderTab}
        />
      </LazyRouteBoundary>
    </RouteGuard>
  );
}

function renderMeRoute(requestedSectionKey?: MeSectionKey) {
  return (
    <RouteGuard routeId="me">
      <MePage requestedSectionKey={requestedSectionKey} />
    </RouteGuard>
  );
}

function FrontStageWorkspaceContent({
  workspaceId,
  pageId,
  tabRef,
  rootNode
}: {
  workspaceId: string;
  pageId?: string;
  tabRef?: string;
  rootNode?: import('../features/frontstage/api/page-tree').FrontstagePageTreeNode;
}) {
  const navigate = useNavigate();
  const pageTreeQuery = useQuery({
    queryKey: frontstagePageTreeQueryKey(workspaceId),
    queryFn: () => fetchFrontstagePageTree(workspaceId),
    retry: false
  });
  const pageTreeMutations = useFrontstagePageTreeMutations(workspaceId);
  const pageTreeFromApi =
    rootNode?.kind === 'group'
      ? rootNode.children
      : rootNode?.kind === 'page'
        ? [rootNode]
        : pageTreeQuery.data;
  const effectivePageId = rootNode?.kind === 'page' ? rootNode.id : pageId;
  const selectedPageId =
    rootNode?.kind === 'page'
      ? rootNode.id
      : pageTreeFromApi
        ? resolveSelectedPageId({
            pageId: effectivePageId,
            pageTree: pageTreeFromApi
          }).selectedPageId
        : null;
  const pageTabsQuery = useQuery({
    queryKey: selectedPageId
      ? frontstagePageTabsQueryKey(workspaceId, selectedPageId)
      : ['frontstage', workspaceId, 'pages', 'unselected', 'tabs'],
    queryFn: () => {
      if (!selectedPageId) {
        throw new Error('FrontStage page tabs query requires selected page');
      }

      return fetchFrontstagePageTabs(workspaceId, selectedPageId);
    },
    enabled: Boolean(selectedPageId),
    retry: false
  });
  const defaultTabs = pageTabsQuery.data?.filter((tab) => tab.is_default) ?? [];
  const defaultTab = defaultTabs.length === 1 ? defaultTabs[0] : undefined;
  const tabReference = tabRef ?? defaultTab?.id;
  const shouldLoadPageContent = Boolean(
    effectivePageId && selectedPageId && tabReference
  );
  const pageContentQuery = useQuery({
    queryKey:
      selectedPageId && tabReference
        ? frontstagePageContentQueryKey(workspaceId, selectedPageId, tabReference)
        : ['frontstage', workspaceId, 'pages', 'unselected', 'content'],
    queryFn: () => {
      if (!selectedPageId) {
        throw new Error('FrontStage page content query requires selected page');
      }

      if (!tabReference) {
        throw new Error('FrontStage page content query requires selected tab');
      }

      return fetchFrontstagePageContent(workspaceId, selectedPageId, tabReference);
    },
    enabled: shouldLoadPageContent,
    retry: false
  });

  if (rootNode?.kind === 'page' && !pageId && rootNode.slug) {
    return (
      <Navigate
        to={FRONTSTAGE_SLUG_PAGE_PATH}
        params={{ slug: rootNode.slug, pageId: rootNode.id }}
        replace
      />
    );
  }

  if (
    selectedPageId &&
    !tabRef &&
    pageTabsQuery.data &&
    defaultTabs.length !== 1
  ) {
    return (
      <Result
        status="error"
        title={i18nText('frontstage', 'auto.page_tabs_invalid_default_title')}
        subTitle={i18nText(
          'frontstage',
          'auto.page_tabs_invalid_default_detail'
        )}
      />
    );
  }

  const resolvedTab = pageContentQuery.data?.tab;
  if (selectedPageId && tabRef && resolvedTab && rootNode?.slug) {
    if (resolvedTab.isDefault) {
      return (
        <Navigate
          to={FRONTSTAGE_SLUG_PAGE_PATH}
          params={{ slug: rootNode.slug, pageId: selectedPageId }}
          replace
        />
      );
    }

    if (resolvedTab.routeSegment && tabRef !== resolvedTab.routeSegment) {
      return (
        <Navigate
          to={FRONTSTAGE_SLUG_PAGE_TAB_PATH}
          params={{
            slug: rootNode.slug,
            pageId: selectedPageId,
            tabRef: resolvedTab.routeSegment
          }}
          replace
        />
      );
    }
  }

  return (
    <LazyRouteBoundary>
      <FrontStagePage
        workspaceId={workspaceId}
        pageId={effectivePageId}
        tabId={resolvedTab?.id}
        showSidebar={rootNode?.kind !== 'page'}
        autoSelectFirstPage={rootNode?.kind !== 'group' || Boolean(pageId)}
        initialPageTree={pageTreeFromApi}
        isPageTreeLoading={pageTreeQuery.isLoading}
        hasPageTreeLoadError={pageTreeQuery.isError}
        pageContent={pageContentQuery.data}
        isPageContentLoading={pageContentQuery.isLoading}
        hasPageContentLoadError={pageContentQuery.isError}
        isPageContentPermissionDenied={isForbiddenResponseError(
          pageContentQuery.error
        )}
        isPageTreeMutating={pageTreeMutations.isPending}
        pageTreeMutationError={pageTreeMutations.error}
        onCreateGroupNode={(input) =>
          pageTreeMutations.createGroup({
            ...input,
            parentId:
              rootNode?.kind === 'group' && input.parentId === null
                ? rootNode.id
                : input.parentId
          })
        }
        onCreatePageNode={(input) =>
          pageTreeMutations.createPage({
            ...input,
            parentId:
              rootNode?.kind === 'group' && input.parentId === null
                ? rootNode.id
                : input.parentId
          })
        }
        onRenamePageNode={pageTreeMutations.renameNode}
        onUpdatePageNodeMetadata={pageTreeMutations.updateNodeMetadata}
        onMovePageNode={pageTreeMutations.moveNode}
        onDeletePageNode={pageTreeMutations.deleteNode}
        onRetryLoadPageTree={() => {
          void pageTreeQuery.refetch();
        }}
        onRetryLoadPageContent={() => {
          void pageContentQuery.refetch();
        }}
        onNavigatePage={(nextPageId) => {
          if (!rootNode?.slug) return;
          void navigate(
            nextPageId
              ? {
                  to: FRONTSTAGE_SLUG_PAGE_PATH,
                  params: { slug: rootNode.slug, pageId: nextPageId }
                }
              : { to: FRONTSTAGE_SLUG_PATH, params: { slug: rootNode.slug } }
          );
        }}
        onNavigateTab={(nextTab: FrontstagePageTab) => {
          if (!selectedPageId) return;
          if (!rootNode?.slug) return;
          if (nextTab.is_default) {
            void navigate({
              to: FRONTSTAGE_SLUG_PAGE_PATH,
              params: { slug: rootNode.slug, pageId: selectedPageId }
            });
            return;
          }
          if (!nextTab.route_segment) return;
          void navigate({
            to: FRONTSTAGE_SLUG_PAGE_TAB_PATH,
            params: {
              slug: rootNode.slug,
              pageId: selectedPageId,
              tabRef: nextTab.route_segment
            }
          });
        }}
      />
    </LazyRouteBoundary>
  );
}

function FrontStageSlugRoute({
  slug,
  pageId,
  tabRef
}: {
  slug: string;
  pageId?: string;
  tabRef?: string;
}) {
  const workspaceId = useAuthStore(
    (state) => state.actor?.current_workspace_id
  );
  const pageTreeQuery = useQuery({
    queryKey: frontstagePageTreeQueryKey(workspaceId ?? ''),
    queryFn: () => fetchFrontstagePageTree(workspaceId ?? ''),
    enabled: Boolean(workspaceId),
    retry: false
  });
  const rootNode = pageTreeQuery.data?.find(
    (node) => node.placement === 'topbar' && node.slug === slug
  );
  if (!workspaceId) return <Navigate to="/" replace />;
  if (pageTreeQuery.isLoading) return <RouteLoadingFallback />;
  if (!rootNode) return <NotFoundPage />;
  return (
    <SessionGuard>
      <FrontStageWorkspaceContent
        workspaceId={workspaceId}
        pageId={pageId}
        tabRef={tabRef}
        rootNode={rootNode}
      />
    </SessionGuard>
  );
}

const settingsIndexRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/settings',
  notFoundComponent: NotFoundPage,
  component: () => renderSettingsRoute()
});

const settingsDocsRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/settings/docs',
  notFoundComponent: NotFoundPage,
  component: () => renderSettingsRoute('docs')
});

const settingsApiKeyAuthenticationRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/settings/api-key-authentication',
  notFoundComponent: NotFoundPage,
  component: () => renderSettingsRoute('api-key-authentication')
});

const settingsAuthCenterRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/settings/auth-center',
  notFoundComponent: NotFoundPage,
  component: () => renderSettingsRoute('auth-center')
});

const settingsSystemRuntimeRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/settings/system-runtime',
  notFoundComponent: NotFoundPage,
  component: () => renderSettingsRoute('system-runtime')
});

const settingsHostInfrastructureRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/settings/host-infrastructure',
  notFoundComponent: NotFoundPage,
  component: () => renderSettingsRoute('host-infrastructure')
});

const settingsMemoryObservationRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/settings/memory-observation',
  notFoundComponent: NotFoundPage,
  component: () => renderSettingsRoute('memory-observation')
});

const settingsFilesRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/settings/files',
  notFoundComponent: NotFoundPage,
  component: () => renderSettingsRoute('files')
});

const settingsApplicationsRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/settings/applications',
  notFoundComponent: NotFoundPage,
  component: () => renderSettingsRoute('applications')
});

const settingsDataModelsRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/settings/data-models',
  notFoundComponent: NotFoundPage,
  component: () => renderSettingsRoute('data-models')
});

const settingsModelProvidersRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/settings/model-providers',
  notFoundComponent: NotFoundPage,
  component: () => <Navigate to="/settings/model-providers/providers" replace />
});

const settingsModelProviderInstancesRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/settings/model-providers/providers',
  notFoundComponent: NotFoundPage,
  component: () => renderSettingsRoute('model-providers', 'providers')
});

const settingsModelProviderRequestLogsRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/settings/model-providers/request-logs',
  notFoundComponent: NotFoundPage,
  component: () => renderSettingsRoute('model-providers', 'request-logs')
});

const settingsMcpManagementRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/settings/mcp-management',
  notFoundComponent: NotFoundPage,
  component: () => renderSettingsRoute('mcp-management')
});

const settingsMembersRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/settings/members',
  notFoundComponent: NotFoundPage,
  component: () => renderSettingsRoute('members')
});

const settingsRolesRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/settings/roles',
  notFoundComponent: NotFoundPage,
  component: () => renderSettingsRoute('roles')
});

const settingsDynamicRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/settings/$sectionKey',
  notFoundComponent: NotFoundPage,
  component: () => {
    const { sectionKey } = settingsDynamicRoute.useParams();

    return renderSettingsRoute(sectionKey);
  }
});

const meIndexRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/me',
  notFoundComponent: NotFoundPage,
  component: () => renderMeRoute()
});

const meProfileRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/me/profile',
  notFoundComponent: NotFoundPage,
  component: () => renderMeRoute('profile')
});

const meSecurityRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: '/me/security',
  notFoundComponent: NotFoundPage,
  component: () => renderMeRoute('security')
});

const frontstageSlugRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: FRONTSTAGE_SLUG_PATH,
  notFoundComponent: NotFoundPage,
  component: () => {
    const { slug } = frontstageSlugRoute.useParams();
    return <FrontStageSlugRoute slug={slug} />;
  }
});

const frontstageSlugPageRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: FRONTSTAGE_SLUG_PAGE_PATH,
  notFoundComponent: NotFoundPage,
  component: () => {
    const { slug, pageId } = frontstageSlugPageRoute.useParams();
    return <FrontStageSlugRoute slug={slug} pageId={pageId} />;
  }
});

const frontstageSlugPageTabRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: FRONTSTAGE_SLUG_PAGE_TAB_PATH,
  notFoundComponent: NotFoundPage,
  component: () => {
    const { slug, pageId, tabRef } = frontstageSlugPageTabRoute.useParams();
    return <FrontStageSlugRoute slug={slug} pageId={pageId} tabRef={tabRef} />;
  }
});

const signInRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/sign-in',
  component: () => (
    <RouteGuard routeId="sign-in">
      <SignInPage />
    </RouteGuard>
  )
});

const routeTree = rootRoute.addChildren([
  shellRoute.addChildren([
    homeRoute,
    applicationIndexRoute,
    applicationOrchestrationRoute,
    applicationApiRoute,
    applicationLogsRoute,
    applicationMonitoringRoute,
    embeddedAppsRoute,
    templatesRoute,
    settingsIndexRoute,
    settingsDocsRoute,
    settingsApiKeyAuthenticationRoute,
    settingsAuthCenterRoute,
    settingsSystemRuntimeRoute,
    settingsHostInfrastructureRoute,
    settingsMemoryObservationRoute,
    settingsApplicationsRoute,
    settingsFilesRoute,
    settingsDataModelsRoute,
    settingsModelProvidersRoute,
    settingsModelProviderInstancesRoute,
    settingsModelProviderRequestLogsRoute,
    settingsMcpManagementRoute,
    settingsMembersRoute,
    settingsRolesRoute,
    settingsDynamicRoute,
    meIndexRoute,
    meProfileRoute,
    meSecurityRoute,
    frontstageSlugRoute,
    frontstageSlugPageRoute,
    frontstageSlugPageTabRoute
  ]),
  signInRoute
]);

function createAppRouter() {
  return createRouter({
    routeTree,
    defaultNotFoundComponent: NotFoundPage,
    notFoundMode: 'root'
  });
}

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof createAppRouter>;
  }
}

export function AppRouterProvider() {
  const [router] = useState(createAppRouter);

  return <RouterProvider router={router} />;
}
