import { fetchPublicSystemMetadata } from '@1flowbase/api-client/auth';
import type { QueryClient } from '@tanstack/react-query';

import {
  fetchFrontstageBlockCatalog,
  frontstageBlockCatalogQueryKey
} from '../../features/frontstage/api/block-catalog';
import {
  createFrontstageBlockCatalogPermissionFingerprint,
  frontstageBlockCatalogQueryStaleTime
} from '../../features/frontstage/api/block-catalog-scope';
import {
  fetchFrontstageBlockRoots,
  frontstageBlockTreeQueryKeys
} from '../../features/frontstage/api/block-tree';
import {
  fetchFrontstagePageContent,
  frontstagePageContentQueryKey
} from '../../features/frontstage/api/page-content';
import {
  fetchFrontstagePageTabs,
  frontstagePageTabsQueryKey
} from '../../features/frontstage/api/page-tabs';
import {
  frontstagePageTreeQueryOptions,
  type FrontstagePageTreeNode
} from '../../features/frontstage/api/page-tree';
import { fetchFrontstageRuntimeI18nCatalog } from '../../features/frontstage/api/runtime-i18n';
import {
  getFirstTopLevelPageId,
  resolveSelectedPageId
} from '../../features/frontstage/lib/page-tree';
import { settingsConsoleNavigationQueryOptions } from '../../features/settings/api/console-navigation';
import { systemMetadataQueryKey } from '../../shared/api/system-metadata';
import { resolveAuthenticatedLocale } from '../../shared/i18n/initial-locale';
import { resolveAppLocale } from '../../shared/i18n/locales';
import { useAuthStore, type AuthSnapshot } from '../../state/auth-store';
import { selectNavigationQueryScope } from '../../state/navigation-query-scope';
import { createApplicationQueryClient } from './authenticated-navigation';

const NON_FRONTSTAGE_PATHNAME_RE =
  /^\/(?:settings|applications|me|sign-in|mcp|console|oauth)(?:\/|$)/u;

const FRONTSTAGE_STARTUP_DATA_STALE_TIME_MS = 5_000;
const FRONTSTAGE_PAGE_TABS_STALE_TIME_MS = 30_000;

interface ParsedFrontstageRoute {
  kind: 'home' | 'slug';
  slug?: string;
  pageId?: string;
  tabRef?: string;
}

interface PrewarmedAuthenticatedClientSlot {
  csrfToken: string;
  actorId: string;
  workspaceId: string;
  queryClient: QueryClient;
  cancelled: boolean;
}

let prewarmedSlot: PrewarmedAuthenticatedClientSlot | null = null;

