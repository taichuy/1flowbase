import { Suspense, lazy } from 'react';

import { AnonymousLoadingState } from '../features/auth/components/anonymous-loading/AnonymousLoadingState';
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
    return <AnonymousLoadingState fullscreen />;
  }

  return (
    <Suspense fallback={<AnonymousLoadingState fullscreen />}>
      {sessionStatus === 'authenticated' ? (
        <AuthenticatedStartup />
      ) : (
        <AnonymousRuntime />
      )}
    </Suspense>
  );
}
