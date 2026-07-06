import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { Grid } from 'antd';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const { updateMyProfile, changeMyPassword, fetchMyProfile } = vi.hoisted(() => ({
  updateMyProfile: vi.fn(),
  changeMyPassword: vi.fn(),
  fetchMyProfile: vi.fn()
}));

const ME_PROFILE_FLOW_TEST_TIMEOUT = 15_000;

vi.mock('../api/me', () => ({
  updateMyProfile,
  changeMyPassword,
  fetchMyProfile
}));

import { AppProviders } from '../../../app/AppProviders';
import { AppRouterProvider } from '../../../app/router';
import { appI18n } from '../../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';

const useBreakpointSpy = vi.spyOn(Grid, 'useBreakpoint');

function authenticate(
  preferredLocale: string | null = null,
  meta: Record<string, unknown> | undefined = undefined
) {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'user-1',
      account: 'root',
      effective_display_role: 'manager',
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'user-1',
      account: 'root',
      email: 'root@example.com',
      phone: null,
      nickname: 'Root',
      name: 'Root',
      avatar_url: null,
      introduction: '',
      preferred_locale: preferredLocale,
      meta,
      effective_display_role: 'manager',
      permissions: []
    }
  });
}

function renderApp(pathname: string) {
  window.history.pushState({}, '', pathname);

  return render(
    <AppProviders>
      <AppRouterProvider />
    </AppProviders>
  );
}

describe('MePage', () => {
  beforeEach(async () => {
    resetAuthStore();
    window.localStorage.setItem('1flowbase.ui.locale_preference', 'zh_Hans');
    await appI18n.changeLanguage('zh_Hans');
    useBreakpointSpy.mockReturnValue({
      xs: true,
      sm: true,
      md: true,
      lg: true,
      xl: false,
      xxl: false
    });
    updateMyProfile.mockReset();
    changeMyPassword.mockReset();
    fetchMyProfile.mockReset();
    authenticate();
  });

  test(
    'me page redirects /me to /me/profile',
    async () => {
      renderApp('/me');

      await waitFor(() => {
        expect(window.location.pathname).toBe('/me/profile');
      });
      expect(await screen.findByRole('heading', { name: '个人资料', level: 4 })).toBeInTheDocument();
    },
    15000
  );

  test('does not render sign-out inside the /me sidebar', async () => {
    renderApp('/me/profile');

    expect(await screen.findByRole('heading', { name: '个人资料', level: 4 })).toBeInTheDocument();
    expect(screen.getByRole('navigation', { name: 'Section navigation' })).toBeInTheDocument();
    expect(screen.getByTestId('section-page-layout')).toHaveClass('section-page-layout--narrow');
    expect(screen.queryByText('退出登录')).not.toBeInTheDocument();
  });

  test(
    'keeps profile update flow working on /me/profile',
    async () => {
      updateMyProfile.mockResolvedValue({
        id: 'user-1',
        account: 'root',
        email: 'root-next@example.com',
        phone: '13900000000',
        nickname: 'Captain Root',
        name: 'Root Next',
        avatar_url: null,
        introduction: 'updated intro',
        preferred_locale: null,
        effective_display_role: 'manager',
        permissions: []
      });

      renderApp('/me/profile');

      expect(await screen.findByRole('heading', { name: '个人资料', level: 4 })).toBeInTheDocument();
      fireEvent.click(screen.getByRole('button', { name: /编辑资料/ }));

      await waitFor(() => {
        expect(screen.getByText('编辑个人信息')).toBeInTheDocument();
      });
      expect(screen.getByLabelText('界面语言')).toBeInTheDocument();

      fireEvent.change(screen.getByLabelText('姓名'), {
        target: { value: 'Root Next' }
      });
      fireEvent.change(screen.getByLabelText('昵称'), {
        target: { value: 'Captain Root' }
      });
      fireEvent.change(screen.getByLabelText('联系邮箱'), {
        target: { value: 'root-next@example.com' }
      });
      fireEvent.change(screen.getByLabelText('手机号码'), {
        target: { value: '13900000000' }
      });
      fireEvent.click(screen.getByRole('button', { name: '保存资料' }));

      await waitFor(() =>
        expect(updateMyProfile).toHaveBeenCalledWith(
          {
            name: 'Root Next',
            nickname: 'Captain Root',
            email: 'root-next@example.com',
            phone: '13900000000',
            avatar_url: null,
            introduction: '',
            preferred_locale: null
          },
          'csrf-123'
        )
      );
    },
    ME_PROFILE_FLOW_TEST_TIMEOUT
  );

  test('renders profile form copy in English when preferred locale is English', async () => {
    resetAuthStore();
    authenticate('en_US');

    renderApp('/me/profile');

    expect(
      await screen.findByRole('heading', { name: 'Personal information', level: 4 })
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Edit profile/ })).toBeInTheDocument();
    expect(screen.getAllByText('Interface language').length).toBeGreaterThan(0);
  });

  test('renders profile form copy in English when locale preference comes from me meta', async () => {
    resetAuthStore();
    authenticate(null, {
      ui: {
        locale: {
          preferred_locale: 'en_US'
        }
      }
    });

    renderApp('/me/profile');

    expect(
      await screen.findByRole('heading', { name: 'Personal information', level: 4 })
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Edit profile/ })).toBeInTheDocument();
  });

  test('submits password change on /me/security and navigates to /sign-in after success', async () => {
    changeMyPassword.mockResolvedValue(undefined);

    renderApp('/me/security');

    expect(await screen.findByRole('heading', { name: '安全设置', level: 3 })).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('密码'), {
      target: { value: 'old-password' }
    });
    fireEvent.change(screen.getByLabelText('新密码'), {
      target: { value: 'new-password-123' }
    });
    fireEvent.change(screen.getByLabelText('确认新密码'), {
      target: { value: 'new-password-123' }
    });
    fireEvent.click(screen.getByRole('button', { name: '更新密码' }));

    await waitFor(() => {
      expect(changeMyPassword).toHaveBeenCalledWith(
        {
          old_password: 'old-password',
          new_password: 'new-password-123'
        },
        'csrf-123'
      );
    });
    await waitFor(() => {
      expect(window.location.pathname).toBe('/sign-in');
    });
  });
});
