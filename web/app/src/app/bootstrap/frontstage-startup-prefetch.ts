import {
  getConsoleFrontstageBootstrap,
  type ConsoleFrontstageBootstrapQuery,
  type ConsoleFrontstageBootstrapResponse
} from '@1flowbase/api-client';
import { fetchPublicSystemMetadata } from '@1flowbase/api-client/auth';
import type { QueryClient, QueryKey } from '@tanstack/react-query';

import {
  DEFAULT_BOOTSTRAP_VIEWPORT_CODE_LIMIT,
  consumeEarlyConsoleNavigationFlight,
  consumeEarlyFrontstageBootstrapFlight,
  fetchWithConditionalBootstrapCache,
  type EarlyBootstrapQuery
} from './early-console-startup-handoff';

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
  frontstagePageContentQueryKey,
  mapFrontstagePageContent
} from '../../features/frontstage/api/page-content';
import {
  fetchFrontstagePageTabs,
  frontstagePageTabsQueryKey
} from '../../features/frontstage/api/page-tabs';
import {
  frontstagePageTreeQueryKey,
  frontstagePageTreeQueryOptions,
  getFrontstageApiBaseUrl,
  type FrontstagePageTreeNode
} from '../../features/frontstage/api/page-tree';
import { fetchFrontstageRuntimeI18nCatalog } from '../../features/frontstage/api/runtime-i18n';
import {
  clearStartupPrefetchedBlockNodeCodes,
  prefetchStartupBlockCodeEntries,
  seedStartupPrefetchedBlockNodeCodes,
  type StartupPrefetchedBlockCodeEntry
} from '../../features/frontstage/api/startup-block-code-cache';
import { getFirstTopLevelPageId } from '../../features/frontstage/lib/page-tree';
import { settingsConsoleNavigationQueryOptions } from '../../features/settings/api/console-navigation';
import { systemMetadataQueryKey } from '../../shared/api/system-metadata';
import {
  getSeededPublicSystemMetadata,
  getSeededPublicSystemMetadataUpdatedAt
} from '../../shared/api/system-metadata-seed';
import { resolveAuthenticatedLocale } from '../../shared/i18n/initial-locale';
import { resolveAppLocale } from '../../shared/i18n/locales';
import {
  resolveRuntimeConcurrencyProfile,
  type RuntimeConcurrencyProfile
} from '../../shared/utils/runtime-concurrency-profile';
import { useAuthStore, type AuthSnapshot } from '../../state/auth-store';
import { selectNavigationQueryScope } from '../../state/navigation-query-scope';
import { createApplicationQueryClient } from './authenticated-navigation';
import {
  loadFrontstageWorkspacePageModule,
  preloadAuthenticatedStartupRuntime
} from './authenticated-shell-preload';

const NON_FRONTSTAGE_PATHNAME_RE =
  /^\/(?:settings|applications|me|sign-in|mcp|console|oauth)(?:\/|$)/u;
const SIGN_IN_PATHNAME_RE = /^\/sign-in\/?$/u;

const FRONTSTAGE_STARTUP_DATA_STALE_TIME_MS = 30_000;

const DEFAULT_CONTROL_GATE_TIMEOUT_MS = 5_000;

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

interface FrontstageStartupHydrationResult {
  targetPath: string;
  prefetchedCodeEntries: ReadonlyArray<StartupPrefetchedBlockCodeEntry>;
}

interface ActiveHydrationRecord {
  keys: Set<string>;
  promise: Promise<FrontstageStartupHydrationResult>;
}

let prewarmedSlot: PrewarmedAuthenticatedClientSlot | null = null;
const activeHydrationFlights = new WeakMap<
  QueryClient,
  ActiveHydrationRecord
>();

function withTimeout<T>(promise: Promise<T>, timeoutMs: number): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new Error('Startup transition stage timed out'));
    }, timeoutMs);
    (timer as { unref?: () => void }).unref?.();
    promise.then(
      (value) => {
        clearTimeout(timer);
        resolve(value);
      },
      (error) => {
        clearTimeout(timer);
        reject(error);
      }
    );
  });
}

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

