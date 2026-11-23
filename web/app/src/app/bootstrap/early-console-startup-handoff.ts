import {
  clearEarlyConsoleStartupState,
  earlyStartupShared
} from '../../features/auth/api/early-console-startup-flight';
import { getAuthApiBaseUrl } from '../../features/auth/api/session';

export const DEFAULT_BOOTSTRAP_VIEWPORT_CODE_LIMIT = 12;
export const resolveEarlyBootstrapPath = earlyStartupShared.path;

const SNAP_PREFIX = '1fb:bs:';
const UNSUP_PREFIX = '1fb:bsu:';
const HANDOFF_TTL_MS = 15_000;
const NON_FRONTSTAGE_ROOT_RE =
  /^(?:api|v1|sign-in|mcp|me|settings|applications|models|plugins|files|notifications|assistant|terminal)$/u;

export interface EarlyBootstrapQuery {
  slug?: string;
  page_id?: string;
  tab_reference?: string;
  limit?: number;
  code_limit?: number;
}

interface SnapshotEntry<T = unknown> {
  e: string;
  p: T;
}

const snapshots = new Map<string, SnapshotEntry>();
const unsupported = new Set<string>();

export function parseEarlyFrontstageBootstrapQuery(
  rawPathname: string
): EarlyBootstrapQuery | null {
  const parts = (/^\/sign-in\/?$/u.test(rawPathname) ? '/' : rawPathname || '/')
    .split('/')
    .filter(Boolean);
  if (parts.length === 0) {
    return { code_limit: DEFAULT_BOOTSTRAP_VIEWPORT_CODE_LIMIT };
  }
  try {
    const slug = decodeURIComponent(parts[0]!);
    if (!slug || NON_FRONTSTAGE_ROOT_RE.test(slug)) return null;
    if (parts.length === 1) {
      return { slug, code_limit: DEFAULT_BOOTSTRAP_VIEWPORT_CODE_LIMIT };
    }
    if (parts.length === 3 && (parts[1] === 'pages' || parts[1] === 'tabs')) {
      const val = decodeURIComponent(parts[2]!);
      if (!val) return null;
      return parts[1] === 'pages'
        ? { slug, page_id: val, code_limit: DEFAULT_BOOTSTRAP_VIEWPORT_CODE_LIMIT }
        : { slug, tab_reference: val, code_limit: DEFAULT_BOOTSTRAP_VIEWPORT_CODE_LIMIT };
    }
    if (parts.length === 5 && parts[1] === 'pages' && parts[3] === 'tabs') {
      const page_id = decodeURIComponent(parts[2]!);
      const tab_reference = decodeURIComponent(parts[4]!);
      return page_id && tab_reference
        ? { slug, page_id, tab_reference, code_limit: DEFAULT_BOOTSTRAP_VIEWPORT_CODE_LIMIT }
        : null;
    }
  } catch {
    return null;
  }
  return null;
}

export function buildBootstrapRequestPath(query: EarlyBootstrapQuery): string {
  const p = new URLSearchParams();
  if (query.slug) p.set('slug', query.slug);
  if (query.page_id) p.set('page_id', query.page_id);
  if (query.tab_reference) p.set('tab_reference', query.tab_reference);
  if (query.limit !== undefined) p.set('limit', String(query.limit));
  if (query.code_limit !== undefined) p.set('code_limit', String(query.code_limit));
  const qs = p.toString();
  return qs ? `/api/console/frontstage/bootstrap?${qs}` : '/api/console/frontstage/bootstrap';
}

export function isBootstrapEndpointMarkedUnsupported(
  baseUrl: string,
  role: string
): boolean {
  const key = `${UNSUP_PREFIX}${baseUrl}:${role}`;
  if (unsupported.has(key)) return true;
  if (earlyStartupShared.store()?.getItem(key) === '1') {
    unsupported.add(key);
    return true;
  }
  return false;
}

