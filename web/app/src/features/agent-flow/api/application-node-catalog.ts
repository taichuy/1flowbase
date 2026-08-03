import {
  getConsoleApplicationNodeCatalog,
  type ConsoleApplicationNodeCatalog
} from '@1flowbase/api-client';

import { getApplicationsApiBaseUrl } from '../../applications/api/applications';

export type ApplicationNodeCatalog = ConsoleApplicationNodeCatalog;

export const applicationNodeCatalogQueryKey = (applicationId: string) =>
  ['applications', applicationId, 'application-node-catalog'] as const;

export function fetchApplicationNodeCatalog(applicationId: string) {
  return getConsoleApplicationNodeCatalog(
    applicationId,
    getApplicationsApiBaseUrl()
  );
}
