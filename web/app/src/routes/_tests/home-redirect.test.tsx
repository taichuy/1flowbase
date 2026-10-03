import { fireEvent, render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider
} from '@tanstack/react-router';
import { beforeEach, expect, test, vi } from 'vitest';
import { HomeRedirect } from '../HomeRedirect';
import { resetAuthStore, useAuthStore } from '../../state/auth-store';

const fetchTree = vi.hoisted(() => vi.fn());
vi.mock('../../features/frontstage/api/page-tree', () => ({
  fetchFrontstagePageTree: fetchTree,
  frontstagePageTreeQueryKey: (workspaceId: string) => [
    'frontstage',
    workspaceId
  ]
}));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key })
}));

function renderRedirect() {
  const history = createMemoryHistory({ initialEntries: ['/'] });
  const rootRoute = createRootRoute();
  const homeRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/',
    component: HomeRedirect
  });
  const meRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/me',
    component: () => <a href="/me">Destination</a>
  });
  const slugRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/$slug',
    component: () => {
      const { slug } = slugRoute.useParams();
      return <a href={`/${slug}`}>Destination</a>;
    }
  });
  const router = createRouter({
    routeTree: rootRoute.addChildren([homeRoute, meRoute, slugRoute]),
    history
  });
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <RouterProvider router={router} />
    </QueryClientProvider>
  );
  return { history, router };
}

beforeEach(() => {
  fetchTree.mockReset();
  resetAuthStore();
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf',
    actor: {
      id: 'actor',
      account: 'member',
      effective_display_role: 'member',
      current_workspace_id: 'workspace'
    },
    me: null
  });
});

test('redirects to the first accessible topbar node, skipping sidebar and missing slugs', async () => {
  fetchTree.mockResolvedValue([
    { placement: 'sidebar', slug: 'sidebar' },
    { placement: 'topbar', slug: null },
    { placement: 'topbar', slug: 'demo' },
    { placement: 'topbar', slug: 'gateway' }
  ]);
  const { history } = renderRedirect();
  expect(await screen.findByRole('link')).toHaveAttribute('href', '/demo');
  expect(history.length).toBe(1);
});

test('uses the personal center when there are no topbar pages', async () => {
  fetchTree.mockResolvedValue([]);
  renderRedirect();
  expect(await screen.findByRole('link')).toHaveAttribute('href', '/me');
});

test('does not redirect before the navigation tree finishes loading', async () => {
  fetchTree.mockReturnValue(new Promise(() => {}));
  renderRedirect();
  expect(await screen.findByRole('status')).toBeInTheDocument();
  expect(screen.queryByRole('link')).not.toBeInTheDocument();
});

test('exposes a retry on failure and redirects after recovery', async () => {
  fetchTree
    .mockRejectedValueOnce(new Error('unavailable'))
    .mockResolvedValueOnce([{ placement: 'topbar', slug: 'gateway' }]);
  renderRedirect();
  fireEvent.click(await screen.findByRole('button', { name: 'auto.retry' }));
  expect(await screen.findByRole('link')).toHaveAttribute('href', '/gateway');
});

test('does not fetch or loop back to home without a workspace', async () => {
  resetAuthStore();
  renderRedirect();
  expect(await screen.findByRole('link')).toHaveAttribute('href', '/me');
  expect(fetchTree).not.toHaveBeenCalled();
});
