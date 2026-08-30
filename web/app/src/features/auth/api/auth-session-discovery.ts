import type { AuthSnapshot } from '../../../state/auth-store';
import { fetchCurrentMe, fetchCurrentSession } from './session';

export type AuthSessionDiscovery =
  | { status: 'authenticated'; snapshot: AuthSnapshot }
  | { status: 'anonymous' };

let discoveryFlight: Promise<AuthSessionDiscovery> | undefined;

export function startAuthSessionDiscovery(): Promise<AuthSessionDiscovery> {
  discoveryFlight ??= discoverAuthSession();
  return discoveryFlight;
}

export function resetAuthSessionDiscovery() {
  discoveryFlight = undefined;
}

async function discoverAuthSession(): Promise<AuthSessionDiscovery> {
  try {
    const session = await fetchCurrentSession();
    const me = await fetchCurrentMe();
    return {
      status: 'authenticated',
      snapshot: {
        csrfToken: session.csrf_token,
        actor: session.actor,
        me,
        availableRoles: session.available_roles
      }
    };
  } catch {
    return { status: 'anonymous' };
  }
}
