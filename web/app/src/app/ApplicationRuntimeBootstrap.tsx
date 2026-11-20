import { Suspense, lazy } from 'react';

import { LoadingState } from '../shared/ui/loading-state/LoadingState';
import { useAuthStore } from '../state/auth-store';

const AnonymousRuntime = lazy(() =>
  import('./AnonymousAppRuntime').then((module) => ({
    default: module.AnonymousAppRuntime
  }))
);
// This entry is preloaded by public sign-in HTML. Keep QueryClient, private API
// consumers and authenticated dictionaries behind the authenticated boundary.
const AuthenticatedStartup = lazy(() =>
  import('./bootstrap/AuthenticatedStartup').then((module) => ({
    default: module.AuthenticatedStartup
  }))
);

export function ApplicationRuntimeBootstrap() {
  const sessionStatus = useAuthStore((state) => state.sessionStatus);
  if (sessionStatus === 'unknown') {
    return <LoadingState fullscreen />;
  }

  return (
    <Suspense fallback={<LoadingState fullscreen />}>
      {sessionStatus === 'authenticated' ? (
        <AuthenticatedStartup />
      ) : (
        <AnonymousRuntime />
      )}
    </Suspense>
  );
}
