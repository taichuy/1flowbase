import type { PropsWithChildren } from 'react';

import { Navigate } from '@tanstack/react-router';

import { LoadingState } from '../shared/ui/loading-state/LoadingState';
import { useAuthStore } from '../state/auth-store';

export function SessionGuard({ children }: PropsWithChildren) {
  const sessionStatus = useAuthStore((state) => state.sessionStatus);

  if (sessionStatus === 'unknown') {
    return <LoadingState fullscreen />;
  }

  if (sessionStatus === 'anonymous') {
    return (
      <Navigate
        to="/sign-in"
        search={{ authenticator_id: undefined }}
        replace
      />
    );
  }

  return <>{children}</>;
}
