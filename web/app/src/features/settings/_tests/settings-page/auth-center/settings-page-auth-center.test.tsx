import { describe, test, expect } from 'vitest';
import {
  authCenterApi,
  authenticateWithPermissions,
  authenticatorConfigSchema,
  renderAuthCenterReady
} from './settings-page.fixtures';
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { useAuthStore } from '../../../../../state/auth-store';

describe('SettingsPage', () => {
  test('renders auth center actions and opens configuration drawer', async () => {
    authenticateWithPermissions(['user.view.all', 'user.manage.all']);
    authCenterApi.fetchSettingsAuthCenterOverview.mockResolvedValue({
      default_login_entry_id: 'auth-oidc-main',
      supported_auth_types: ['password-local'],
      authentication_provider_templates: [
        { auth_type: 'password-local', display_name: 'Password' }
      ],
      login_entries: [
        {
          id: 'auth-oidc-main',
          auth_type: 'oidc',
          title: 'OIDC',
          enabled: true,
          is_builtin: false,
          sort_order: 10,
          config_schema: [
            ...authenticatorConfigSchema(),
            {
              key: 'issuer_url',
              label: 'issuer_url',
              type: 'string'
            },
            {
              key: 'allow_signup',
              label: 'allow_signup',
              type: 'boolean'
            }
          ],
          config_values: {
            description: 'Primary OIDC',
            extension_config: {
              issuer_url: 'https://idp.example.com',
              allow_signup: true
            }
          }
        }
      ]
    });

    await renderAuthCenterReady();

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/auth-center');
    });
    expect(
      screen.queryByRole('heading', { name: '认证中心' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('password-local')).not.toBeInTheDocument();
    expect(screen.queryByText('auth-oidc-main')).not.toBeInTheDocument();
    expect(await screen.findByText('OIDC')).toBeInTheDocument();
    expect(
      screen.getAllByRole('columnheader').map((header) => header.textContent)
    ).toEqual(['序号', '名称', '分类', '说明', '启用', '操作']);
    expect(screen.getByText('1')).toBeInTheDocument();
    expect(screen.getByText('oidc')).toBeInTheDocument();
    expect(screen.getByText('Primary OIDC')).toBeInTheDocument();
    expect(screen.queryByText('10')).not.toBeInTheDocument();
    expect(
      screen.queryByRole('columnheader', { name: '排序值' })
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '操作' })
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole('switch'));
    await waitFor(() => {
      expect(
        authCenterApi.updateSettingsAuthCenterLoginEntryEnabled
      ).toHaveBeenCalledWith('auth-oidc-main', { enabled: false }, 'csrf-123');
    });

    fireEvent.click(screen.getByRole('button', { name: '编辑' }));
    const dialog = await screen.findByRole('dialog', { name: 'OIDC 配置' });
    expect(dialog).toBeInTheDocument();
    expect(within(dialog).queryByText('类型')).not.toBeInTheDocument();
    expect(within(dialog).queryByLabelText('标识')).not.toBeInTheDocument();
    expect(within(dialog).getByLabelText('Authenticator title')).toHaveValue(
      'OIDC'
    );
    expect(within(dialog).getByLabelText('Description')).toHaveValue(
      'Primary OIDC'
    );
    expect(
      within(dialog).getByRole('switch', { name: 'Enabled' })
    ).toBeChecked();
  });

  test('initializes auth center config form from authenticator fields', async () => {
    authenticateWithPermissions(['user.view.all', 'user.manage.all']);
    authCenterApi.fetchSettingsAuthCenterOverview.mockResolvedValue({
      default_login_entry_id: 'auth-password-local',
      supported_auth_types: ['password-local'],
      authentication_provider_templates: [
        { auth_type: 'password-local', display_name: 'Password' }
      ],
      login_entries: [
        {
          id: 'auth-password-local',
          auth_type: 'password-local',
          title: 'Password',
          enabled: true,
          is_builtin: true,
          sort_order: 0,
          config_schema: authenticatorConfigSchema(),
          config_values: {
            description: 'Local password authentication',
            extension_config: {}
          }
        }
      ]
    });

    await renderAuthCenterReady();

    fireEvent.click(await screen.findByRole('button', { name: '编辑' }));
    const dialog = await screen.findByRole('dialog', { name: 'Password 配置' });

    expect(within(dialog).queryByLabelText('标识')).not.toBeInTheDocument();
    expect(within(dialog).getByLabelText('Authenticator title')).toHaveValue(
      'Password'
    );
    expect(within(dialog).getByLabelText('Description')).toHaveValue(
      'Local password authentication'
    );
    expect(
      within(dialog).getByRole('switch', { name: 'Enabled' })
    ).toBeChecked();
    const footer = within(dialog)
      .getByRole('button', { name: /保\s*存/ })

      .closest('.ant-drawer-footer');
    expect(footer).not.toBeNull();

    expect(footer?.querySelector('.ant-flex-justify-start')).not.toBeNull();
    const footerButtons = within(footer as HTMLElement).getAllByRole('button');
    expect(footerButtons.map((button) => button.textContent)).toEqual([
      '保 存',
      '取 消'
    ]);
  });

  test('submits auth center config, refreshes the list, and closes the drawer', async () => {
    authenticateWithPermissions(['user.view.all', 'user.manage.all']);
    authCenterApi.fetchSettingsAuthCenterOverview
      .mockResolvedValueOnce({
        default_login_entry_id: 'auth-oidc-main',
        supported_auth_types: ['password-local'],
        authentication_provider_templates: [
          { auth_type: 'password-local', display_name: 'Password' }
        ],
        login_entries: [
          {
            id: 'auth-oidc-main',
            auth_type: 'oidc',
            title: 'OIDC',
            enabled: false,
            is_builtin: false,
            sort_order: 10,
            config_schema: authenticatorConfigSchema(),
            config_values: {
              title: 'OIDC',
              enabled: false,
              description: 'Old description',
              extension_config: {
                issuer_url: 'https://idp.example.com'
              }
            }
          }
        ]
      })
      .mockResolvedValueOnce({
        default_login_entry_id: 'auth-oidc-main',
        supported_auth_types: ['password-local'],
        authentication_provider_templates: [
          { auth_type: 'password-local', display_name: 'Password' }
        ],
        login_entries: [
          {
            id: 'auth-oidc-main',
            auth_type: 'oidc',
            title: 'OIDC Login',
            enabled: true,
            is_builtin: false,
            sort_order: 10,
            config_schema: authenticatorConfigSchema(),
            config_values: {
              title: 'OIDC Login',
              enabled: true,
              description: 'Primary OIDC login',
              extension_config: {
                issuer_url: 'https://idp.example.com'
              }
            }
          }
        ]
      });
    authCenterApi.updateSettingsAuthCenterLoginEntryConfig.mockResolvedValue({
      id: 'auth-oidc-main',
      auth_type: 'oidc',
      title: 'OIDC Login',
      enabled: true,
      is_builtin: false,
      sort_order: 10,
      config_schema: authenticatorConfigSchema(),
      config_values: {
        title: 'OIDC Login',
        enabled: true,
        description: 'Primary OIDC login',
        extension_config: {
          issuer_url: 'https://idp.example.com'
        }
      }
    });

    await renderAuthCenterReady();

    const editButton = await screen.findByRole('button', { name: '编辑' });
    fireEvent.click(editButton);
    const dialog = await screen.findByRole('dialog', { name: 'OIDC 配置' });

    expect(within(dialog).queryByLabelText('标识')).not.toBeInTheDocument();
    fireEvent.change(within(dialog).getByLabelText('Authenticator title'), {
      target: { value: 'OIDC Login' }
    });
    fireEvent.change(within(dialog).getByLabelText('Description'), {
      target: { value: 'Primary OIDC login' }
    });
    fireEvent.click(within(dialog).getByRole('switch', { name: 'Enabled' }));
    fireEvent.click(within(dialog).getByRole('button', { name: /保\s*存/ }));

    await waitFor(() => {
      expect(
        authCenterApi.updateSettingsAuthCenterLoginEntryConfig
      ).toHaveBeenCalledWith(
        'auth-oidc-main',
        {
          title: 'OIDC Login',
          enabled: true,
          description: 'Primary OIDC login',
          self_registration_enabled: false,
          extension_config: {}
        },
        'csrf-123'
      );
    });
    expect(screen.getByText('OIDC Login')).toBeInTheDocument();
    await waitFor(() => {
      expect(
        screen.queryByRole('dialog', { name: 'OIDC Login 配置' })
      ).not.toBeInTheDocument();
    });
  });

  test('shows auth center config errors in the drawer', async () => {
    authenticateWithPermissions(['user.view.all', 'user.manage.all']);
    authCenterApi.fetchSettingsAuthCenterOverview.mockResolvedValue({
      default_login_entry_id: 'auth-oidc-main',
      supported_auth_types: ['password-local'],
      authentication_provider_templates: [
        { auth_type: 'password-local', display_name: 'Password' }
      ],
      login_entries: [
        {
          id: 'auth-oidc-main',
          auth_type: 'oidc',
          title: 'OIDC',
          enabled: true,
          is_builtin: false,
          sort_order: 10,
          config_schema: authenticatorConfigSchema(),
          config_values: {
            title: 'OIDC',
            enabled: true,
            description: 'Old description'
          }
        }
      ]
    });
    authCenterApi.updateSettingsAuthCenterLoginEntryConfig.mockRejectedValue(
      new Error('permission denied')
    );

    await renderAuthCenterReady();

    fireEvent.click(await screen.findByRole('button', { name: '编辑' }));
    const dialog = await screen.findByRole('dialog', { name: /OIDC.*配置/ });
    fireEvent.change(within(dialog).getByLabelText('Authenticator title'), {
      target: { value: 'OIDC Login' }
    });
    fireEvent.click(within(dialog).getByRole('button', { name: /保\s*存/ }));

    expect(
      await within(dialog).findByText('permission denied')
    ).toBeInTheDocument();
  });

  test('shows auth center manage permission error in the drawer', async () => {
    authenticateWithPermissions(['user.view.all']);
    authCenterApi.fetchSettingsAuthCenterOverview.mockResolvedValue({
      default_login_entry_id: 'auth-oidc-main',
      supported_auth_types: ['password-local'],
      authentication_provider_templates: [
        { auth_type: 'password-local', display_name: 'Password' }
      ],
      login_entries: [
        {
          id: 'auth-oidc-main',
          auth_type: 'oidc',
          title: 'OIDC',
          enabled: true,
          is_builtin: false,
          sort_order: 10,
          config_schema: authenticatorConfigSchema(),
          config_values: {
            title: 'OIDC',
            enabled: true,
            description: 'Old description'
          }
        }
      ]
    });

    await renderAuthCenterReady();

    fireEvent.click(await screen.findByRole('button', { name: '编辑' }));
    const dialog = await screen.findByRole('dialog', { name: /OIDC.*配置/ });

    expect(
      within(dialog).getByText('需要登录入口管理权限。')
    ).toBeInTheDocument();
    expect(within(dialog).getByLabelText('Authenticator title')).toBeDisabled();
    expect(
      within(dialog).getByRole('switch', { name: 'Enabled' })
    ).toBeDisabled();
    expect(
      within(dialog).getByRole('button', { name: /保\s*存/ })
    ).toBeDisabled();
  });

  test('shows auth center csrf error in the drawer', async () => {
    authenticateWithPermissions(['user.view.all', 'user.manage.all']);
    useAuthStore.setState({ csrfToken: null });
    authCenterApi.fetchSettingsAuthCenterOverview.mockResolvedValue({
      default_login_entry_id: 'auth-oidc-main',
      supported_auth_types: ['password-local'],
      authentication_provider_templates: [
        { auth_type: 'password-local', display_name: 'Password' }
      ],
      login_entries: [
        {
          id: 'auth-oidc-main',
          auth_type: 'oidc',
          title: 'OIDC',
          enabled: true,
          is_builtin: false,
          sort_order: 10,
          config_schema: authenticatorConfigSchema(),
          config_values: {
            title: 'OIDC',
            enabled: true,
            description: 'Old description'
          }
        }
      ]
    });

    await renderAuthCenterReady();

    fireEvent.click(await screen.findByRole('button', { name: '编辑' }));
    const dialog = await screen.findByRole('dialog', { name: /OIDC.*配置/ });

    expect(
      within(dialog).getByText('缺少安全校验令牌，请刷新页面后重试。')
    ).toBeInTheDocument();
    expect(within(dialog).getByLabelText('Authenticator title')).toBeDisabled();
    expect(
      within(dialog).getByRole('switch', { name: 'Enabled' })
    ).toBeDisabled();
    expect(
      within(dialog).getByRole('button', { name: /保\s*存/ })
    ).toBeDisabled();
  });

  test('opens auth center configuration drawer when extension config fields are absent', async () => {
    authenticateWithPermissions(['user.view.all']);
    authCenterApi.fetchSettingsAuthCenterOverview.mockResolvedValue({
      default_login_entry_id: 'auth-password-local',
      supported_auth_types: ['password-local'],
      authentication_provider_templates: [
        { auth_type: 'password-local', display_name: 'Password' }
      ],
      login_entries: [
        {
          id: 'auth-password-local',
          auth_type: 'password-local',
          title: 'Password',
          enabled: true,
          is_builtin: true,
          sort_order: 0,
          config_schema: authenticatorConfigSchema(),
          config_values: {
            title: 'Password',
            enabled: true,
            description: null,
            extension_config: {}
          }
        }
      ]
    });

    await renderAuthCenterReady();

    const editButton = await screen.findByRole('button', { name: '编辑' });
    fireEvent.click(editButton);

    expect(
      await screen.findByRole('dialog', { name: 'Password 配置' })
    ).toBeInTheDocument();
    expect(screen.getByLabelText('Authenticator title')).toBeDisabled();
    expect(screen.getByLabelText('Authenticator title')).toHaveValue(
      'Password'
    );
    expect(screen.getByRole('switch', { name: 'Enabled' })).toBeChecked();
  });
});
