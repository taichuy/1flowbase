import { Suspense, lazy } from 'react';

import { AuthBootstrap } from '../features/auth/components/AuthBootstrap';
import { AnonymousLoadingState } from '../features/auth/components/anonymous-loading/AnonymousLoadingState';
import { ApplicationBootBoundary } from './ApplicationBootBoundary';

const ApplicationRuntimeBootstrap = lazy(() =>
  import('./ApplicationRuntimeBootstrap').then((module) => ({
    default: module.ApplicationRuntimeBootstrap
  }))
);

export function App() {
  return (
    <ApplicationBootBoundary>
      <AuthBootstrap>
        <Suspense fallback={<AnonymousLoadingState fullscreen />}>
          <ApplicationRuntimeBootstrap />
        </Suspense>
      </AuthBootstrap>
    </ApplicationBootBoundary>
  );
}
