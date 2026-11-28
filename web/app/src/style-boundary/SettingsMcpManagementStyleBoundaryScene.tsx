import { useMemo } from 'react';
import {
  Outlet,
  RouterProvider,
  createRootRoute,
  createRoute,
  createRouter
} from '@tanstack/react-router';

import { McpManagementPanel } from '../features/settings/components/mcp-management/McpManagementPanel';
import { McpTemplateLibrary } from '../features/settings/components/mcp-management/bundle/McpTemplateLibrary';

export function SettingsMcpManagementStyleBoundaryScene() {
  const router = useMemo(() => {
    window.history.replaceState({}, '', '/');
    const rootRoute = createRootRoute({
      component: () => <Outlet />
    });
    const pageRoute = createRoute({
      getParentRoute: () => rootRoute,
      path: '/',
      component: () => (
        <>
          <McpManagementPanel canManage />
          <McpTemplateLibrary variant="compact" />
        </>
      )
    });

    return createRouter({
      routeTree: rootRoute.addChildren([pageRoute])
    });
  }, []);

  return <RouterProvider router={router} />;
}
