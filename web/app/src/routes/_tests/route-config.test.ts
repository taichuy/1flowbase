import { describe, expect, test } from 'vitest';

import { APP_ROUTES, getSelectedRouteId } from '../route-config';

describe('route truth layer', () => {
  test('keeps navigation ids, labels, paths, and selected-state logic in one source', () => {
    expect(APP_ROUTES.map((route) => route.id)).toEqual([
      'home',
      'frontstage',
      'application-detail',
      'embedded-apps',
      'templates',
      'settings',
      'me',
      'sign-in'
    ]);
    expect(APP_ROUTES.find((route) => route.id === 'templates')).toMatchObject({
      path: '/templates',
      navLabelKey: 'auto.templates'
    });
    expect(getSelectedRouteId('/settings')).toBe('settings');
    expect(getSelectedRouteId('/settings/docs')).toBe('settings');
    expect(getSelectedRouteId('/settings/roles')).toBe('settings');
    expect(getSelectedRouteId('/me')).toBe('me');
    expect(getSelectedRouteId('/me/profile')).toBe('me');
    expect(getSelectedRouteId('/me/security')).toBe('me');
    expect(getSelectedRouteId('/frontstage')).toBe('frontstage');
    expect(getSelectedRouteId('/frontstage/pages/page-1')).toBe('frontstage');
    expect(getSelectedRouteId('/frontstage/workspace-1')).toBe('home');
    expect(getSelectedRouteId('/frontstage/workspace-1/page-1')).toBe('home');
    expect(getSelectedRouteId('/templates')).toBe('templates');
    expect(getSelectedRouteId('/templates/agent-flow')).toBe('templates');
    expect(getSelectedRouteId('/templates-foo')).toBe('home');
    expect(getSelectedRouteId('/applications/app-1')).toBe('home');
    expect(getSelectedRouteId('/applications/app-1/orchestration')).toBe('home');
    expect(getSelectedRouteId('/applications/app-1/api')).toBe('home');
    expect(getSelectedRouteId('/settings-foo')).toBe('home');
    expect(getSelectedRouteId('/me-profile')).toBe('home');
  });

  test('declares guard and permission metadata for formal console routes', () => {
    expect(APP_ROUTES.find((route) => route.id === 'home')?.permissionKey).toBeNull();
    expect(APP_ROUTES.find((route) => route.id === 'frontstage')?.permissionKey).toBeNull();
    expect(APP_ROUTES.find((route) => route.id === 'frontstage')?.guard).toBe('session-required');
    expect(
      APP_ROUTES.find((route) => route.id === 'application-detail')?.permissionKey
    ).toBeNull();
    expect(APP_ROUTES.find((route) => route.id === 'embedded-apps')?.permissionKey).toBe(
      'embedded_app.view.all'
    );
    expect(APP_ROUTES.find((route) => route.id === 'templates')?.permissionKey).toBeNull();
    expect(APP_ROUTES.find((route) => route.id === 'settings')?.permissionKey).toBeNull();
    expect(APP_ROUTES.find((route) => route.id === 'sign-in')?.guard).toBe('public-only');
  });
});
