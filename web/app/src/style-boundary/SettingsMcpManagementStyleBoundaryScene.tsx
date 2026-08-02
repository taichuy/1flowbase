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
import {
  styleBoundaryMcpCatalog,
  styleBoundaryMcpInterfaceCapabilities
} from './scene-fixtures';

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
          <McpManagementPanel
            canManage
            catalog={styleBoundaryMcpCatalog}
            interfaceCapabilities={styleBoundaryMcpInterfaceCapabilities}
          />
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
