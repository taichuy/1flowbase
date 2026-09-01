import { Suspense, lazy } from 'react';

import { AuthBootstrap } from '../features/auth/components/AuthBootstrap';
import { LoadingState } from '../shared/ui/loading-state/LoadingState';
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
        <Suspense fallback={<LoadingState fullscreen />}>
          <ApplicationRuntimeBootstrap />
        </Suspense>
      </AuthBootstrap>
    </ApplicationBootBoundary>
  );
}
