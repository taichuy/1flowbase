import { useCallback, useSyncExternalStore } from 'react';

import { evaluateConsoleAccess } from '@1flowbase/api-client/auth';

import { useAuthStore } from './auth-store';

type AccessStatus = 'loading' | 'allowed' | 'denied';
type AuthState = ReturnType<typeof useAuthStore.getState>;
type Pending = {
  promise: Promise<boolean>;
  resolve: (allowed: boolean) => void;
};
type Scope = {
  authKey: string;
  key: string;
  base: string[];
  values: Map<string, boolean>;
  pending: Map<string, Pending>;
  failedUntil: Map<string, number>;
  running: boolean;
};

const FAILURE_COOLDOWN_MS = 5_000;
let scope: Scope | undefined;
let version = 0;
const listeners = new Set<() => void>();

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function publish() {
  version += 1;
  listeners.forEach((listener) => listener());
}

function authScope(state: AuthState) {
  const { actor, sessionStatus, me } = state;
  return sessionStatus === 'authenticated' && actor
    ? [
        actor.id,
        actor.current_workspace_id,
        actor.effective_display_role,
        me?.policy_fingerprint ?? ''
      ]
    : undefined;
}

function syncScope(state: AuthState) {
  const base = authScope(state);
  const authKey = base?.join('|');
  if (authKey === scope?.authKey) return;
  scope?.pending.forEach(({ resolve }) => resolve(false));
  scope =
    base && authKey !== undefined
      ? {
          base,
          authKey,
          key: authKey,
          values: new Map(),
          pending: new Map(),
          failedUntil: new Map(),
          running: false
        }
      : undefined;
  publish();
}

useAuthStore.subscribe(syncScope);
syncScope(useAuthStore.getState());

function schedule(current: Scope) {
  if (current.running) return;
  current.running = true;
  queueMicrotask(async () => {
    const csrfToken = useAuthStore.getState().csrfToken;
    if (scope !== current) return;
    const codes = [...current.pending.keys()];
    try {
      if (!csrfToken) throw new Error('missing console CSRF token');
      const response = await evaluateConsoleAccess(
        codes,
        csrfToken,
        import.meta.env.VITE_API_BASE_URL
      );
      if (scope !== current) return;
      const key = [
        ...current.base.slice(0, 3),
        response.policy_fingerprint
      ].join('|');
      if (current.key !== key) {
        current.key = key;
        current.values.clear();
        current.failedUntil.clear();
      }
      for (const code of codes) {
        const allowed =
          Object.hasOwn(response.allowed, code) &&
          response.allowed[code] === true;
        current.values.set(code, allowed);
        current.failedUntil.delete(code);
        current.pending.get(code)?.resolve(allowed);
      }
    } catch {
      if (scope !== current) return;
      for (const code of codes) {
        current.failedUntil.set(code, Date.now() + FAILURE_COOLDOWN_MS);
        current.pending.get(code)?.resolve(false);
      }
    } finally {
      for (const code of codes) current.pending.delete(code);
      current.running = false;
      if (scope === current) {
        if (current.pending.size) schedule(current);
        publish();
      }
    }
  });
}

function request(code: string): Promise<boolean> {
  const current = scope;
  if (!current || !code.trim()) return Promise.resolve(false);
  const cached = current.values.get(code);
  if (cached !== undefined) return Promise.resolve(cached);
  const pending = current.pending.get(code);
  if (pending) return pending.promise;
  if (!useAuthStore.getState().csrfToken) return Promise.resolve(false);
  if ((current.failedUntil.get(code) ?? 0) > Date.now())
    return Promise.resolve(false);
  let resolve!: Pending['resolve'];
  const promise = new Promise<boolean>((settle) => {
    resolve = settle;
  });
  current.pending.set(code, { promise, resolve });
  schedule(current);
  return promise;
}

function readStatus(code: string, projected = false): AccessStatus {
  if (!code.trim()) return 'denied';
  if (!scope)
    return useAuthStore.getState().sessionStatus === 'unknown'
      ? 'loading'
      : 'denied';
  if (!projected && scope.base[2] === 'root') return 'allowed';
  const cached = scope.values.get(code);
  if (cached !== undefined) return cached ? 'allowed' : 'denied';
  void request(code);
  return 'loading';
}

const getVersion = () => version;

function useAccess(projected: boolean) {
  const revision = useSyncExternalStore(subscribe, getVersion, getVersion);
  // The callback identity also invalidates consumers' memoized actions and effects.
  return useCallback(
    (code: string) => readStatus(code, projected) === 'allowed',
    [projected, revision]
  );
}

export function useConsoleOperationAccess() {
  return useAccess(false);
}

/** Host-scoped capabilities must be evaluated even for root. */
export function useProjectedConsoleOperationAccess() {
  return useAccess(true);
}

export function useConsoleOperationAccessStatus(code: string): AccessStatus {
  // Unknown and anonymous both have no cache scope, but differ for loading UI.
  useAuthStore((state) => state.sessionStatus);
  useSyncExternalStore(subscribe, getVersion, getVersion);
  return readStatus(code);
}

export function hasCachedConsoleOperationAccess(code: string): boolean {
  return Boolean(
    code.trim() &&
    scope &&
    (scope.base[2] === 'root' || scope.values.get(code) === true)
  );
}

export async function ensureConsoleOperationAccess(
  codes: string[]
): Promise<Record<string, boolean>> {
  const current = scope;
  const unique = [...new Set(codes.filter((code) => code.trim()))];
  if (current?.base[2] !== 'root') {
    await Promise.all(unique.map(request));
  }
  // A missing code can return a newer fingerprint while others were cached.
  return Object.fromEntries(
    unique.map((code) => [
      code,
      scope === current && hasCachedConsoleOperationAccess(code)
    ])
  );
}

export function seedConsoleOperationAccessForTests(
  values: Record<string, boolean>
) {
  for (const [code, allowed] of Object.entries(values))
    scope?.values.set(code, allowed);
  publish();
}

export function resetConsoleOperationAccessCache() {
  scope?.pending.forEach(({ resolve }) => resolve(false));
  scope = undefined;
  syncScope(useAuthStore.getState());
  publish();
}
