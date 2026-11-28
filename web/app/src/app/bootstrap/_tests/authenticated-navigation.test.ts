import { afterEach, beforeEach, expect, test, vi } from 'vitest';

import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { waitFor } from '@testing-library/react';

import { selectNavigationQueryScope } from '../../../state/navigation-query-scope';
import { frontstagePageTreeQueryOptions } from '../../../features/frontstage/api/page-tree';
import { settingsConsoleNavigationQueryOptions } from '../../../features/settings/api/console-navigation';
import {
  createApplicationQueryClient,
  startAuthenticatedNavigation
} from '../authenticated-navigation';

const api = vi.hoisted(() => ({ navigation: vi.fn(), pages: vi.fn() }));
vi.mock('@1flowbase/api-client', () => ({
  getConsoleNavigation: api.navigation,
  listFrontstagePages: api.pages,
  getDefaultApiBaseUrl: () => '/api',
  createFrontstageGroup: vi.fn(),
  createFrontstagePage: vi.fn(),
  deleteFrontstagePageNode: vi.fn(),
  moveFrontstagePageNode: vi.fn(),
  updateFrontstagePageNodeTitle: vi.fn()
}));

const settingsFlight = vi.hoisted(() => ({
  shell: vi.fn(),
  network: vi.fn(),
  members: vi.fn()
}));
vi.mock('../../../features/settings/pages/SettingsPage', async () => {
  await settingsFlight.shell();
  return { SettingsPage: () => null };
});
vi.mock('../../../features/settings/pages/network-center/NetworkCenterSection', () => {
  settingsFlight.network();
  return { NetworkCenterSection: () => null };
});
vi.mock('../../../features/settings/pages/settings-page/MemberSettingsTabs', () => {
  settingsFlight.members();
  return { MemberSettingsTabs: () => null };
});

function authenticate(
  id = 'actor-a',
  workspace = 'workspace-a',
  role = 'developer'
) {
  useAuthStore.getState().setAuthenticated({
    actor: {
      id,
      account: id,
      current_workspace_id: workspace,
      effective_display_role: role
    },
    csrfToken: 'csrf',
    me: null
  });
}
const stops: Array<() => void> = [];
beforeEach(() => {
  resetAuthStore();
  api.navigation
    .mockReset()
    .mockResolvedValue({ navigation_items: [], route_definitions: [] });
  api.pages.mockReset().mockResolvedValue([{ id: 'page-a' }]);
});
afterEach(() => {
  stops.splice(0).forEach((stop) => stop());
  resetAuthStore();
});

function start() {
  const client = createApplicationQueryClient();
  stops.push(startAuthenticatedNavigation(client));
  return client;
}

test('starts both requests as soon as auth scope is known, without any shell mount', async () => {
  const client = start();
  expect(api.navigation).not.toHaveBeenCalled();
  expect(api.pages).not.toHaveBeenCalled();
  authenticate();
  // Both transports start synchronously from the store notification, not serially.
  expect(api.navigation).toHaveBeenCalledTimes(1);
  expect(api.pages).toHaveBeenCalledTimes(1);
  const scope = selectNavigationQueryScope(useAuthStore.getState());
  expect(
    await client.fetchQuery(
      frontstagePageTreeQueryOptions('workspace-a', scope)
    )
  ).toEqual([{ id: 'page-a' }]);
  await client.fetchQuery(settingsConsoleNavigationQueryOptions(scope));
  authenticate();
  expect(api.navigation).toHaveBeenCalledTimes(1);
  expect(api.pages).toHaveBeenCalledTimes(1);
});

test('consumers deduplicate the pending preload using the same keys and options', async () => {
  let finish!: (value: Array<{ id: string }>) => void;
  api.pages.mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      })
  );
  authenticate();
  const client = start();
  const scope = selectNavigationQueryScope(useAuthStore.getState());
  const first = client.fetchQuery(
    frontstagePageTreeQueryOptions('workspace-a', scope)
  );
  const second = client.fetchQuery(
    frontstagePageTreeQueryOptions('workspace-a', scope)
  );
  expect(api.pages).toHaveBeenCalledTimes(1);
  finish([{ id: 'prefetched' }]);
  expect(await first).toEqual(await second);
});

test.each([
  ['actor-b', 'workspace-a', 'developer'],
  ['actor-a', 'workspace-b', 'developer'],
  ['actor-a', 'workspace-a', 'root']
])(
  'isolates changes to actor/workspace/role: %s %s %s',
  async (id, workspace, role) => {
    authenticate();
    const client = start();
    const oldKey = frontstagePageTreeQueryOptions(
      'workspace-a',
      selectNavigationQueryScope(useAuthStore.getState())
    ).queryKey;
    await client.fetchQuery(
      frontstagePageTreeQueryOptions(
        'workspace-a',
        selectNavigationQueryScope(useAuthStore.getState())
      )
    );
    authenticate(id, workspace, role);
    expect(client.getQueryData(oldKey)).toBeUndefined();
    expect(api.pages).toHaveBeenCalledTimes(2);
    expect(api.navigation).toHaveBeenCalledTimes(2);
  }
);

