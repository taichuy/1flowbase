import { useMemo } from 'react';
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterProvider
} from '@tanstack/react-router';
import { ApplicationCollectorPage } from '../../features/applications/pages/ApplicationCollectorPage';
import { CollectorInstallation } from '../../features/applications/components/collector/CollectorInstallation';

import { applicationId, collector } from './collector/data';

export function CollectorStyleBoundaryScene({
  installation
}: {
  installation: boolean;
}) {
  const router = useMemo(() => {
    const root = createRootRoute({
      component: () =>
        installation ? (
          <div className="application-collector">
            <CollectorInstallation
              collector={collector}
              applicationId={applicationId}
              endpoint="https://console.example.com/api/logs/v1/events"
              apiBaseUrl="https://console.example.com"
            />
          </div>
        ) : (
          <ApplicationCollectorPage applicationId={applicationId} />
        )
    });
    return createRouter({
      routeTree: root,
      history: createMemoryHistory({ initialEntries: ['/'] })
    });
  }, [installation]);
  return <RouterProvider router={router} />;
}
