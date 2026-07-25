import {
  act,
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
  updateSettingsAuthCenterAuthenticatorPublicUiBlock: vi.fn(),
  createSettingsAuthCenterAuthenticator: vi.fn(),
  copySettingsAuthCenterAuthenticator: vi.fn(),
  deleteSettingsAuthCenterAuthenticator: vi.fn(),
  reorderSettingsAuthCenterAuthenticators: vi.fn()
}));
const frontstageInterfaceCapabilities = vi.hoisted(() => ({
  useFrontstageInterfaceCapabilities: vi.fn(() => ({
    data: { items: [], adapter_ids: [], methods: [], total: 0 },
    loading: false,
    error: null
  }))
}));

vi.mock('../api/auth-center', () => authCenterApi);
vi.mock('../../frontstage/hooks/use-frontstage-interface-capabilities', () => ({
  useFrontstageInterfaceCapabilities:
    frontstageInterfaceCapabilities.useFrontstageInterfaceCapabilities
}));
vi.mock('../../frontstage/api/interface-capabilities', () => ({
  fetchFrontstageInterfaceCapability: vi.fn()
}));

vi.mock('@monaco-editor/react', () => ({
  default: ({
    value,
    onChange
  }: {
    value?: string;
    onChange?: (value: string) => void;
  }) => (
    <textarea
      aria-label="区块源码"
      value={value}
      onChange={(event) => onChange?.(event.target.value)}
    />
  )
}));

import { AppProviders } from '../../../app/AppProviders';
import { useAuthStore } from '../../../state/auth-store';
import { SettingsAuthCenterSection } from '../pages/settings-page/SettingsAuthCenterSection';

