import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LanguageChromeMenu } from '../LanguageChromeMenu';
import { resetAuthStore, useAuthStore } from '../../state/auth-store';
import { appI18n } from '../../shared/i18n/app-i18n';
import { i18nText } from '../../shared/i18n/text';

const { patchUserPreferences } = vi.hoisted(() => ({
  patchUserPreferences: vi.fn()
}));
vi.mock('../../shared/user-preferences/user-preferences', () => ({
  patchUserPreferences
}));

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

beforeEach(async () => {
  resetAuthStore();
  window.localStorage.clear();
  patchUserPreferences.mockReset();
  await appI18n.changeLanguage('zh_Hans');
});
afterEach(() => resetAuthStore());

describe('language preference operation access', () => {
  it.each(
    [[], ['patch_me'], ['assistant_update_settings']].map(
      (permissions) => ({ permissions })
    )
  )(
    'keeps language preference unchanged with unrelated grants $permissions',
    async ({ permissions }) => {
      authenticate('member', permissions);
      const me = useAuthStore.getState().me;
      const write = vi.spyOn(Storage.prototype, 'setItem');
      const view = render(<LanguageChromeMenu />);
      const trigger = screen.getByLabelText(
        i18nText('appShell', 'language.trigger')
      );
      expect(trigger.closest('[role="menuitem"]')).toHaveAttribute(
        'aria-disabled',
        'true'
      );
      await act(async () => {
        fireEvent.mouseEnter(trigger);
        fireEvent.click(trigger);
      });
      expect(useAuthStore.getState().me).toEqual(me);
      expect(patchUserPreferences).not.toHaveBeenCalled();
      expect(write).not.toHaveBeenCalled();
      view.unmount();
      write.mockRestore();
    }
  );

  it.each([
    { role: 'root', permissions: [] },
    { role: 'member', permissions: ['patch_me_meta'] }
  ])(
    'allows the exact preference operation for $role',
    async ({ role, permissions }) => {
      authenticate(role, permissions);
      patchUserPreferences.mockResolvedValue(useAuthStore.getState().me);
      const view = render(<LanguageChromeMenu />);
      await act(async () => {
        fireEvent.mouseEnter(
          screen.getByLabelText(i18nText('appShell', 'language.trigger'))
        );
      });
      const english = await screen.findByText(
        i18nText('appShell', 'language.en_us')
      );
      await act(async () => {
        fireEvent.click(english);
      });
      await waitFor(() =>
        expect(patchUserPreferences).toHaveBeenCalledWith(
          { ui: { locale: { preferred_locale: 'en_US' } } },
          'csrf'
        )
      );
      expect(useAuthStore.getState().me?.preferred_locale).toBe('en_US');
      expect(
        window.localStorage.getItem('1flowbase.ui.locale_preference')
      ).toBe('en_US');
      view.unmount();
    }
  );
});