test('logout removes cached and in-flight private data; a late response cannot repopulate it', async () => {
  let finish!: (value: unknown[]) => void;
  api.pages.mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      })
  );
  authenticate();
  const client = start();
  useAuthStore.getState().setAnonymous();
  finish([{ id: 'private-a' }]);
  await Promise.resolve();
  expect(client.getQueryCache().getAll()).toHaveLength(0);
  authenticate('actor-b');
  expect(api.pages).toHaveBeenCalledTimes(2);
  stops.pop()!();
  expect(client.getQueryCache().getAll()).toHaveLength(0);
});

test('permission changes invalidate private caches; locale and permission ordering do not', async () => {
  authenticate();
  const me = {
    id: 'actor-a',
    account: 'actor-a',
    email: '',
    phone: null,
    nickname: '',
    name: '',
    avatar_url: null,
    introduction: '',
    effective_display_role: 'developer',
    policy_fingerprint: 'fixture-policy',

    preferred_locale: 'en_US'
  };
  useAuthStore.getState().setMe(me);
  const client = start();
  const oldKey = frontstagePageTreeQueryOptions(
    'workspace-a',
    selectNavigationQueryScope(useAuthStore.getState())
  ).queryKey;
  await client.fetchQuery(
    frontstagePageTreeQueryOptions(
      'workspace-a',
      selectNavigationQueryScope(useAuthStore.getState())
    )
  );
  useAuthStore
    .getState()
    .setMe({
      ...me,
      preferred_locale: 'zh_Hans'
    });
  expect(api.pages).toHaveBeenCalledTimes(1);
  expect(client.getQueryData(oldKey)).toBeDefined();
  useAuthStore.getState().setMe({ ...me, policy_fingerprint: 'revoked-policy' });
  expect(client.getQueryData(oldKey)).toBeUndefined();
  expect(api.pages).toHaveBeenCalledTimes(2);
});

test('bootstrap owners do not share a singleton or a public-auth cache', () => {
  authenticate();
  const first = start();
  const second = createApplicationQueryClient();
  first.setQueryData(['private'], 'actor-a');
  expect(second.getQueryData(['private'])).toBeUndefined();
  expect(first.getDefaultOptions().queries?.retry).toBe(false);
  expect(first.getQueryDefaults(['frontstage']).refetchOnWindowFocus).toBe(
    false
  );
  second.clear();
});


test('only caches stable Settings families and leaves live query freshness intact', () => {
  const client = createApplicationQueryClient();
  const defaults = (queryKey: string[]) => client.defaultQueryOptions({ queryKey });
  expect(defaults(['settings', 'docs', 'catalog'])).toMatchObject({
    refetchOnWindowFocus: false,
    staleTime: 30_000
  });
  for (const key of [
    ['settings', 'system-runtime'],
    ['settings', 'system-runtime', 'processes'],
    ['settings', 'departments', 'access'],
    ['notifications', 'scope', 'unread-count']
  ]) {
    expect(defaults(key).staleTime ?? 0).toBe(0);
  }
  expect(client.defaultQueryOptions(settingsConsoleNavigationQueryOptions('scope')).staleTime).toBe(30_000);
});


test('starts the Settings shell and matching section before navigation or the shell module resolves', async () => {
  const previousPath = window.location.pathname;
  let finishShell!: () => void;
  let finishNavigation!: (value: { navigation_items: []; route_definitions: [] }) => void;
  settingsFlight.shell.mockReturnValueOnce(new Promise<void>((resolve) => {
    finishShell = resolve;
  }));
  api.navigation.mockReturnValueOnce(new Promise((resolve) => {
    finishNavigation = resolve;
  }));
  try {
    start();
    expect(settingsFlight.shell).not.toHaveBeenCalled();
    window.history.replaceState({}, '', '/settings/network-center/proxy-pools');
    authenticate();
    expect(api.navigation).toHaveBeenCalledTimes(1);
    await waitFor(() => {
      expect(settingsFlight.shell).toHaveBeenCalledTimes(1);
      expect(settingsFlight.network).toHaveBeenCalledTimes(1);
    });
    expect(settingsFlight.members).not.toHaveBeenCalled();
  } finally {
    finishShell();
    finishNavigation({ navigation_items: [], route_definitions: [] });
    window.history.replaceState({}, '', previousPath);
  }
});
