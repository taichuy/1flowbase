import { render, screen } from '@testing-library/react';
import {
  Outlet,
  RouterProvider,
  createRootRoute,
  createRoute,
  createRouter
} from '@tanstack/react-router';
import { beforeEach, describe, expect, test } from 'vitest';

import { AppProviders } from '../../app/AppProviders';
import { useAuthStore } from '../../state/auth-store';
import { RouteGuard } from '../route-guards';

function renderGuardedRouter(pathname: string) {
  window.history.pushState({}, '', pathname);

  const rootRoute = createRootRoute({
    component: () => <Outlet />
  });
  const homeRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/',
    component: () => (
      <RouteGuard routeId="home">
        <div>home page</div>
      </RouteGuard>
    )
  });
  const signInRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/sign-in',
    component: () => <div>sign-in page</div>
  });
  const router = createRouter({
    routeTree: rootRoute.addChildren([homeRoute, signInRoute])
  });

  return render(
    <AppProviders>
      <RouterProvider router={router} />
    </AppProviders>
  );
}

describe('RouteGuard', () => {
  beforeEach(() => {
    useAuthStore.getState().setAnonymous();
  });

  test('redirects anonymous users from session routes to /sign-in', async () => {
    renderGuardedRouter('/');

    expect(await screen.findByText('sign-in page')).toBeInTheDocument();
  });
});
