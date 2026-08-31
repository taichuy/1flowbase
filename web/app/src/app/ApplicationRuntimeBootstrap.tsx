import { Suspense, lazy } from 'react';

import { loadApplicationI18nResources } from '../shared/i18n/app-i18n';
import { useAuthStore } from '../state/auth-store';
import { ApplicationBootStage } from './ApplicationBootBoundary';

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
    return (
      <div
        className="application-bootstrap"
        role="status"
        aria-label="thinking"
      >
        <span className="application-bootstrap__pulse" />
      </div>
    );
  }
  const Runtime =
    sessionStatus === 'authenticated' ? AuthenticatedRuntime : AnonymousRuntime;

  return (
    <Suspense fallback={<ApplicationBootStage />}>
      <Runtime />
    </Suspense>
  );
}