function normalizeAuthenticatedStartupPathname(pathname: string): string {
  return SIGN_IN_PATHNAME_RE.test(pathname) ? '/' : pathname;
}

export function isFrontstageBootstrapOwnedForState(
  state: Pick<
    ReturnType<typeof useAuthStore.getState>,
    'sessionStatus' | 'actor' | 'me'
  >,
  pathname = typeof window !== 'undefined' ? window.location.pathname : '/'
): boolean {
  const effectivePathname = normalizeAuthenticatedStartupPathname(pathname);
  return Boolean(
    state.sessionStatus === 'authenticated' &&
    state.actor?.current_workspace_id &&
    state.me &&
    isFrontstageCandidatePathname(effectivePathname)
  );
}

export function clearFrontstageStartupHydrationState(
  queryClient?: QueryClient
): void {
  if (queryClient) {
    activeHydrationFlights.delete(queryClient);
  }
  clearStartupPrefetchedBlockNodeCodes();
}

function buildHydrationKey(
  scope: string,
  workspaceId: string,
  actorId: string,
  pathname: string
): string {
  return JSON.stringify([scope, workspaceId, actorId, pathname]);
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
  return containsPageNode(scopedTree, parsed.pageId) ? parsed.pageId : null;
}

function resolveCanonicalFrontstageLanding(
  nodes: readonly FrontstagePageTreeNode[]
): { targetPath: string; selectedPageId: string | null } {
  const rootNode = nodes.find(
    (node) => node.placement === 'topbar' && Boolean(node.slug)
  );
  if (!rootNode?.slug) {
    return { targetPath: '/me', selectedPageId: null };
  }
  const encodedSlug = encodeURIComponent(rootNode.slug);
  if (rootNode.kind === 'page') {
    return { targetPath: `/${encodedSlug}`, selectedPageId: rootNode.id };
  }
  const firstPageId = getFirstTopLevelPageId(rootNode.children);
  if (firstPageId) {
    return {
      targetPath: `/${encodedSlug}/pages/${encodeURIComponent(firstPageId)}`,
      selectedPageId: firstPageId
    };
  }
  return { targetPath: `/${encodedSlug}`, selectedPageId: null };
}

function extractRootBlockIds(roots: unknown): string[] {
  if (!Array.isArray(roots)) return [];
  return roots
    .map((item) =>
      item && typeof item === 'object' && 'block_id' in item
        ? (item as { block_id?: unknown }).block_id
        : undefined
    )
    .filter((id): id is string => typeof id === 'string' && id.length > 0);
}

