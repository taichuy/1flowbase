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
  const embeddedAppsRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/embedded-apps',
    component: () => (
      <RouteGuard routeId="embedded-apps">
        <div>embedded apps page</div>
      </RouteGuard>
    )
  });
  const signInRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/sign-in',
    component: () => <div>sign-in page</div>
  });
  const router = createRouter({
    routeTree: rootRoute.addChildren([homeRoute, embeddedAppsRoute, signInRoute])
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

  test('renders permission denied state for authenticated users missing the route permission', async () => {
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-123',
      actor: {
        id: 'user-1',
        account: 'manager',
        effective_display_role: 'manager',
        current_workspace_id: 'workspace-1'
      },
      me: {
        id: 'user-1',
        account: 'manager',
        email: 'manager@example.com',
        phone: null,
        nickname: 'Manager',
        name: 'Manager',
        avatar_url: null,
        introduction: '',
        effective_display_role: 'manager',
        permissions: []
      }
    });

    renderGuardedRouter('/embedded-apps');

    expect(await screen.findByText('无权限访问')).toBeInTheDocument();
  });
});
