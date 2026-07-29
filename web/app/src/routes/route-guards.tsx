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
  const actor = useAuthStore((state) => state.actor);
  const me = useAuthStore((state) => state.me);

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
    return (
      <Navigate
        to="/sign-in"
        search={{ authenticator_id: undefined }}
        replace
      />
    );
  }

  if (!route.permissionKey) {
    return <>{children}</>;
  }

  const hasPermission =
    actor?.effective_display_role === 'root' ||
    Boolean(me?.permissions.includes(route.permissionKey));

  if (!hasPermission) {
    return <PermissionDeniedState />;
  }

  return <>{children}</>;
}
