import type { AppRouteId } from '@1flowbase/shared-types';

export const FRONTSTAGE_SLUG_PATH = '/$slug';
export const FRONTSTAGE_SLUG_PAGE_PATH = '/$slug/pages/$pageId';
export const FRONTSTAGE_SLUG_PAGE_TAB_PATH =
  '/$slug/pages/$pageId/tabs/$tabRef';

export interface AppRouteDefinition {
  id: AppRouteId;
  path: string;
  navLabelKey: string | null;
  chromeSlot: 'primary' | 'secondary' | 'hidden';
  selectedMatchers: Array<(pathname: string) => boolean>;
  permissionKey: string | null;
  guard: 'public-only' | 'session-required';
}

export const APP_ROUTES: AppRouteDefinition[] = [
  {
    id: 'home',
    path: '/',
    navLabelKey: "auto.workbench",
    chromeSlot: 'primary',
    selectedMatchers: [(pathname) => pathname === '/'],
    permissionKey: null,
    guard: 'session-required'
  },
  {
    id: 'application-detail',
    path: '/applications',
    navLabelKey: null,
    chromeSlot: 'hidden',
    selectedMatchers: [(pathname) => /^\/applications\/[^/]+(\/|$)/.test(pathname)],
    permissionKey: null,
    guard: 'session-required'
  },
  // Keep embedded apps hidden because the roadmap is distant and may change.
  {
    id: 'templates',
    path: '/templates',
    navLabelKey: "auto.templates",
    chromeSlot: 'primary',
    selectedMatchers: [
      (pathname) => pathname === '/templates' || pathname.startsWith('/templates/')
    ],
    permissionKey: null,
    guard: 'session-required'
  },
  {
    id: 'settings',
    path: '/settings',
    navLabelKey: "auto.settings",
    chromeSlot: 'secondary',
    selectedMatchers: [(pathname) => pathname === '/settings' || pathname.startsWith('/settings/')],
    permissionKey: null,
    guard: 'session-required'
  },
  {
    id: 'me',
    path: '/me',
    navLabelKey: null,
    chromeSlot: 'hidden',
    selectedMatchers: [(pathname) => pathname === '/me' || pathname.startsWith('/me/')],
    permissionKey: null,
    guard: 'session-required'
  },
  {
    id: 'sign-in',
    path: '/sign-in',
    navLabelKey: null,
    chromeSlot: 'hidden',
    selectedMatchers: [(pathname) => pathname.startsWith('/sign-in')],
    permissionKey: null,
    guard: 'public-only'
  }
];

export function getSelectedRouteId(pathname: string): AppRouteId {
  const matchedRouteId =
    APP_ROUTES.find((route) => route.selectedMatchers.some((match) => match(pathname)))?.id ??
    'home';

  return matchedRouteId === 'application-detail' ? 'home' : matchedRouteId;
}