function seedSupportingFrontstageQueries({
  queryClient,
  workspaceId,
  actorId,
  me,
  deferSupportingCatalogs = false
}: {
  queryClient: QueryClient;
  workspaceId: string;
  actorId: string;
  me: NonNullable<AuthSnapshot['me']>;
  deferSupportingCatalogs?: boolean;
}): () => void {
  const permissionFingerprint =
    createFrontstageBlockCatalogPermissionFingerprint(me);
  const locale = resolveAppLocale(resolveAuthenticatedLocale(me));

  const seededMetadata = getSeededPublicSystemMetadata();
  if (
    seededMetadata?.system_name !== undefined &&
    seededMetadata?.hide_login_tagline !== undefined &&
    getSeededPublicSystemMetadataUpdatedAt() > 0
  ) {
    if (!queryClient.getQueryData(systemMetadataQueryKey)) {
      queryClient.setQueryData(systemMetadataQueryKey, seededMetadata, {
        updatedAt: getSeededPublicSystemMetadataUpdatedAt()
      });
    }
  } else {
    void queryClient.prefetchQuery({
      queryKey: systemMetadataQueryKey,
      queryFn: fetchPublicSystemMetadata,
      staleTime: 60_000
    });
  }

  let catalogStarted = false;
  const runSupportingCatalogs = () => {
    if (catalogStarted) return;
    catalogStarted = true;
    void queryClient.prefetchQuery({
      queryKey: frontstageBlockCatalogQueryKey({
        workspaceId,
        actorId,
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
        actorId,
        permissionFingerprint,
        locale
      ],
      queryFn: () => fetchFrontstageRuntimeI18nCatalog(locale),
      staleTime: 5 * 60 * 1000
    });
  };

  if (!deferSupportingCatalogs) {
    runSupportingCatalogs();
  }
  return runSupportingCatalogs;
}

function toBootstrapQuery(
  parsed: ParsedFrontstageRoute | null
): ConsoleFrontstageBootstrapQuery & EarlyBootstrapQuery {
  if (!parsed || parsed.kind === 'home') {
    return { code_limit: DEFAULT_BOOTSTRAP_VIEWPORT_CODE_LIMIT };
  }
  return {
    ...(parsed.slug ? { slug: parsed.slug } : {}),
    ...(parsed.pageId ? { page_id: parsed.pageId } : {}),
    ...(parsed.tabRef ? { tab_reference: parsed.tabRef } : {}),
    code_limit: DEFAULT_BOOTSTRAP_VIEWPORT_CODE_LIMIT
  };
}

function prefetchSettingsConsoleNavigationWithEarlyHandoff(
  queryClient: QueryClient,
  actor: NonNullable<AuthSnapshot['actor']>,
  scope: string
): void {
  const baseOptions = settingsConsoleNavigationQueryOptions(scope);
  const earlyNavFlight = consumeEarlyConsoleNavigationFlight<
    Awaited<ReturnType<NonNullable<typeof baseOptions.queryFn>>>
  >(actor.id, actor.current_workspace_id, actor.effective_display_role ?? '');
  if (earlyNavFlight) {
    void queryClient.prefetchQuery({
      ...baseOptions,
      queryFn: () => earlyNavFlight
    });
    return;
  }
  void queryClient.prefetchQuery(baseOptions);
}

async function tryHydrateFromBootstrapEndpoint({
  queryClient,
  actorId,
  workspaceId,
  role,
  locale,
  scope,
  parsedRoute,
  isCancelled
}: {
  queryClient: QueryClient;
  actorId: string;
  workspaceId: string;
  role: string;
  locale: string;
  scope: string;
  parsedRoute: ParsedFrontstageRoute | null;
  isCancelled: () => boolean;
}): Promise<{
  hydrated: boolean;
  targetPath: string;
  prefetchedCodeEntries: ReadonlyArray<StartupPrefetchedBlockCodeEntry>;
}> {
  try {
    const query = toBootstrapQuery(parsedRoute);
    const earlyFlight =
      consumeEarlyFrontstageBootstrapFlight<ConsoleFrontstageBootstrapResponse>(
        actorId,
        workspaceId,
        role,
        query
      );
    const payload = earlyFlight
      ? await earlyFlight
      : import.meta.env.VITEST
        ? await getConsoleFrontstageBootstrap(query, getFrontstageApiBaseUrl())
        : await fetchWithConditionalBootstrapCache<ConsoleFrontstageBootstrapResponse>(
            {
              actorId,
              workspaceId,
              role,
              locale,
              query,
              baseUrl: getFrontstageApiBaseUrl()
            }
          );
    if (isCancelled() || !Array.isArray(payload?.pages)) {
      return { hydrated: false, targetPath: '/', prefetchedCodeEntries: [] };
    }
    const pageTree = payload.pages as FrontstagePageTreeNode[];
    seedStartupQuery(
      queryClient,
      frontstagePageTreeQueryKey(workspaceId, scope),
      pageTree
    );

    const landing = resolveCanonicalFrontstageLanding(pageTree);
    const targetPath = landing.targetPath;
    const selectedPageId = payload.resolved_page_id;

    if (!selectedPageId) {
      // Requested slug/page_id was invisible or missing: never write empty payload.tabs
      // onto landing.selectedPageId. If a default landing page exists, let fallback
      // hydrate that landing page cleanly using the already-seeded pageTree cache.
      return {
        hydrated: !landing.selectedPageId,
        targetPath,
        prefetchedCodeEntries: []
      };
    }

    const resolvedTargetPath =
      parsedRoute && parsedRoute.kind !== 'home'
        ? (targetPathFromResolvedSelection(pageTree, selectedPageId) ??
          targetPath)
        : targetPath;

    const tabsKey = frontstagePageTabsQueryKey(workspaceId, selectedPageId);
    if (Array.isArray(payload.tabs)) {
      seedStartupQuery(queryClient, tabsKey, payload.tabs);
    }

    if (payload.page_detail) {
      seedStartupPageContent(
        queryClient,
        workspaceId,
        selectedPageId,
        mapFrontstagePageContent(payload.page_detail),
        parsedRoute?.tabRef
      );
    }

    const resolvedTabId = payload.resolved_tab_id;
    let prefetchedCodeEntries: ReadonlyArray<StartupPrefetchedBlockCodeEntry> =
      [];
    if (resolvedTabId && Array.isArray(payload.block_roots)) {
      const rootsKey = frontstageBlockTreeQueryKeys.roots(
        workspaceId,
        selectedPageId,
        { tab_id: resolvedTabId }
      );
      seedStartupQuery(queryClient, rootsKey, payload.block_roots);

      if (Array.isArray(payload.block_codes)) {
        prefetchedCodeEntries = seedStartupPrefetchedBlockNodeCodes({
          actorId,
          workspaceId,
          pageId: selectedPageId,
          codes: payload.block_codes
        });
      }
    }

    return {
      hydrated: true,
      targetPath: resolvedTargetPath,
      prefetchedCodeEntries
    };
  } catch {
    return { hydrated: false, targetPath: '/', prefetchedCodeEntries: [] };
  }
}

function startupQuery(queryClient: QueryClient, queryKey: QueryKey) {
  const options = {
    queryKey,
    staleTime: FRONTSTAGE_STARTUP_DATA_STALE_TIME_MS
  };
  queryClient.setQueryDefaults(queryKey, { staleTime: options.staleTime });
  return options;
}

function seedStartupQuery<T>(queryClient: QueryClient, key: QueryKey, data: T) {
  queryClient.setQueryData<T>(
    startupQuery(queryClient, key).queryKey,
    () => data
  );
}

function seedStartupPageContent(
  queryClient: QueryClient,
  workspaceId: string,
  pageId: string,
  content: ReturnType<typeof mapFrontstagePageContent>,
  tabRef?: string | null,
  fetchedTabRef?: string
) {
  const { id } = content.tab;
  for (const key of new Set([id, tabRef])) {
    if (key && key !== fetchedTabRef) {
      seedStartupQuery(
        queryClient,
        frontstagePageContentQueryKey(workspaceId, pageId, key),
        content
      );
    }
  }
}

function targetPathFromResolvedSelection(
  nodes: readonly FrontstagePageTreeNode[],
  pageId: string
): string | null {
  for (const rootNode of nodes) {
    if (rootNode.placement !== 'topbar' || !rootNode.slug) continue;
    const encodedSlug = encodeURIComponent(rootNode.slug);
    if (rootNode.kind === 'page' && rootNode.id === pageId) {
      return `/${encodedSlug}`;
    }
    if (containsPageNode(rootNode.children, pageId)) {
      return `/${encodedSlug}/pages/${encodeURIComponent(pageId)}`;
    }
  }
  return null;
}

function containsPageNode(
  nodes: readonly FrontstagePageTreeNode[],
  pageId: string
): boolean {
  for (const node of nodes) {
    if (node.kind === 'page' && node.id === pageId) return true;
    if (node.children.length > 0 && containsPageNode(node.children, pageId)) {
      return true;
    }
  }
  return false;
}

/**
 * Single authoritative Frontstage startup control-plane owner shared across
 * `prewarmAuthenticatedStartup` (session discovery), `startFrontstageStartupPrefetch`
 * (`AuthenticatedStartup` `useLayoutEffect`), and `prepareAuthenticatedStartupMount`
 * (`AuthenticatedStartup` `useEffect`).
 *
 * Uses 1-RTT `GET /api/console/frontstage/bootstrap` first and falls back to
 * stepwise `/pages -> /tabs -> /document + /blocks -> /code` only on error.
 */
function ensureFrontstageStartupHydration({
  queryClient,
  actor,
  me,
  scope,
  pathname,
  isCurrent,
  preloadWorkspaceRoute,
  preloadHomeRoute,
  deferRoutePreloads = false,
  deferSupportingCatalogs = false,
  useBootstrapEndpoint = true
}: {
  queryClient: QueryClient;
  actor: AuthSnapshot['actor'];
  me: NonNullable<AuthSnapshot['me']>;
  scope: string;
  pathname: string;
  isCurrent: () => boolean;
  preloadWorkspaceRoute: () => Promise<unknown>;
  preloadHomeRoute: () => Promise<unknown>;
  deferRoutePreloads?: boolean;
  deferSupportingCatalogs?: boolean;
  useBootstrapEndpoint?: boolean;
}): Promise<FrontstageStartupHydrationResult> {
  const effectivePathname = normalizeAuthenticatedStartupPathname(pathname);
  const workspaceId = actor.current_workspace_id;
  if (!workspaceId || !isFrontstageCandidatePathname(effectivePathname)) {
    return Promise.resolve({
      targetPath: workspaceId ? effectivePathname : '/me',
      prefetchedCodeEntries: []
    });
  }

  const initialKey = buildHydrationKey(
    scope,
    workspaceId,
    actor.id,
    effectivePathname
  );
  const existing = activeHydrationFlights.get(queryClient);
  let preloadedRoutes = false;
  const triggerRoutePreloads = () => {
    if (preloadedRoutes || !isCurrent()) return;
    preloadedRoutes = true;
    void preloadWorkspaceRoute().catch(() => undefined);
    if (effectivePathname === '/') {
      void preloadHomeRoute().catch(() => undefined);
    }
  };
  if (!deferRoutePreloads) triggerRoutePreloads();
  if (existing?.keys.has(initialKey)) {
    if (!preloadedRoutes) void existing.promise.finally(triggerRoutePreloads);
    return existing.promise;
  }

  const flushSupportingCatalogs = seedSupportingFrontstageQueries({
    queryClient,
    workspaceId,
    actorId: actor.id,
    me,
    deferSupportingCatalogs
  });

  const parsedRoute = parseFrontstagePathname(effectivePathname);
  if (!parsedRoute) {
    flushSupportingCatalogs();
    triggerRoutePreloads();
    return Promise.resolve({
      targetPath: effectivePathname,
      prefetchedCodeEntries: []
    });
  }

  const prewarmRuntime = (
    entries: ReadonlyArray<StartupPrefetchedBlockCodeEntry>
  ) => {
    if (!entries.length || import.meta.env.VITEST || !isCurrent()) return;
    void import('../../features/frontstage/lib/runtime-cache/block-code-prefetch')
      .then(({ prewarmStartupBlockRuntime }) => {
        if (isCurrent()) prewarmStartupBlockRuntime(entries);
      })
      .catch(() => undefined);
  };

  const keys = new Set<string>([initialKey]);
  const promise = (async (): Promise<FrontstageStartupHydrationResult> => {
    try {
      if (useBootstrapEndpoint) {
        const bootstrapResult = await tryHydrateFromBootstrapEndpoint({
          queryClient,
          actorId: actor.id,
          workspaceId,
          role: actor.effective_display_role ?? '',
          locale: me.preferred_locale ?? 'default',
          scope,
          parsedRoute,
          isCancelled: () => !isCurrent()
        });
        if (bootstrapResult.hydrated) {
          keys.add(
            buildHydrationKey(
              scope,
              workspaceId,
              actor.id,
              bootstrapResult.targetPath
            )
          );
          prewarmRuntime(bootstrapResult.prefetchedCodeEntries);
          return {
            targetPath: bootstrapResult.targetPath,
            prefetchedCodeEntries: bootstrapResult.prefetchedCodeEntries
          };
        }
      }

      const pageTree = await queryClient.fetchQuery(
        frontstagePageTreeQueryOptions(workspaceId, scope)
      );
      if (!isCurrent()) {
        return { targetPath: effectivePathname, prefetchedCodeEntries: [] };
      }
      const landing = resolveCanonicalFrontstageLanding(pageTree);
      const selectedPageId =
        resolvePrefetchTargetPage(pageTree, parsedRoute) ??
        landing.selectedPageId;
      const targetPath =
        (selectedPageId
          ? targetPathFromResolvedSelection(pageTree, selectedPageId)
          : null) ?? landing.targetPath;
      keys.add(buildHydrationKey(scope, workspaceId, actor.id, targetPath));

      if (!selectedPageId) {
        return { targetPath, prefetchedCodeEntries: [] };
      }

      const tabsKey = frontstagePageTabsQueryKey(workspaceId, selectedPageId);
      const tabs = await queryClient.fetchQuery({
        ...startupQuery(queryClient, tabsKey),
        queryFn: () => fetchFrontstagePageTabs(workspaceId, selectedPageId)
      });
      if (!isCurrent()) {
        return { targetPath, prefetchedCodeEntries: [] };
      }

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

      const dataFlights: Array<Promise<unknown>> = [];
      if (tabReference) {
        const contentKey = frontstagePageContentQueryKey(
          workspaceId,
          selectedPageId,
          tabReference
        );
        dataFlights.push(
          queryClient
            .fetchQuery({
              ...startupQuery(queryClient, contentKey),
              queryFn: () =>
                fetchFrontstagePageContent(
                  workspaceId,
                  selectedPageId,
                  tabReference
                )
            })
            .then((content) => {
              if (isCurrent()) {
                seedStartupPageContent(
                  queryClient,
                  workspaceId,
                  selectedPageId,
                  content,
                  parsedRoute.tabRef,
                  tabReference
                );
              }
            })
        );
      }

      let prefetchedCodeEntries: ReadonlyArray<StartupPrefetchedBlockCodeEntry> =
        [];
      if (matchedTab?.id) {
        const rootsKey = frontstageBlockTreeQueryKeys.roots(
          workspaceId,
          selectedPageId,
          { tab_id: matchedTab.id }
        );
        dataFlights.push(
          queryClient
            .fetchQuery({
              ...startupQuery(queryClient, rootsKey),
              queryFn: () =>
                fetchFrontstageBlockRoots(workspaceId, selectedPageId, {
                  tab_id: matchedTab.id
                })
            })
            .then((roots) => {
              if (!isCurrent()) return;
              const blockIds = extractRootBlockIds(roots);
              if (blockIds.length === 0) return;
              prefetchedCodeEntries = prefetchStartupBlockCodeEntries({
                actorId: actor.id,
                workspaceId,
                pageId: selectedPageId,
                blockIds
              });
              prewarmRuntime(prefetchedCodeEntries);
            })
        );
      }

      await Promise.allSettled(dataFlights);
      return { targetPath, prefetchedCodeEntries };
    } finally {
      if (isCurrent()) {
        flushSupportingCatalogs();
        triggerRoutePreloads();
      }
    }
  })();

  const record: ActiveHydrationRecord = { keys, promise };
  activeHydrationFlights.set(queryClient, record);
  return promise;
}

function createPrewarmedSlot(snapshot: AuthSnapshot): {
  slot: PrewarmedAuthenticatedClientSlot;
  scope: string;
} {
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
  return { slot, scope };
}

export interface PrepareAuthenticatedStartupMountOptions {
  getPathname?: () => string;
  controlTimeoutMs?: number;
  concurrencyProfile?: RuntimeConcurrencyProfile;
  preloadShellAndRoute?: (targetPath: string) => Promise<void>;
  preloadWorkspaceRoute?: () => Promise<unknown>;
  preloadHomeRoute?: () => Promise<unknown>;
  useBootstrapEndpoint?: boolean;
}

export async function prepareAuthenticatedStartupMount(
  queryClient: QueryClient,
  isCancelled: () => boolean,
  options: PrepareAuthenticatedStartupMountOptions = {}
): Promise<{ targetPath: string }> {
  const state = useAuthStore.getState();
  if (state.sessionStatus !== 'authenticated' || !state.actor) {
    return { targetPath: '/sign-in' };
  }

  const profile =
    options.concurrencyProfile ?? resolveRuntimeConcurrencyProfile();
  const rawPathname =
    options.getPathname?.() ??
    (typeof window !== 'undefined' ? window.location.pathname : '/');
  let targetPathname = normalizeAuthenticatedStartupPathname(rawPathname);
  const locale = resolveAppLocale(resolveAuthenticatedLocale(state.me));
  const scope = selectNavigationQueryScope(state);
  const isFrontstageTarget = isFrontstageCandidatePathname(targetPathname);
  const preloadWorkspaceRoute =
    options.preloadWorkspaceRoute ??
    (() => loadFrontstageWorkspacePageModule());
  const preloadHomeRoute =
    options.preloadHomeRoute ?? (() => import('../../routes/HomeRedirect'));

  prefetchSettingsConsoleNavigationWithEarlyHandoff(
    queryClient,
    state.actor,
    scope
  );

  // SRPT Two-Stage Static Loading in PROD:
  // Stage-A (`preloadAuthenticatedStartupRuntime`: shell-core + active locale dictionary +
  // frontstage-shell) enters the HTTP/2 pipe FIRST alongside the 1-RTT bootstrap API so tiny
  // ~0.6 kB i18n and JSON responses never queue behind 160 kB `native-host`.
  // Stage-B (`preloadWorkspaceRoute`: frontstage-canvas + native-host) chains immediately
  // after Stage-A settles. Compiler Worker stays 100% lazy until an `artifactCache` miss.
  let parallelRuntimeFlight: Promise<unknown> | null = null;
  if (profile.parallelStartupAssetAndControl) {
    parallelRuntimeFlight = options.preloadShellAndRoute
      ? options.preloadShellAndRoute(targetPathname)
      : preloadAuthenticatedStartupRuntime(locale, targetPathname);
    if (isFrontstageTarget) {
      void parallelRuntimeFlight.finally(() => {
        if (!isCancelled()) {
          void preloadWorkspaceRoute().catch(() => undefined);
        }
      });
    }
  }

  if (isFrontstageTarget && state.me) {
    const hydrationFlight = ensureFrontstageStartupHydration({
      queryClient,
      actor: state.actor,
      me: state.me,
      scope,
      pathname: rawPathname,
      isCurrent: () => !isCancelled(),
      preloadWorkspaceRoute,
      preloadHomeRoute,
      deferRoutePreloads: true,
      deferSupportingCatalogs: !profile.parallelStartupAssetAndControl,
      useBootstrapEndpoint: options.useBootstrapEndpoint ?? true
    });
    const settled = await withTimeout(
      hydrationFlight,
      options.controlTimeoutMs ?? DEFAULT_CONTROL_GATE_TIMEOUT_MS
    ).catch(() => null);
    if (isCancelled()) return { targetPath: targetPathname };

    if (settled?.targetPath) {
      if (
        !options.getPathname &&
        typeof window !== 'undefined' &&
        (window.location.pathname === '/' ||
          SIGN_IN_PATHNAME_RE.test(window.location.pathname)) &&
        settled.targetPath !== window.location.pathname
      ) {
        window.history.replaceState(
          window.history.state,
          '',
          settled.targetPath
        );
      }
      targetPathname = settled.targetPath;
    }
  } else if (state.actor.current_workspace_id) {
    await withTimeout(
      queryClient.prefetchQuery(
        frontstagePageTreeQueryOptions(state.actor.current_workspace_id, scope)
      ),
      options.controlTimeoutMs ?? DEFAULT_CONTROL_GATE_TIMEOUT_MS
    ).catch(() => undefined);
    if (isCancelled()) return { targetPath: targetPathname };
  }

  if (options.preloadShellAndRoute) {
    if (parallelRuntimeFlight) {
      await parallelRuntimeFlight;
    } else {
      await options.preloadShellAndRoute(targetPathname);
    }
    return { targetPath: targetPathname };
  }

  if (isFrontstageCandidatePathname(targetPathname) && state.me) {
    void preloadWorkspaceRoute().catch(() => undefined);
  }
  if (parallelRuntimeFlight) {
    await parallelRuntimeFlight;
    return { targetPath: targetPathname };
  }
  await preloadAuthenticatedStartupRuntime(locale, targetPathname);
  return { targetPath: targetPathname };
}

/**
 * Starts authenticated navigation + 1-RTT Frontstage bootstrap as soon as session
 * discovery settles, overlapping network I/O with React/Antd chunk evaluation.
 */
export function prewarmAuthenticatedStartup(
  snapshot: AuthSnapshot,
  options: {
    getPathname?: () => string;
    preloadWorkspaceRoute?: () => Promise<unknown>;
    preloadHomeRoute?: () => Promise<unknown>;
    concurrencyProfile?: RuntimeConcurrencyProfile;
    useBootstrapEndpoint?: boolean;
  } = {}
): QueryClient {
  const profile =
    options.concurrencyProfile ?? resolveRuntimeConcurrencyProfile();
  const { slot, scope } = createPrewarmedSlot(snapshot);
  const { queryClient, workspaceId } = slot;
  const pathname =
    options.getPathname?.() ??
    (typeof window !== 'undefined' ? window.location.pathname : '/');
  const effectivePathname = normalizeAuthenticatedStartupPathname(pathname);

  prefetchSettingsConsoleNavigationWithEarlyHandoff(
    queryClient,
    snapshot.actor,
    scope
  );

  if (
    snapshot.me &&
    workspaceId &&
    isFrontstageCandidatePathname(effectivePathname)
  ) {
    const preloadWorkspaceRoute =
      options.preloadWorkspaceRoute ??
      (() => import('../../features/frontstage/pages/FrontstageWorkspacePage'));
    const preloadHomeRoute =
      options.preloadHomeRoute ?? (() => import('../../routes/HomeRedirect'));
    void ensureFrontstageStartupHydration({
      queryClient,
      actor: snapshot.actor,
      me: snapshot.me,
      scope,
      pathname,
      isCurrent: () => !slot.cancelled,
      preloadWorkspaceRoute,
      preloadHomeRoute,
      deferRoutePreloads:
        !options.preloadWorkspaceRoute && !import.meta.env.VITEST,
      deferSupportingCatalogs:
        !profile.parallelStartupAssetAndControl &&
        !options.preloadWorkspaceRoute &&
        !import.meta.env.VITEST,
      useBootstrapEndpoint: options.useBootstrapEndpoint ?? true
    });
  } else if (workspaceId) {
    void queryClient.prefetchQuery(
      frontstagePageTreeQueryOptions(workspaceId, scope)
    );
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
  clearStartupPrefetchedBlockNodeCodes();
  if (!prewarmedSlot) return;
  const slot = prewarmedSlot;
  prewarmedSlot = null;
  slot.cancelled = true;
  activeHydrationFlights.delete(slot.queryClient);
  slot.queryClient.clear();
}

export function startFrontstageStartupPrefetch(
  queryClient: QueryClient,
  options: {
    getPathname?: () => string;
    preloadWorkspaceRoute?: () => Promise<unknown>;
    preloadHomeRoute?: () => Promise<unknown>;
    concurrencyProfile?: RuntimeConcurrencyProfile;
    useBootstrapEndpoint?: boolean;
  } = {}
): () => void {
  const profile =
    options.concurrencyProfile ?? resolveRuntimeConcurrencyProfile();
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
    const rawPathname = getPathname();
    const effectivePathname =
      normalizeAuthenticatedStartupPathname(rawPathname);
    if (
      state.sessionStatus !== 'authenticated' ||
      !state.actor ||
      !state.me ||
      !isFrontstageCandidatePathname(effectivePathname)
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
      effectivePathname
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

    void ensureFrontstageStartupHydration({
      queryClient,
      actor,
      me,
      scope,
      pathname: rawPathname,
      isCurrent,
      preloadWorkspaceRoute,
      preloadHomeRoute,
      deferRoutePreloads:
        !profile.parallelStartupAssetAndControl &&
        !options.preloadWorkspaceRoute &&
        !import.meta.env.VITEST,
      deferSupportingCatalogs:
        !profile.parallelStartupAssetAndControl &&
        !options.preloadWorkspaceRoute &&
        !import.meta.env.VITEST,
      useBootstrapEndpoint: options.useBootstrapEndpoint ?? true
    });
  };

  const unsubscribe = useAuthStore.subscribe(synchronize);
  synchronize();
  return () => {
    activeGeneration += 1;
    unsubscribe();
  };
}