function authenticate() {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'user-1',
      account: 'member',
      effective_display_role: 'member',
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'user-1',
      account: 'member',
      email: 'manager@example.com',
      phone: null,
      nickname: 'member',
      name: 'member',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'member',
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
      public_ui_block: 'original password block',
      interface_path_prefixes: ['/api/public/'],
      public_variables: {
        title: 'Password',
        description: 'Local password authentication',
        enabled: true,
        self_registration_enabled: false
      },
      context_variables: [
        {
          group: 'configuration',
          label: 'Authenticator title',
          member_path: 'inputs.public_variables.title',
          schema: { type: 'string' }
        },
        {
          group: 'configuration',
          label: 'Description',
          member_path: 'inputs.public_variables.description',
          schema: { type: 'string' }
        },
        {
          group: 'configuration',
          label: 'Enabled',
          member_path: 'inputs.public_variables.enabled',
          schema: { type: 'boolean' }
        },
        {
          group: 'configuration',
          label: 'Allow self registration',
          member_path: 'inputs.public_variables.self_registration_enabled',
          schema: { type: 'boolean' }
        },
        {
          group: 'runtime',
          label: 'Authenticator ID',
          member_path: 'inputs.authenticator_id',
          schema: { type: 'string', format: 'uuid' }
        },
        {
          group: 'runtime',
          label: 'Authentication event',
          member_path: 'inputs.auth_event',
          schema: { type: 'object' }
        },
        {
          group: 'runtime',
          label: 'API',
          member_path: 'api',
          schema: { type: 'object' }
        }
      ],
      config_schema: [
        {
          key: 'title',
          label: 'Authenticator title',
          type: 'string',
          required: true
        },
        {
          key: 'description',
          label: 'Description',
          type: 'string',
          control: 'textarea'
        },
        {
          key: 'enabled',
          label: 'Enabled',
          type: 'boolean',
          control: 'switch'
        },
        {
          key: 'self_registration_enabled',
          label: 'Allow self registration',
          type: 'boolean',
          control: 'switch'
        }
      ],
      config_values: {
        title: 'Password',
        enabled: true,
        description: 'Local password authentication',
        self_registration_enabled: false,
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
      public_ui_block: 'staff password block',
      interface_path_prefixes: ['/api/public/'],
      public_variables: {
        title: 'Staff Password',
        description: 'Staff login',
        enabled: false,
        self_registration_enabled: false
      },
      context_variables: [],
      config_schema: [
        {
          key: 'title',
          label: 'Authenticator title',
          type: 'string',
          required: true
        },
        {
          key: 'description',
          label: 'Description',
          type: 'string',
          control: 'textarea'
        },
        {
          key: 'enabled',
          label: 'Enabled',
          type: 'boolean',
          control: 'switch'
        },
        {
          key: 'self_registration_enabled',
          label: 'Allow self registration',
          type: 'boolean',
          control: 'switch'
        }
      ],
      config_values: {
        title: 'Staff Password',
        enabled: false,
        description: 'Staff login',
        self_registration_enabled: false,
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
    expect(within(dialog).queryByLabelText('排序值')).not.toBeInTheDocument();
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

  test('hides copy, drag-sorts, and deletes authenticators from table actions', async () => {
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

    await screen.findByText('Staff Password');
    expect(screen.queryByText('auth-password-local')).not.toBeInTheDocument();
    expect(screen.queryByText('auth-staff-password')).not.toBeInTheDocument();
    expect(
      screen.queryByRole('columnheader', { name: '排序值' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('10')).not.toBeInTheDocument();
    expect(
      screen.getAllByRole('columnheader').map((header) => header.textContent)
    ).toEqual(['序号', '名称', '分类', '说明', '启用', '操作']);

    const staffRow = await screen.findByRole('row', {
      name: /Staff Password/
    });
    expect(within(staffRow).getByText('2')).toBeInTheDocument();
    expect(within(staffRow).getByText('Staff login')).toBeInTheDocument();
    expect(within(staffRow).getByText('password-local')).toBeInTheDocument();
    expect(
      within(staffRow).queryByRole('button', { name: '复制' })
    ).not.toBeInTheDocument();
    expect(
      within(staffRow).queryByRole('button', { name: '上移' })
    ).not.toBeInTheDocument();
    expect(
      within(staffRow).queryByRole('button', { name: '下移' })
    ).not.toBeInTheDocument();

    const passwordRow = await screen.findByRole('row', {
      name: /Local password authentication/
    });
    const dragHandle = within(staffRow).getByRole('button', {
      name: '拖拽排序认证器 Staff Password'
    });
    const dataTransfer = {
      data: new Map<string, string>(),
      effectAllowed: '',
      dropEffect: '',
      setData(format: string, value: string) {
        this.data.set(format, value);
      },
      getData(format: string) {
        return this.data.get(format) ?? '';
      }
    };

    fireEvent.dragStart(dragHandle, { dataTransfer });
    fireEvent.dragOver(passwordRow, { dataTransfer });
    fireEvent.drop(passwordRow, { dataTransfer });
    await waitFor(() => {
      expect(
        authCenterApi.reorderSettingsAuthCenterAuthenticators
      ).toHaveBeenCalledWith(
        ['auth-staff-password', 'auth-password-local'],
        'csrf-123'
      );
    });

    fireEvent.click(within(staffRow).getByRole('button', { name: '删除' }));
    const confirmDeleteText = (await screen.findAllByText(/删\s*除/)).find(
      (element) => element.tagName === 'SPAN'
    );
    // Ant Design Popconfirm nests the OK text in a span inside the action button.

    const confirmDeleteButton = confirmDeleteText?.closest('button');
    expect(confirmDeleteButton).not.toBeNull();
    fireEvent.click(confirmDeleteButton as HTMLElement);
    await waitFor(() => {
      expect(
        authCenterApi.deleteSettingsAuthCenterAuthenticator
      ).toHaveBeenCalledWith('auth-staff-password', 'csrf-123');
    });
  });

  test('edits regular configuration separately from the public UI Block', async () => {
    authCenterApi.updateSettingsAuthCenterAuthenticatorConfig.mockResolvedValue(
      baseOverview.authenticators[0]
    );
    authCenterApi.updateSettingsAuthCenterAuthenticatorPublicUiBlock.mockResolvedValue(
      baseOverview.authenticators[0]
    );
    render(
      <AppProviders>
        <SettingsAuthCenterSection />
      </AppProviders>
    );

    fireEvent.click(
      (await screen.findAllByRole('button', { name: '编辑' }))[0]
    );
    const dialog = await screen.findByRole('dialog', { name: 'Password 配置' });
    expect(
      within(dialog).queryByLabelText('Public authentication block')
    ).not.toBeInTheDocument();
    expect(within(dialog).queryByLabelText('区块源码')).not.toBeInTheDocument();
    fireEvent.click(within(dialog).getByLabelText('Allow self registration'));
    fireEvent.click(within(dialog).getByRole('button', { name: /保\s*存/ }));

    await waitFor(() =>
      expect(
        authCenterApi.updateSettingsAuthCenterAuthenticatorConfig
      ).toHaveBeenCalledWith(
        'auth-password-local',
        {
          title: 'Password',
          enabled: true,
          description: 'Local password authentication',
          self_registration_enabled: true,
          extension_config: {}
        },
        'csrf-123'
      )
    );

    const passwordRow = await screen.findByRole('row', {
      name: /Local password authentication/
    });
    fireEvent.click(within(passwordRow).getByRole('button', { name: 'UI' }));
    const uiDialog = await screen.findByRole('dialog', {
      name: 'TSX 编辑器'
    });
    expect(uiDialog).toHaveClass('frontstage-jsx-studio--window');
    expect(uiDialog.closest('.ant-drawer')).toBeNull();
    expect(
      within(uiDialog).getByRole('button', { name: /重\s*置/ })
    ).toBeDisabled();
    expect(
      within(uiDialog).getByRole('button', { name: '最大化窗口' })
    ).toBeEnabled();
    expect(
      within(uiDialog).getByRole('button', { name: '关闭' })
    ).toBeEnabled();
    expect(
      within(uiDialog)
        .getAllByRole('button')
        .map((button) => button.getAttribute('aria-label'))
    ).toEqual(
      expect.arrayContaining([
        '代码',
        '接口',
        '变量',
        '组件',
        '区块设置',
        '预览'
      ])
    );
    fireEvent.click(within(uiDialog).getByRole('button', { name: '接口' }));
    expect(within(uiDialog).getByText('接口连接器')).toBeInTheDocument();
    expect(
      frontstageInterfaceCapabilities.useFrontstageInterfaceCapabilities
    ).toHaveBeenLastCalledWith(
      'workspace-1',
      expect.objectContaining({ path_prefixes: ['/api/public/'] })
    );
    fireEvent.click(within(uiDialog).getByRole('button', { name: '变量' }));
    expect(
      within(uiDialog).getByText(
        'ctx.inputs.public_variables.self_registration_enabled'
      )
    ).toBeInTheDocument();
    expect(
      within(uiDialog).getByText('Allow self registration')
    ).toBeInTheDocument();
    fireEvent.click(within(uiDialog).getByRole('button', { name: '代码' }));
    const blockEditor = within(uiDialog).getByRole('textbox', {
      name: '区块源码'
    });
    expect(blockEditor).toHaveValue('original password block');
    fireEvent.change(blockEditor, { target: { value: 'custom saved block' } });
    expect(
      within(uiDialog).getByRole('button', { name: /重\s*置/ })
    ).toBeEnabled();
    fireEvent.click(within(uiDialog).getByRole('button', { name: /保\s*存/ }));

    await waitFor(() =>
      expect(
        authCenterApi.updateSettingsAuthCenterAuthenticatorPublicUiBlock
      ).toHaveBeenCalledWith(
        'auth-password-local',
        { public_ui_block: 'custom saved block' },
        'csrf-123'
      )
    );
  });

  test('AC-014 shows the backend reason when public UI saving fails', async () => {
    authCenterApi.updateSettingsAuthCenterAuthenticatorPublicUiBlock.mockRejectedValue(
      new Error('invalid input: public_ui_block')
    );
    render(
      <AppProviders>
        <SettingsAuthCenterSection />
      </AppProviders>
    );

    const passwordRow = await screen.findByRole('row', {
      name: /Local password authentication/
    });
    fireEvent.click(within(passwordRow).getByRole('button', { name: 'UI' }));
    const uiDialog = await screen.findByRole('dialog', { name: 'TSX 编辑器' });
    const blockEditor = within(uiDialog).getByRole('textbox', {
      name: '区块源码'
    });
    fireEvent.change(blockEditor, { target: { value: 'invalid draft' } });
    fireEvent.click(within(uiDialog).getByRole('button', { name: /保\s*存/ }));

    expect(
      await within(uiDialog).findByText('invalid input: public_ui_block')
    ).toBeInTheDocument();
  });

  test('AC-020 shows one backend error when configuration saving fails', async () => {
    authCenterApi.updateSettingsAuthCenterAuthenticatorConfig.mockRejectedValue(
      new Error('invalid input: extension_config')
    );
    render(
      <AppProviders>
        <SettingsAuthCenterSection />
      </AppProviders>
    );

    fireEvent.click(
      (await screen.findAllByRole('button', { name: '编辑' }))[0]
    );
    const dialog = await screen.findByRole('dialog', { name: 'Password 配置' });
    fireEvent.click(within(dialog).getByRole('button', { name: /保\s*存/ }));

    expect(
      await within(dialog).findAllByText('invalid input: extension_config')
    ).toHaveLength(1);
  });

  test('throttles drawer mouse resize updates with animation frames', async () => {
    let animationFrameCallback: FrameRequestCallback | null = null;
    const requestAnimationFrameSpy = vi
      .spyOn(window, 'requestAnimationFrame')
      .mockImplementation((callback) => {
        animationFrameCallback = callback;
        return 123;
      });
    const cancelAnimationFrameSpy = vi
      .spyOn(window, 'cancelAnimationFrame')
      .mockImplementation(() => undefined);

    try {
      render(
        <AppProviders>
          <SettingsAuthCenterSection />
        </AppProviders>
      );

      const editButtons = await screen.findAllByRole('button', {
        name: '编辑'
      });
      fireEvent.click(editButtons[0]);
      const dialog = await screen.findByRole('dialog', {
        name: 'Password 配置'
      });
      const resizeHandle = within(dialog).getByRole('separator', {
        name: '调整认证器配置抽屉宽度'
      });
      // Ant Design applies the drawer width to a wrapper without an accessible role.

      const drawerWrapper = dialog.closest('.ant-drawer-content-wrapper');
      expect(drawerWrapper).toBeInstanceOf(HTMLElement);

      expect(resizeHandle).toHaveAttribute('aria-valuenow', '520');
      expect(drawerWrapper).toHaveStyle({ width: '520px' });
      fireEvent.mouseDown(resizeHandle, { clientX: 500 });
      fireEvent.mouseMove(document, { clientX: 460 });
      fireEvent.mouseMove(document, { clientX: 450 });

      expect(requestAnimationFrameSpy).toHaveBeenCalledTimes(1);
      expect(resizeHandle).toHaveAttribute('aria-valuenow', '520');
      expect(drawerWrapper).toHaveStyle({ width: '520px' });

      await act(async () => {
        animationFrameCallback?.(performance.now());
      });

      expect(drawerWrapper).toHaveStyle({ width: '570px' });
      expect(resizeHandle).toHaveAttribute('aria-valuenow', '520');
      fireEvent.mouseUp(document);
      expect(resizeHandle).toHaveAttribute('aria-valuenow', '570');
      expect(document.body).not.toHaveClass('schema-form-drawer--resizing');
    } finally {
      requestAnimationFrameSpy.mockRestore();
      cancelAnimationFrameSpy.mockRestore();
    }
  });
});
