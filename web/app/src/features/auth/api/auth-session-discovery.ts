import {
  takeDocumentSessionDiscovery,
  discardDocumentSessionDiscovery
} from './document-start-discovery';
import {
  clearEarlyConsoleStartupState,
  startEarlyConsoleStartupFlight
} from './early-console-startup-flight';
import type { AuthSnapshot } from '../../../state/auth-store';
import { fetchCurrentMe, fetchCurrentSession } from './session';

export type AuthSessionDiscovery =
  | { status: 'authenticated'; snapshot: AuthSnapshot }
  | { status: 'anonymous' };

let discoveryFlight: Promise<AuthSessionDiscovery> | undefined;
let resetPrewarmCallback: (() => void) | undefined;

export function startAuthSessionDiscovery(): Promise<AuthSessionDiscovery> {
  discoveryFlight ??= discoverAuthSession();
  return discoveryFlight;
}

export function resetAuthSessionDiscovery() {
  discoveryFlight = undefined;
  resetPrewarmCallback?.();
  clearEarlyConsoleStartupState();
  discardDocumentSessionDiscovery();
}

function finishAuthenticated(
  session: Awaited<ReturnType<typeof fetchCurrentSession>>,
  me: Awaited<ReturnType<typeof fetchCurrentMe>>
): AuthSessionDiscovery {
  const snapshot: AuthSnapshot = {
    csrfToken: session.csrf_token,
    actor: session.actor,
    me,
    availableRoles: session.available_roles
  };
  if (!import.meta.env.VITEST) {
    startEarlyConsoleStartupFlight(snapshot);
    void Promise.all([
      import('../../../app/bootstrap/frontstage-startup-prefetch').then((m) => {
        resetPrewarmCallback = m.discardPrewarmedAuthenticatedQueryClient;
        m.prewarmAuthenticatedStartup(snapshot);
      }),
      import('../../../app/bootstrap/AuthenticatedStartup')
    ]).catch(() => undefined);
  }
  return { status: 'authenticated', snapshot };
}

async function discoverAuthSession(): Promise<AuthSessionDiscovery> {
  try {
    const docSession = takeDocumentSessionDiscovery();
    if (typeof window !== 'undefined' && /^\/sign-in\/?$/u.test(window.location.pathname)) {
      const session = await (docSession ?? fetchCurrentSession());
      return finishAuthenticated(session, await fetchCurrentMe());
    }
    const [sRes, mRes] = await Promise.allSettled([
      docSession ?? Promise.resolve().then(() => fetchCurrentSession()),
      Promise.resolve().then(() => fetchCurrentMe())
    ]);
    if (
      sRes.status !== 'fulfilled' ||
      mRes.status !== 'fulfilled' ||
      !sRes.value ||
      !mRes.value
    ) {
      return { status: 'anonymous' };
    }
    return finishAuthenticated(sRes.value, mRes.value);
  } catch {
    return { status: 'anonymous' };
  }
}
