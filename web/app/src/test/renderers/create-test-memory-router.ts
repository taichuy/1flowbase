import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterProvider
} from '@tanstack/react-router';
import { createElement, type ReactNode } from 'react';

export async function createTestMemoryRouter(node: ReactNode) {
  const rootRoute = createRootRoute({ component: () => node });
  const router = createRouter({
    routeTree: rootRoute,
    history: createMemoryHistory({ initialEntries: ['/'] })
  });
  await router.load();
  return createElement(RouterProvider, { router });
}
