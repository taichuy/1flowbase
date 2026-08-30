import { Suspense, lazy } from 'react';

import { AuthBootstrap } from '../features/auth/components/AuthBootstrap';

const ApplicationRuntimeBootstrap = lazy(() =>
  import('./ApplicationRuntimeBootstrap').then((module) => ({
    default: module.ApplicationRuntimeBootstrap
  }))
);

export function App() {
  return (
    <AuthBootstrap>
      <Suspense fallback={null}>
        <ApplicationRuntimeBootstrap />
      </Suspense>
    </AuthBootstrap>
  );
}
