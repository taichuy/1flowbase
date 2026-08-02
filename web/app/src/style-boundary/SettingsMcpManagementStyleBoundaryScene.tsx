import { useEffect, useMemo } from 'react';
import {
  Outlet,
  RouterProvider,
  createRootRoute,
  createRoute,
  createRouter
} from '@tanstack/react-router';

import { McpManagementPanel } from '../features/settings/components/mcp-management/McpManagementPanel';
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
        <McpManagementPanel
          canManage
          catalog={styleBoundaryMcpCatalog}
          interfaceCapabilities={styleBoundaryMcpInterfaceCapabilities}
        />
      )
    });

    return createRouter({
      routeTree: rootRoute.addChildren([pageRoute])
    });
  }, []);

  useEffect(() => {
    let attempts = 0;
    const timer = window.setInterval(() => {
      const importButton = [
        ...document.querySelectorAll<HTMLButtonElement>('button')
      ].find((button) =>
        /^(导入|Import)$/.test(button.textContent?.trim() ?? '')
      );

      attempts += 1;
      if (importButton) {
        importButton.click();
        window.clearInterval(timer);
      }
      if (attempts >= 60) {
        window.clearInterval(timer);
      }
    }, 100);

    return () => window.clearInterval(timer);
  }, []);

  return <RouterProvider router={router} />;
}