export function markBootstrapEndpointUnsupported(
  baseUrl: string,
  role: string
): void {
  const key = `${UNSUP_PREFIX}${baseUrl}:${role}`;
  unsupported.add(key);
  try {
    earlyStartupShared.store()?.setItem(key, '1');
  } catch {
    // ignore quota errors
  }
}

export function clearCachedBootstrapSnapshots(): void {
  snapshots.clear();
  unsupported.clear();
  clearEarlyConsoleStartupState();
}

function unwrap<T>(body: unknown): T {
  return body && typeof body === 'object' && 'data' in body
    ? (body as { data: T }).data
    : (body as T);
}

function findRootSlugForPage(nodes: unknown, pageId: string): string | null {
  if (!Array.isArray(nodes)) return null;
  const hasPage = (items: unknown[]): boolean =>
    items.some((n) => {
      if (!n || typeof n !== 'object') return false;
      const rec = n as { id?: unknown; kind?: unknown; children?: unknown };
      return (
        (rec.kind === 'page' && rec.id === pageId) ||
        (Array.isArray(rec.children) && hasPage(rec.children))
      );
    });
  for (const raw of nodes) {
    if (!raw || typeof raw !== 'object') continue;
    const root = raw as {
      id?: unknown;
      kind?: unknown;
      placement?: unknown;
      slug?: unknown;
      children?: unknown;
    };
    if (root.placement !== 'topbar' || typeof root.slug !== 'string' || !root.slug) {
      continue;
    }
    if (
      (root.kind === 'page' && root.id === pageId) ||
      (Array.isArray(root.children) && hasPage(root.children))
    ) {
      return root.slug;
    }
  }
  return null;
}

function writeSingleSnapshot<T>(cacheKey: string, entry: SnapshotEntry<T>, raw: string | null): void {
  const store = earlyStartupShared.store();
  snapshots.set(cacheKey, entry);
  if (snapshots.size > 6) {
    const oldest = snapshots.keys().next().value;
    if (oldest) {
      snapshots.delete(oldest);
      store?.removeItem(oldest);
    }
  }
  if (store && raw) {
    try {
      store.setItem(cacheKey, raw);
    } catch {
      // ignore quota errors
    }
  }
}

function persistSnapshot<T>(cacheKey: string, etag: string, payload: T): void {
  const entry: SnapshotEntry<T> = { e: etag, p: payload };
  let raw: string | null = null;
  try {
    const serialized = JSON.stringify(entry);
    if (serialized.length <= 512_000) raw = serialized;
  } catch {
    // ignore serialization errors
  }
  writeSingleSnapshot(cacheKey, entry, raw);

  // Also index under canonical `/:slug` and `/:slug/pages/:resolved_page_id` paths
  // because backend ETag hashes resolved `(page_id, tab_id)` and router normalizes URLs.
  const marker = ':/api/console/frontstage/bootstrap';
  const markerIdx = cacheKey.indexOf(marker);
  if (markerIdx > 0 && payload && typeof payload === 'object') {
    const prefix = cacheKey.slice(0, markerIdx + 1);
    const resolvedPageId = (payload as { resolved_page_id?: unknown }).resolved_page_id;
    const pages = (payload as { pages?: unknown }).pages;
    if (typeof resolvedPageId === 'string' && resolvedPageId) {
      const slug = findRootSlugForPage(pages, resolvedPageId);
      if (slug) {
        const canonicalPaths = [
          buildBootstrapRequestPath({
            slug,
            code_limit: DEFAULT_BOOTSTRAP_VIEWPORT_CODE_LIMIT
          }),
          buildBootstrapRequestPath({
            slug,
            page_id: resolvedPageId,
            code_limit: DEFAULT_BOOTSTRAP_VIEWPORT_CODE_LIMIT
          })
        ];
        for (const p of canonicalPaths) {
          const aliasKey = `${prefix}${p}`;
          if (aliasKey !== cacheKey) {
            writeSingleSnapshot(aliasKey, entry, raw);
          }
        }
      }
    }
  }
}

