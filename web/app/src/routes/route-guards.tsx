import { useConsoleOperationAccessStatus } from '../state/console-operation-access';
import type { PropsWithChildren } from 'react';

import type { AppRouteId } from '@1flowbase/shared-types';
import { Navigate } from '@tanstack/react-router';

import { useAuthStore } from '../state/auth-store';
import { LoadingState } from '../shared/ui/loading-state/LoadingState';
import { PermissionDeniedState } from '../shared/ui/PermissionDeniedState';
import { getRouteDefinition } from './route-helpers';

export function RouteGuard({
  children,
  routeId
}: PropsWithChildren<{ routeId: AppRouteId }>) {
  const route = getRouteDefinition(routeId);
  const sessionStatus = useAuthStore((state) => state.sessionStatus);
  const access = useConsoleOperationAccessStatus(route.permissionKey ?? '');

  if (sessionStatus === 'unknown') {
    return <LoadingState fullscreen />;
  }

  if (route.guard === 'public-only') {
    if (sessionStatus === 'authenticated') {
      return <Navigate to="/" replace />;
    }

    return <>{children}</>;
  }

  if (sessionStatus === 'anonymous') {
    const invite =
      typeof window !== 'undefined'
        ? new URLSearchParams(window.location.search).get('invite')?.trim() ||
          undefined
        : undefined;
    return (
      <Navigate
        to="/sign-in"
        search={{ login_entry_id: undefined, invite }}
        replace
      />
    );
  }

  if (!route.permissionKey) {
    return <>{children}</>;
  }

  if (access === 'loading') return <LoadingState fullscreen />;

  if (access === 'denied') {
    return <PermissionDeniedState />;
  }

  return <>{children}</>;
}
