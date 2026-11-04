import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import {
  persistInboxSplitRatio,
  readInboxSplitRatio
} from '../lib/inbox-split-ratio';

const storageKey = '1flowbase.notifications.inbox_split_ratio';
function authenticate(role: string, permissions: string[]) {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf',
    actor: {
      id: 'user-1',
      account: 'member',
      effective_display_role: role,
      current_workspace_id: 'workspace'
    },
    me: {
      id: 'user-1',
      account: 'member',
      name: 'Member',
      nickname: 'Member',
      email: 'member@example.com',
      phone: null,
      avatar_url: null,
      introduction: '',
      effective_display_role: role,
      permissions
    }
  });
}
beforeEach(() => {
  resetAuthStore();
  window.localStorage.setItem(storageKey, '0.35');
});
afterEach(() => {
  resetAuthStore();
  window.localStorage.removeItem(storageKey);
  vi.restoreAllMocks();
});

describe('notification layout preference persistence', () => {
  test.each([
    { role: 'root', permissions: [], allowed: true },
    { role: 'member', permissions: ['patch_me_meta'], allowed: true },
    { role: 'member', permissions: [], allowed: false },
    { role: 'member', permissions: ['patch_me'], allowed: false },
    {
      role: 'member',
      permissions: ['notifications.messages.read'],
      allowed: false
    },
    {
      role: 'member',
      permissions: ['notifications.messages.unread'],
      allowed: false
    },
    {
      role: 'member',
      permissions: ['notifications.messages.read_all'],
      allowed: false
    },
    {
      role: 'member',
      permissions: ['notifications.channels.read_all'],
      allowed: false
    }
  ])(
    'persists split ratio only with its exact preference grant $role/$permissions',
    ({ role, permissions, allowed }) => {
      authenticate(role, permissions);
      const me = useAuthStore.getState().me;
      const write = vi.spyOn(Storage.prototype, 'setItem');
      expect(readInboxSplitRatio()).toBe(0.35);
      persistInboxSplitRatio(0.456789);
      expect(readInboxSplitRatio()).toBe(allowed ? 0.4568 : 0.35);
      if (allowed) expect(write).toHaveBeenCalledWith(storageKey, '0.4568');
      else expect(write).not.toHaveBeenCalled();
      expect(useAuthStore.getState().me).toEqual(me);
    }
  );

  test('checks the current grant when a delayed resize ends after revocation', () => {
    authenticate('member', ['patch_me_meta']);
    const finishResize = () => persistInboxSplitRatio(0.5);
    useAuthStore
      .getState()
      .setMe({ ...useAuthStore.getState().me!, permissions: [] });
    finishResize();
    expect(readInboxSplitRatio()).toBe(0.35);
  });
});
