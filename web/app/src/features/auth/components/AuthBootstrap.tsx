import { type PropsWithChildren, useEffect } from 'react';

import { ApiClientError } from '@1flowbase/api-client';

import { useAuthStore } from '../../../state/auth-store';
import { fetchCurrentMe, fetchCurrentSession } from '../api/session';

function getErrorStatus(error: unknown): number | null {
  if (error instanceof ApiClientError) {
    return error.status;
  }

  if (
    typeof error === 'object' &&
    error !== null &&
    'status' in error &&
    typeof error.status === 'number'
  ) {
    return error.status;
  }

  return null;
}

export function AuthBootstrap({ children }: PropsWithChildren) {
  const setAuthenticated = useAuthStore((state) => state.setAuthenticated);
  const setAnonymous = useAuthStore((state) => state.setAnonymous);

  useEffect(() => {
    let cancelled = false;

    const bootstrap = async () => {
      try {
        const session = await fetchCurrentSession();

        if (cancelled) {
          return;
        }

        const me = await fetchCurrentMe();

        if (cancelled) {
          return;
        }

        setAuthenticated({
          csrfToken: session.csrf_token,
          actor: session.actor,
          me,
          availableRoles: session.available_roles
        });
      } catch (error) {
        if (cancelled) {
          return;
        }

        if (getErrorStatus(error) === 401) {
          setAnonymous();
          return;
        }

        setAnonymous();
      }
    };

    void bootstrap();

    return () => {
      cancelled = true;
    };
  }, [setAuthenticated, setAnonymous]);

  return <>{children}</>;
}
