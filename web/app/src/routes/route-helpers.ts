import type { AppRouteId } from '@1flowbase/shared-types';

import { APP_ROUTES, type AppRouteDefinition } from './route-config';

export function getRouteDefinition(routeId: AppRouteId): AppRouteDefinition {
  const route = APP_ROUTES.find((entry) => entry.id === routeId);

  if (!route) {
    throw new Error(`Unknown route id: ${routeId}`);
  }

  return route;
}

export function getSecondaryChromeRoutes(): AppRouteDefinition[] {
  return APP_ROUTES.filter((route) => route.chromeSlot === 'secondary' && route.navLabelKey !== null);
}
