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
    <Navigate to="/sign-in" search={{ login_entry_id: undefined }} replace />
  )
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  component: () => (
    <Navigate to="/sign-in" search={{ login_entry_id: undefined }} replace />
  )
});

const signInRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/sign-in',
  validateSearch: (search: Record<string, unknown>) => ({
    login_entry_id:
      typeof search.login_entry_id === 'string' &&
      search.login_entry_id.trim().length > 0
        ? search.login_entry_id
        : undefined
  }),
  component: () => {
    const { login_entry_id } = signInRoute.useSearch();
    return <SignInPage loginEntryId={login_entry_id} />;
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
