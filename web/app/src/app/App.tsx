import { Suspense, lazy } from 'react';

import { AuthBootstrap } from '../features/auth/components/AuthBootstrap';
import {
  ApplicationBootBoundary,
  ApplicationBootStage
} from './ApplicationBootBoundary';

const ApplicationRuntimeBootstrap = lazy(() =>
  import('./ApplicationRuntimeBootstrap').then((module) => ({
    default: module.ApplicationRuntimeBootstrap
  }))
);

export function App() {
  return (
    <ApplicationBootBoundary>
      <AuthBootstrap>
        <Suspense fallback={<ApplicationBootStage />}>
          <ApplicationRuntimeBootstrap />
        </Suspense>
      </AuthBootstrap>
    </ApplicationBootBoundary>
  );
}
