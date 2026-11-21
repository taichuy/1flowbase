import {
  takeDocumentSessionDiscovery,
  discardDocumentSessionDiscovery
} from './document-start-discovery';
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
  discardDocumentSessionDiscovery();
}

function isAnonymousSignInPathname(): boolean {
  return (
    typeof window !== 'undefined' &&
    /^\/sign-in\/?$/u.test(window.location.pathname)
  );
}

function startEarlyAuthenticatedStartup(snapshot: AuthSnapshot): void {
  if (import.meta.env.VITEST) return;
  void Promise.all([
    import('../../../app/bootstrap/frontstage-startup-prefetch').then(
      (module) => {
        resetPrewarmCallback = module.discardPrewarmedAuthenticatedQueryClient;
        module.prewarmAuthenticatedStartup(snapshot);
      }
    ),
    import('../../../app/bootstrap/AuthenticatedStartup')
  ]).catch(() => undefined);
}

async function discoverAuthSession(): Promise<AuthSessionDiscovery> {
  try {
    const documentSessionFlight = takeDocumentSessionDiscovery();
    if (documentSessionFlight || isAnonymousSignInPathname()) {
      const session = await (documentSessionFlight ?? fetchCurrentSession());
      const me = await fetchCurrentMe();
      const snapshot: AuthSnapshot = {
        csrfToken: session.csrf_token,
        actor: session.actor,
        me,
        availableRoles: session.available_roles
      };
      startEarlyAuthenticatedStartup(snapshot);
      return {
        status: 'authenticated',
        snapshot
      };
    }

    const sessionFlight = Promise.resolve().then(() => fetchCurrentSession());
    const meFlight = Promise.resolve().then(() => fetchCurrentMe());
    const [sessionResult, meResult] = await Promise.allSettled([
      sessionFlight,
      meFlight
    ]);
    if (
      sessionResult.status !== 'fulfilled' ||
      meResult.status !== 'fulfilled' ||
      !sessionResult.value ||
      !meResult.value
    ) {
      return { status: 'anonymous' };
    }
    const session = sessionResult.value;
    const me = meResult.value;
    const snapshot: AuthSnapshot = {
      csrfToken: session.csrf_token,
      actor: session.actor,
      me,
      availableRoles: session.available_roles
    };
    startEarlyAuthenticatedStartup(snapshot);
    return {
      status: 'authenticated',
      snapshot
    };
  } catch {
    return { status: 'anonymous' };
  }
}