async function settleBootstrapResponse<T>(
  res: Response,
  baseUrl: string,
  role: string,
  cacheKey: string,
  cachedPayload: T | undefined
): Promise<T> {
  if (res.status === 304 && cachedPayload !== undefined) {
    return cachedPayload;
  }
  if (res.status === 404 || res.status === 403) {
    markBootstrapEndpointUnsupported(baseUrl, role);
    throw new Error(`Bootstrap HTTP ${res.status}`);
  }
  if (!res.ok) {
    throw Object.assign(new Error(`Bootstrap HTTP ${res.status}`), {
      status: res.status
    });
  }
  const payload = unwrap<T>(await res.json());
  const etag = res.headers.get('etag');
  if (etag && payload) {
    persistSnapshot(cacheKey, etag, payload);
  }
  return payload;
}

export async function fetchWithConditionalBootstrapCache<T>({
  actorId,
  workspaceId,
  role,
  locale,
  query,
  baseUrl = getAuthApiBaseUrl()
}: {
  actorId: string;
  workspaceId: string;
  role: string;
  locale: string;
  query: EarlyBootstrapQuery;
  baseUrl?: string;
}): Promise<T> {
  if (isBootstrapEndpointMarkedUnsupported(baseUrl, role)) {
    throw new Error('frontstage_bootstrap_endpoint_unsupported');
  }
  const requestPath = buildBootstrapRequestPath(query);
  const cacheKey = `${SNAP_PREFIX}${actorId}:${workspaceId}:${role}:${locale}:${requestPath}`;
  let cached = snapshots.get(cacheKey) as SnapshotEntry<T> | undefined;
  if (!cached) {
    const store = earlyStartupShared.store();
    if (store) {
      try {
        const raw = store.getItem(cacheKey);
        if (raw) {
          const parsed = JSON.parse(raw) as SnapshotEntry<T>;
          if (parsed?.e && parsed.p) {
            cached = parsed;
            snapshots.set(cacheKey, parsed);
          }
        }
      } catch {
        // ignore corrupt cache
      }
    }
  }
  const headers: Record<string, string> = cached?.e
    ? { 'if-none-match': cached.e }
    : {};
  const res = await fetch(`${baseUrl}${requestPath}`, {
    method: 'GET',
    credentials: 'include',
    headers,
    cache: 'no-store'
  });
  return settleBootstrapResponse<T>(res, baseUrl, role, cacheKey, cached?.p);
}

export function consumeEarlyConsoleNavigationFlight<T>(
  actorId: string,
  workspaceId: string | null | undefined,
  role: string
): Promise<T> | null {
  const record = earlyStartupShared.peek();
  if (!record || Date.now() - record.at > HANDOFF_TTL_MS) return null;
  if (record.sc !== `${actorId}:${workspaceId ?? ''}:${role}` || !record.nav) {
    return null;
  }
  const rawNav = record.nav;
  record.nav = null;
  return rawNav.then(async (res) => {
    if (!res.ok) throw new Error(`Navigation HTTP ${res.status}`);
    return unwrap<T>(await res.json());
  });
}

export function consumeEarlyFrontstageBootstrapFlight<T>(
  actorId: string,
  workspaceId: string,
  role: string,
  query: EarlyBootstrapQuery
): Promise<T> | null {
  const record = earlyStartupShared.peek();
  if (!record || Date.now() - record.at > HANDOFF_TTL_MS) return null;
  const expectedPath = buildBootstrapRequestPath(query);
  if (
    record.sc !== `${actorId}:${workspaceId}:${role}` ||
    record.path !== expectedPath ||
    !record.bs ||
    !record.key
  ) {
    return null;
  }
  const rawBs = record.bs;
  const cacheKey = record.key;
  const cachedPayload = record.val as T | undefined;
  const baseUrl = record.url;
  record.bs = null;
  record.path = null;
  return rawBs.then((res) =>
    settleBootstrapResponse<T>(res, baseUrl, role, cacheKey, cachedPayload)
  );
}
