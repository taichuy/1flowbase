import { act, render, screen } from '@testing-library/react';
import {
  Outlet,
  RouterProvider,
  createRootRoute,
  createRoute,
  createRouter
} from '@tanstack/react-router';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { AppProviders } from '../../app/AppProviders';
import { useAuthStore } from '../../state/auth-store';
import { RouteGuard } from '../route-guards';
import { getRouteDefinition } from '../route-helpers';

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


test('does not display the denial state until the backend resolves a guarded route', async () => {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf',
    actor: { id: 'user', account: 'user', current_workspace_id: 'workspace', effective_display_role: 'member' },
    me: null
  });
  const route = getRouteDefinition('home');
  const originalKey = route.permissionKey;
  route.permissionKey = 'test.guarded';
  let finish!: (response: Response) => void;
  const fetch = vi.spyOn(globalThis, 'fetch').mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  let unmount: (() => void) | undefined;
  try {
    const rendered = render(<RouteGuard routeId="home"><div>guarded content</div></RouteGuard>);
    const { container } = rendered;
    unmount = rendered.unmount;
    expect(container.querySelector('.ant-result-403')).not.toBeInTheDocument();
    expect(screen.queryByText('guarded content')).not.toBeInTheDocument();
    await act(async () => {
      await Promise.resolve();
      finish(new Response(JSON.stringify({ data: { policy_fingerprint: 'restricted', allowed: { 'test.guarded': false } } }), { headers: { 'content-type': 'application/json' } }));
    });
    expect(container.querySelector('.ant-result-403')).toBeInTheDocument();
  } finally {
    unmount?.();
    route.permissionKey = originalKey;
    fetch.mockRestore();
    useAuthStore.getState().setAnonymous();
  }
});
