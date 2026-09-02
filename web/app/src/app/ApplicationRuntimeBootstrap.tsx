import { Suspense, lazy } from 'react';

import { loadApplicationI18nResources } from '../shared/i18n/app-i18n';
import { LoadingState } from '../shared/ui/loading-state/LoadingState';
import { useAuthStore } from '../state/auth-store';

const AnonymousRuntime = lazy(() =>
  import('./AnonymousAppRuntime').then((module) => ({
    default: module.AnonymousAppRuntime
  }))
);
const AuthenticatedRuntime = lazy(async () => {
  const [runtimeModule] = await Promise.all([
    import('./AuthenticatedAppRuntime'),
    loadApplicationI18nResources(),
    import('../app-shell/AppShellFrame')
  ]);
  return { default: runtimeModule.AuthenticatedAppRuntime };
});

export function ApplicationRuntimeBootstrap() {
  const sessionStatus = useAuthStore((state) => state.sessionStatus);
  if (sessionStatus === 'unknown') {
    return <LoadingState fullscreen />;
  }
  const Runtime =
    sessionStatus === 'authenticated' ? AuthenticatedRuntime : AnonymousRuntime;

  return (
    <Suspense fallback={<LoadingState fullscreen />}>
      <Runtime />
    </Suspense>
  );
}
