import { useNavigate } from '@tanstack/react-router';
import { useQueryClient } from '@tanstack/react-query';
import { App, Menu } from 'antd';
import type { MenuProps } from 'antd';
import { useState } from 'react';

import { switchActiveRole } from '../features/auth/api/session';
import { i18nText } from '../shared/i18n/text';
import { useAuthStore } from '../state/auth-store';
import {
  createAccountMenuClickHandler,
  selectAccountLabel
} from './account-menu-actions';
import { createAccountMenuItems } from './account-menu-items';

interface AccountMenuBaseProps {
  navigateTo: (path: '/me' | '/sign-in') => Promise<void> | void;
  navigateHome: () => Promise<void> | void;
}

function AccountMenuBase({ navigateTo, navigateHome }: AccountMenuBaseProps) {
  const queryClient = useQueryClient();
  const { message } = App.useApp();
  const [switchingRoleCode, setSwitchingRoleCode] = useState<string | null>(
    null
  );
  const {
    csrfToken,
    actor,
    me,
    availableRoles,
    setAuthenticated,
    setAnonymous
  } = useAuthStore();
  const accountLabel = selectAccountLabel({ me, actor });
  const handleClick = createAccountMenuClickHandler({
    csrfToken,
    setAnonymous,
    navigateTo
  });

  const handleMenuClick: NonNullable<MenuProps['onClick']> = (event) => {
    if (!event.key.startsWith('role:')) {
      handleClick?.(event);
      return;
    }
    const roleCode = event.key.slice('role:'.length);
    if (!csrfToken || !me || roleCode === actor?.effective_display_role) return;
    void (async () => {
      setSwitchingRoleCode(roleCode);
      try {
        const session = await switchActiveRole(roleCode, csrfToken);
        setAuthenticated({
          csrfToken: session.csrf_token,
          actor: session.actor,
          me: {
            ...me,
            effective_display_role: session.actor.effective_display_role,
            permissions: session.active_role_permissions
          },
          availableRoles: session.available_roles
        });
        queryClient.clear();
        await navigateHome();
      } catch {
        void message.error(
          i18nText('appShell', 'auto.switch_role_failed', {
            defaultValue: 'Failed to switch role'
          })
        );
      } finally {
        setSwitchingRoleCode(null);
      }
    })();
  };

  return (
    <Menu
      className="app-shell-account-menu"
      mode="horizontal"
      selectable={false}
      items={createAccountMenuItems(
        accountLabel,
        availableRoles,
        actor?.effective_display_role,
        switchingRoleCode
      )}
      onClick={handleMenuClick}
      disabledOverflow
    />
  );
}

function RoutedAccountMenu() {
  const navigate = useNavigate();

  return (
    <AccountMenuBase
      navigateTo={(path) => navigate({ to: path })}
      navigateHome={() => navigate({ to: '/' })}
    />
  );
}

function StaticAccountMenu() {
  return (
    <AccountMenuBase
      navigateTo={(path) => {
        window.history.pushState({}, '', path);
        window.dispatchEvent(new PopStateEvent('popstate'));
      }}
      navigateHome={() => {
        window.history.pushState({}, '', '/');
        window.dispatchEvent(new PopStateEvent('popstate'));
      }}
    />
  );
}

export function AccountMenu({
  useRouterNavigation = false
}: {
  useRouterNavigation?: boolean;
}) {
  return useRouterNavigation ? <RoutedAccountMenu /> : <StaticAccountMenu />;
}
