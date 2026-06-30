import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const authCenterApi = vi.hoisted(() => ({
  settingsAuthCenterOverviewQueryKey: ['settings', 'auth-center', 'overview'],
  fetchSettingsAuthCenterOverview: vi.fn(),
  enableSettingsAuthCenterAuthenticator: vi.fn(),
  updateSettingsAuthCenterAuthenticatorConfig: vi.fn(),
  createSettingsAuthCenterAuthenticator: vi.fn(),
  copySettingsAuthCenterAuthenticator: vi.fn(),
  deleteSettingsAuthCenterAuthenticator: vi.fn(),
  reorderSettingsAuthCenterAuthenticators: vi.fn()
}));

vi.mock('../api/auth-center', () => authCenterApi);

import { AppProviders } from '../../../app/AppProviders';
import { useAuthStore } from '../../../state/auth-store';
import { SettingsAuthCenterSection } from '../pages/settings-page/SettingsAuthCenterSection';

function authenticate() {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'user-1',
      account: 'manager',
      effective_display_role: 'manager',
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'user-1',
      account: 'manager',
      email: 'manager@example.com',
      phone: null,
      nickname: 'manager',
      name: 'manager',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'manager',
      permissions: ['user.view.all', 'user.manage.all']
    }
  });
}

const baseOverview = {
  default_authenticator_id: 'auth-password-local',
  supported_auth_types: ['password-local'],
  authenticators: [
    {
      id: 'auth-password-local',
      auth_type: 'password-local',
      title: 'Password',
      enabled: true,
      is_builtin: true,
      sort_order: 0,
      config_schema: [],
      config_values: {
        title: 'Password',
        enabled: true,
        description: 'Local password authentication',
        extension_config: {}
      }
    },
    {
      id: 'auth-staff-password',
      auth_type: 'password-local',
      title: 'Staff Password',
      enabled: false,
      is_builtin: false,
      sort_order: 10,
      config_schema: [],
      config_values: {
        title: 'Staff Password',
        enabled: false,
        description: 'Staff login',
        extension_config: {}
      }
    }
  ]
};

describe('SettingsAuthCenterSection lifecycle', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAuthStore.getState().setAnonymous();
    authenticate();
    authCenterApi.fetchSettingsAuthCenterOverview.mockResolvedValue(
      baseOverview
    );
  });

  test('creates an authenticator with backend-supported auth types', async () => {
    authCenterApi.createSettingsAuthCenterAuthenticator.mockResolvedValue({
      ...baseOverview.authenticators[1],
      title: 'Customer Password',
      sort_order: 20
    });

    render(
      <AppProviders>
        <SettingsAuthCenterSection />
      </AppProviders>
    );

    fireEvent.click(await screen.findByRole('button', { name: '新增' }));
    const dialog = await screen.findByRole('dialog', { name: '新建认证器' });
    expect(within(dialog).getByText('password-local')).toBeInTheDocument();
    expect(within(dialog).queryByLabelText('标识')).not.toBeInTheDocument();
    fireEvent.change(within(dialog).getByLabelText('名称'), {
      target: { value: 'Customer Password' }
    });
    fireEvent.change(within(dialog).getByLabelText('说明'), {
      target: { value: 'Customer login' }
    });
    fireEvent.click(within(dialog).getByRole('button', { name: /保\s*存/ }));

    await waitFor(() => {
      expect(
        authCenterApi.createSettingsAuthCenterAuthenticator
      ).toHaveBeenCalledWith(
        {
          auth_type: 'password-local',
          title: 'Customer Password',
          description: 'Customer login',
          enabled: false,
          sort_order: 20
        },
        'csrf-123'
      );
    });
  });

  test('copies, deletes, and reorders authenticators from table actions', async () => {
    authCenterApi.copySettingsAuthCenterAuthenticator.mockResolvedValue({
      ...baseOverview.authenticators[1],
      id: 'auth-staff-password-copy',
      title: 'Staff Password Copy',
      sort_order: 20
    });
    authCenterApi.deleteSettingsAuthCenterAuthenticator.mockResolvedValue(
      undefined
    );
    authCenterApi.reorderSettingsAuthCenterAuthenticators.mockResolvedValue({
      ...baseOverview,
      authenticators: [
        baseOverview.authenticators[1],
        baseOverview.authenticators[0]
      ]
    });

    render(
      <AppProviders>
        <SettingsAuthCenterSection />
      </AppProviders>
    );

    const staffRow = (
      await screen.findByText('auth-staff-password')
    ).closest('tr');
    expect(staffRow).not.toBeNull();
    expect(
      within(staffRow as HTMLElement).getByText('auth-staff-password')
    ).toBeInTheDocument();
    expect(
      within(staffRow as HTMLElement).getByText('password-local')
    ).toBeInTheDocument();
    expect(within(staffRow as HTMLElement).getByText('10')).toBeInTheDocument();
    fireEvent.click(
      within(staffRow as HTMLElement).getByRole('button', { name: '复制' })
    );
    const dialog = await screen.findByRole('dialog', { name: '复制认证器' });
    fireEvent.click(within(dialog).getByRole('button', { name: /保\s*存/ }));
    await waitFor(() => {
      expect(
        authCenterApi.copySettingsAuthCenterAuthenticator
      ).toHaveBeenCalledWith(
        'auth-staff-password',
        {
          title: 'Staff Password Copy',
          sort_order: 20
        },
        'csrf-123'
      );
    });

    fireEvent.click(
      within(staffRow as HTMLElement).getByRole('button', { name: '上移' })
    );
    await waitFor(() => {
      expect(
        authCenterApi.reorderSettingsAuthCenterAuthenticators
      ).toHaveBeenCalledWith(
        ['auth-staff-password', 'auth-password-local'],
        'csrf-123'
      );
    });

    fireEvent.click(
      within(staffRow as HTMLElement).getByRole('button', { name: '删除' })
    );
    const confirmDeleteText = (await screen.findAllByText(/删\s*除/)).find(
      (element) => element.tagName === 'SPAN'
    );
    const confirmDeleteButton = confirmDeleteText?.closest('button');
    expect(confirmDeleteButton).not.toBeNull();
    fireEvent.click(confirmDeleteButton as HTMLElement);
    await waitFor(() => {
      expect(
        authCenterApi.deleteSettingsAuthCenterAuthenticator
      ).toHaveBeenCalledWith('auth-staff-password', 'csrf-123');
    });
  });
});
