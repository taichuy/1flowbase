import {
  Navigate,
  Outlet,
  RouterProvider,
  createRootRoute,
  createRoute,
  createRouter
} from '@tanstack/react-router';
import { useState } from 'react';

import { SignInPage } from '../features/auth/pages/SignInPage';

const rootRoute = createRootRoute({
  component: () => <Outlet />,
  notFoundComponent: () => (
    <Navigate to="/sign-in" search={{ authenticator_id: undefined }} replace />
  )
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  component: () => (
    <Navigate to="/sign-in" search={{ authenticator_id: undefined }} replace />
  )
});

const signInRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/sign-in',
  validateSearch: (search: Record<string, unknown>) => ({
    authenticator_id:
      typeof search.authenticator_id === 'string' &&
      search.authenticator_id.trim().length > 0
        ? search.authenticator_id
        : undefined
  }),
  component: () => {
    const { authenticator_id } = signInRoute.useSearch();
    return <SignInPage authenticatorId={authenticator_id} />;
  }
});

const routeTree = rootRoute.addChildren([indexRoute, signInRoute]);

function createPublicRouter() {
  return createRouter({ routeTree, notFoundMode: 'root' });
}

export function PublicRouterProvider() {
  const [router] = useState(createPublicRouter);
  return <RouterProvider router={router} />;
}
