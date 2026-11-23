import type { AuthSnapshot } from '../../../state/auth-store';
import { getAuthApiBaseUrl } from './session';

// Minimal zero-AntD early producer inside `auth-anonymous`; consumer/LRU lives in `shell-core`.
const NON_FS_RE =
  /^(?:api|v1|sign-in|mcp|me|settings|applications|models|plugins|files|notifications|assistant|terminal)$/u;

export interface EarlyStartupFlightRecord {
  at: number;
  sc: string;
  url: string;
  key: string | null;
  val: unknown;
  path: string | null;
  bs: Promise<Response> | null;
  nav: Promise<Response> | null;
}

let activeRecord: EarlyStartupFlightRecord | null = null;

function store(): Storage | null {
  try {
    return typeof sessionStorage !== 'undefined' ? sessionStorage : null;
  } catch {
    return null;
  }
}

function resolvePath(raw: string): string | null {
  const parts = (/^\/sign-in\/?$/u.test(raw) ? '/' : raw || '/')
    .split('/')
    .filter(Boolean);
  const p = new URLSearchParams();
  if (parts.length) {
    try {
      const slug = decodeURIComponent(parts[0]!);
      if (!slug || NON_FS_RE.test(slug)) return null;
      p.set('slug', slug);
      if (parts.length === 3 && (parts[1] === 'pages' || parts[1] === 'tabs')) {
        const v = decodeURIComponent(parts[2]!);
        if (!v) return null;
        p.set(parts[1] === 'pages' ? 'page_id' : 'tab_reference', v);
      } else if (parts.length === 5 && parts[1] === 'pages' && parts[3] === 'tabs') {
        const pid = decodeURIComponent(parts[2]!);
        const tid = decodeURIComponent(parts[4]!);
        if (!pid || !tid) return null;
        p.set('page_id', pid);
        p.set('tab_reference', tid);
      } else if (parts.length !== 1) return null;
    } catch {
      return null;
    }
  }
  p.set('code_limit', '12');
  return `/api/console/frontstage/bootstrap?${p}`;
}

export function clearEarlyConsoleStartupState(): void {
  activeRecord = null;
  const s = store();
  if (!s) return;
  const rm: string[] = [];
  for (let i = 0; i < s.length; i += 1) {
    const k = s.key(i);
    if (k?.startsWith('1fb:bs')) rm.push(k);
  }
  for (const k of rm) s.removeItem(k);
}

export function startEarlyConsoleStartupFlight(
  snapshot: AuthSnapshot,
  pathnameOverride?: string
): void {
  if (import.meta.env.VITEST || typeof fetch !== 'function') return;
  const wid = snapshot.actor.current_workspace_id;
  const role = snapshot.actor.effective_display_role ?? '';
  const sc = `${snapshot.actor.id}:${wid ?? ''}:${role}`;
  const url = getAuthApiBaseUrl();
  const s = store();
  const nav = fetch(`${url}/api/console/navigation`, { credentials: 'include' });
  void nav.catch(() => undefined);

  let path: string | null = null;
  let key: string | null = null;
  let val: unknown = null;
  let bs: Promise<Response> | null = null;
  if (wid && snapshot.me && s?.getItem(`1fb:bsu:${url}:${role}`) !== '1') {
    path = resolvePath(
      pathnameOverride ?? (typeof window !== 'undefined' ? window.location.pathname : '/')
    );
    if (path) {
      key = `1fb:bs:${sc}:${snapshot.me.preferred_locale ?? 'default'}:${path}`;
      let headers: Record<string, string> | undefined;
      try {
        const raw = s?.getItem(key);
        if (raw) {
          const j = JSON.parse(raw) as { e?: string; p?: unknown };
          if (j?.e && j.p) {
            headers = { 'if-none-match': j.e };
            val = j.p;
          }
        }
      } catch {
        // ignore
      }
      bs = fetch(`${url}${path}`, { credentials: 'include', headers, cache: 'no-store' });
      void bs.catch(() => undefined);
    }
  }
  activeRecord = { at: Date.now(), sc, url, key, val, path, bs, nav };
}

export const earlyStartupShared = {
  store,
  path: resolvePath,
  peek: () => activeRecord
};