function safeDecodeSegment(value: string): string {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

export function isFrontstageCandidatePathname(pathname: string): boolean {
  return !NON_FRONTSTAGE_PATHNAME_RE.test(pathname);
}

function parseFrontstagePathname(
  pathname: string
): ParsedFrontstageRoute | null {
  if (!isFrontstageCandidatePathname(pathname)) return null;
  if (pathname === '/' || pathname === '') {
    return { kind: 'home' };
  }
  const segments = pathname.split('/').filter(Boolean).map(safeDecodeSegment);
  if (segments.length === 1) {
    return { kind: 'slug', slug: segments[0] };
  }
  if (segments.length === 3 && segments[1] === 'pages') {
    return { kind: 'slug', slug: segments[0], pageId: segments[2] };
  }
  if (
    segments.length === 5 &&
    segments[1] === 'pages' &&
    segments[3] === 'tabs'
  ) {
    return {
      kind: 'slug',
      slug: segments[0],
      pageId: segments[2],
      tabRef: segments[4]
    };
  }
  return null;
}

function resolvePrefetchTargetPage(
  nodes: readonly FrontstagePageTreeNode[],
  parsed: ParsedFrontstageRoute
): string | null {
  const rootNode =
    parsed.kind === 'home'
      ? nodes.find((node) => node.placement === 'topbar' && Boolean(node.slug))
      : nodes.find(
          (node) => node.placement === 'topbar' && node.slug === parsed.slug
        );
  if (!rootNode) return null;
  if (rootNode.kind === 'page') return rootNode.id;
  const scopedTree = rootNode.children;
  if (!parsed.pageId) {
    return getFirstTopLevelPageId(scopedTree);
  }
  return resolveSelectedPageId({
    pageId: parsed.pageId,
    pageTree: scopedTree
  }).selectedPageId;
}

function runFrontstagePrefetchPass({
  queryClient,
  actor,
  me,
  scope,
  pathname,
  isCurrent,
  preloadWorkspaceRoute,
  preloadHomeRoute
}: {
  queryClient: QueryClient;
  actor: AuthSnapshot['actor'];
  me: NonNullable<AuthSnapshot['me']>;
  scope: string;
  pathname: string;
  isCurrent: () => boolean;
  preloadWorkspaceRoute: () => Promise<unknown>;
  preloadHomeRoute: () => Promise<unknown>;
}): void {
  const workspaceId = actor.current_workspace_id;
  if (!workspaceId || !isFrontstageCandidatePathname(pathname)) return;

  const permissionFingerprint =
    createFrontstageBlockCatalogPermissionFingerprint(me);
  const sortedPermissions = me.permissions.slice().sort().join(',');
  const locale = resolveAppLocale(resolveAuthenticatedLocale(me));

  void preloadWorkspaceRoute().catch(() => undefined);
  if (pathname === '/') {
    void preloadHomeRoute().catch(() => undefined);
  }

  void queryClient.prefetchQuery({
    queryKey: systemMetadataQueryKey,
    queryFn: fetchPublicSystemMetadata,
    staleTime: 60_000
  });

  void queryClient.prefetchQuery({
    queryKey: frontstageBlockCatalogQueryKey({
      workspaceId,
      actorId: actor.id,
      permissionFingerprint
    }),
    queryFn: () => fetchFrontstageBlockCatalog(),
    staleTime: frontstageBlockCatalogQueryStaleTime
  });

  void queryClient.prefetchQuery({
    queryKey: [
      'frontstage',
      workspaceId,
      'runtime-i18n',
      actor.id,
      sortedPermissions,
      locale
    ],
    queryFn: () => fetchFrontstageRuntimeI18nCatalog(locale),
    staleTime: 5 * 60 * 1000
  });

  const parsedRoute = parseFrontstagePathname(pathname);
  if (!parsedRoute) return;

  void queryClient
    .fetchQuery(frontstagePageTreeQueryOptions(workspaceId, scope))
    .then(async (pageTree) => {
      if (!isCurrent()) return;
      const selectedPageId = resolvePrefetchTargetPage(pageTree, parsedRoute);
      if (!selectedPageId) return;

      const tabsKey = frontstagePageTabsQueryKey(workspaceId, selectedPageId);
      queryClient.setQueryDefaults(tabsKey, {
        staleTime: FRONTSTAGE_PAGE_TABS_STALE_TIME_MS
      });
      const tabs = await queryClient.fetchQuery({
        queryKey: tabsKey,
        queryFn: () => fetchFrontstagePageTabs(workspaceId, selectedPageId),
        staleTime: FRONTSTAGE_PAGE_TABS_STALE_TIME_MS
      });
      if (!isCurrent()) return;

      const defaultTabs = tabs.filter((tab) => tab.is_default);
      const defaultTab = defaultTabs.length === 1 ? defaultTabs[0] : undefined;
      const matchedTab = parsedRoute.tabRef
        ? tabs.find(
            (tab) =>
              tab.id === parsedRoute.tabRef ||
              tab.route_segment === parsedRoute.tabRef
          )
        : defaultTab;
      const tabReference = parsedRoute.tabRef ?? defaultTab?.id;

      if (tabReference) {
        const contentKey = frontstagePageContentQueryKey(
          workspaceId,
          selectedPageId,
          tabReference
        );
        queryClient.setQueryDefaults(contentKey, {
          staleTime: FRONTSTAGE_STARTUP_DATA_STALE_TIME_MS
        });
        void queryClient.prefetchQuery({
          queryKey: contentKey,
          queryFn: () =>
            fetchFrontstagePageContent(
              workspaceId,
              selectedPageId,
              tabReference
            ),
          staleTime: FRONTSTAGE_STARTUP_DATA_STALE_TIME_MS
        });
      }

      if (matchedTab?.id) {
        const rootsKey = frontstageBlockTreeQueryKeys.roots(
          workspaceId,
          selectedPageId,
          { tab_id: matchedTab.id }
        );
        queryClient.setQueryDefaults(rootsKey, {
          staleTime: FRONTSTAGE_STARTUP_DATA_STALE_TIME_MS
        });
        void queryClient.prefetchQuery({
          queryKey: rootsKey,
          queryFn: () =>
            fetchFrontstageBlockRoots(workspaceId, selectedPageId, {
              tab_id: matchedTab.id
            }),
          staleTime: FRONTSTAGE_STARTUP_DATA_STALE_TIME_MS
        });
      }
    })
    .catch(() => undefined);
}

/**
 * Starts authenticated navigation + frontstage prefetches as soon as session
 * discovery settles, overlapping network I/O with React/Antd chunk evaluation.
 */
export function prewarmAuthenticatedStartup(
  snapshot: AuthSnapshot,
  options: {
    getPathname?: () => string;
    preloadWorkspaceRoute?: () => Promise<unknown>;
    preloadHomeRoute?: () => Promise<unknown>;
  } = {}
): QueryClient {
  discardPrewarmedAuthenticatedQueryClient();
  const queryClient = createApplicationQueryClient();
  const workspaceId = snapshot.actor.current_workspace_id ?? '';
  const slot: PrewarmedAuthenticatedClientSlot = {
    csrfToken: snapshot.csrfToken,
    actorId: snapshot.actor.id,
    workspaceId,
    queryClient,
    cancelled: false
  };
  prewarmedSlot = slot;

  const scope = selectNavigationQueryScope({
    ...useAuthStore.getState(),
    sessionStatus: 'authenticated',
    csrfToken: snapshot.csrfToken,
    actor: snapshot.actor,
    me: snapshot.me,
    availableRoles: snapshot.availableRoles ?? []
  });
  void queryClient.prefetchQuery(settingsConsoleNavigationQueryOptions(scope));
  if (workspaceId) {
    void queryClient.prefetchQuery(
      frontstagePageTreeQueryOptions(workspaceId, scope)
    );
  }

  if (snapshot.me && workspaceId) {
    const pathname =
      options.getPathname?.() ??
      (typeof window !== 'undefined' ? window.location.pathname : '/');
    const preloadWorkspaceRoute =
      options.preloadWorkspaceRoute ??
      (() => import('../../features/frontstage/pages/FrontstageWorkspacePage'));
    const preloadHomeRoute =
      options.preloadHomeRoute ?? (() => import('../../routes/HomeRedirect'));
    runFrontstagePrefetchPass({
      queryClient,
      actor: snapshot.actor,
      me: snapshot.me,
      scope,
      pathname,
      isCurrent: () => !slot.cancelled,
      preloadWorkspaceRoute,
      preloadHomeRoute
    });
  }

  return queryClient;
}

export function claimPrewarmedAuthenticatedQueryClient(): QueryClient | null {
  if (!prewarmedSlot || prewarmedSlot.cancelled) return null;
  const state = useAuthStore.getState();
  if (
    state.sessionStatus !== 'authenticated' ||
    !state.actor ||
    state.csrfToken !== prewarmedSlot.csrfToken ||
    state.actor.id !== prewarmedSlot.actorId ||
    (state.actor.current_workspace_id ?? '') !== prewarmedSlot.workspaceId
  ) {
    discardPrewarmedAuthenticatedQueryClient();
    return null;
  }
  return prewarmedSlot.queryClient;
}

export function commitPrewarmedAuthenticatedQueryClient(
  queryClient: QueryClient
): void {
  if (!prewarmedSlot) return;
  if (prewarmedSlot.queryClient === queryClient) {
    prewarmedSlot = null;
    return;
  }
  discardPrewarmedAuthenticatedQueryClient();
}

export function discardPrewarmedAuthenticatedQueryClient(): void {
  if (!prewarmedSlot) return;
  const slot = prewarmedSlot;
  prewarmedSlot = null;
  slot.cancelled = true;
  slot.queryClient.clear();
}

export function startFrontstageStartupPrefetch(
  queryClient: QueryClient,
  options: {
    getPathname?: () => string;
    preloadWorkspaceRoute?: () => Promise<unknown>;
    preloadHomeRoute?: () => Promise<unknown>;
  } = {}
): () => void {
  const getPathname =
    options.getPathname ??
    (() => (typeof window !== 'undefined' ? window.location.pathname : '/'));
  const preloadWorkspaceRoute =
    options.preloadWorkspaceRoute ??
    (() => import('../../features/frontstage/pages/FrontstageWorkspacePage'));
  const preloadHomeRoute =
    options.preloadHomeRoute ?? (() => import('../../routes/HomeRedirect'));

  let previousFingerprint: string | null = null;
  let activeGeneration = 0;

  const synchronize = () => {
    const state = useAuthStore.getState();
    const pathname = getPathname();
    if (
      state.sessionStatus !== 'authenticated' ||
      !state.actor ||
      !state.me ||
      !isFrontstageCandidatePathname(pathname)
    ) {
      if (previousFingerprint !== null) {
        activeGeneration += 1;
        previousFingerprint = null;
      }
      return;
    }

    const { actor, me } = state;
    const workspaceId = actor.current_workspace_id;
    if (!workspaceId) return;

    const scope = selectNavigationQueryScope(state);
    const permissionFingerprint =
      createFrontstageBlockCatalogPermissionFingerprint(me);
    const locale = resolveAppLocale(resolveAuthenticatedLocale(me));
    const fingerprint = JSON.stringify([
      scope,
      workspaceId,
      permissionFingerprint,
      locale,
      pathname
    ]);
    if (fingerprint === previousFingerprint) return;
    previousFingerprint = fingerprint;
    const generation = ++activeGeneration;
    const isCurrent = () => {
      const latest = useAuthStore.getState();
      return (
        generation === activeGeneration &&
        latest.sessionStatus === 'authenticated' &&
        latest.actor?.id === actor.id &&
        latest.actor?.current_workspace_id === workspaceId
      );
    };

    runFrontstagePrefetchPass({
      queryClient,
      actor,
      me,
      scope,
      pathname,
      isCurrent,
      preloadWorkspaceRoute,
      preloadHomeRoute
    });
  };

  const unsubscribe = useAuthStore.subscribe(synchronize);
  synchronize();
  return () => {
    activeGeneration += 1;
    unsubscribe();
  };
}
