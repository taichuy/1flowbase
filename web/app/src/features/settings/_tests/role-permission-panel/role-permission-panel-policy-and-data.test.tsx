import { describe, test, expect } from 'vitest';
import {
  allRowScopeOptions,
  appI18n,
  consolePolicyCatalog,
  fireEvent,
  path,
  permissionsApi,
  policyModeOptions,
  renderPanel,
  rolesApi,
  screen,
  selectDefaultPolicyScope,
  selectPolicyCombobox,
  setPreferredLocale,
  waitFor,
  within
} from './role-permission-panel.fixtures';

describe('RolePermissionPanel', () => {
  test('AC-004 renders a catalog group absent from stored policy as disabled and enables it as full', async () => {
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue(
      consolePolicyCatalog([
        {
          kind: 'settings_feature',
          group_id: 'settings.missing',
          label: '缺失策略组',
          description: '后端 catalog 中存在，角色策略中缺失',
          operations: [
            {
              operation_id: 'missing.read',
              summary: 'View missing resource',
              description: '读取当前空间资源',
              order: 1,
              route: { method: 'GET', path: '/api/console/test' },
              full_profile: { kind: 'row', scope: 'scope_all' },
              allowed_row_scopes: allRowScopeOptions,
              authorization: {
                kind: 'resource_action',
                resource_code: 'missing_resource',
                action_code: 'read'
              }
            }
          ]
        }
      ])
    );
    rolesApi.fetchSettingsRoleConsolePolicy.mockResolvedValue({
      role_code: 'member',
      groups: []
    });

    renderPanel();

    const policySelect = await screen.findByRole('combobox', {
      name: '缺失策略组 授权策略'
    });
    expect(policySelect.closest('.ant-select')).toHaveTextContent('全部操作');
    const missingSwitch = screen.getByRole('switch', {
      name: '缺失策略组 启用'
    });
    expect(missingSwitch).not.toBeChecked();
    fireEvent.click(missingSwitch);

    await waitFor(() => {
      expect(rolesApi.replaceSettingsRoleConsolePolicy).toHaveBeenCalledWith(
        'member',
        {
          groups: [
            {
              kind: 'settings_feature',
              group_id: 'settings.missing',
              enabled: true,
              strategy: 'full',
              operations: []
            }
          ]
        },
        'csrf-123'
      );
    });
  });

  test('AC-004 projects the server full profile before converting a detail edit to explicit custom policies', async () => {
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue(
      consolePolicyCatalog([
        {
          kind: 'settings_feature',
          group_id: 'settings.full-profile',
          label: '完整策略组',
          description: '',
          operations: [
            {
              operation_id: 'full-profile.read',
              summary: 'List records',
              description: '',
              order: 1,
              route: { method: 'GET', path: '/api/console/test' },
              full_profile: { kind: 'row', scope: 'scope_all' },
              allowed_row_scopes: allRowScopeOptions,
              authorization: {
                kind: 'resource_action',
                resource_code: 'full_profile_resource',
                action_code: 'read'
              }
            },
            {
              operation_id: 'full-profile.publish',
              summary: 'Publish record',
              description: '',
              order: 2,
              route: { method: 'GET', path: '/api/console/test' },
              full_profile: { kind: 'simple', enabled: true },
              allowed_row_scopes: [],
              authorization: { kind: 'simple' }
            }
          ]
        }
      ])
    );
    rolesApi.fetchSettingsRoleConsolePolicy.mockResolvedValue({
      role_code: 'member',
      groups: [
        {
          kind: 'settings_feature',
          group_id: 'settings.full-profile',
          enabled: true,
          strategy: 'full',
          operations: []
        }
      ]
    });

    renderPanel();

    fireEvent.click(
      await screen.findByRole(
        'button',
        { name: '详细配置 完整策略组' },
        { timeout: 5000 }
      )
    );
    const drawer = await screen.findByRole('dialog');
    expect(within(drawer).getByText('当前空间')).toBeInTheDocument();
    expect(
      within(drawer).getByRole('switch', { name: 'Publish record' })
    ).toBeChecked();

    fireEvent.mouseDown(
      within(drawer).getByRole('combobox', { name: 'List records 作用域' })
    );
    fireEvent.click((await screen.findAllByTitle('仅自己')).at(-1)!);
    fireEvent.click(
      within(drawer).getByRole('button', { name: '保存权限配置' })
    );

    await waitFor(() => {
      expect(rolesApi.replaceSettingsRoleConsolePolicy).toHaveBeenCalledWith(
        'member',
        {
          groups: [
            {
              kind: 'settings_feature',
              group_id: 'settings.full-profile',
              enabled: true,
              strategy: 'custom',
              operations: [
                {
                  operation_id: 'full-profile.read',
                  kind: 'row',
                  scope: 'own'
                },
                {
                  operation_id: 'full-profile.publish',
                  kind: 'simple',
                  enabled: true
                }
              ]
            }
          ]
        },
        'csrf-123'
      );
    });
  });

  test('AC-002 restores a custom group to full while retaining explicit operations', async () => {
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue(
      consolePolicyCatalog([
        {
          kind: 'settings_feature',
          group_id: 'settings.restore-full',
          label: '可恢复策略组',
          description: '',
          operations: [
            {
              operation_id: 'restore-full.publish',
              summary: 'Publish',
              description: '',
              order: 1,
              route: { method: 'GET', path: '/api/console/test' },
              full_profile: { kind: 'simple', enabled: true },
              allowed_row_scopes: [],
              authorization: { kind: 'simple' }
            }
          ]
        }
      ])
    );
    rolesApi.fetchSettingsRoleConsolePolicy.mockResolvedValue({
      role_code: 'member',
      groups: [
        {
          kind: 'settings_feature',
          group_id: 'settings.restore-full',
          enabled: true,
          strategy: 'custom',
          operations: [
            {
              operation_id: 'restore-full.publish',
              kind: 'simple',
              enabled: false
            }
          ]
        }
      ]
    });

    renderPanel();

    await screen.findByRole('combobox', {
      name: '可恢复策略组 授权策略'
    });
    await selectPolicyCombobox(
      document.body,
      '可恢复策略组 授权策略',
      '全部操作'
    );

    await waitFor(() => {
      expect(rolesApi.replaceSettingsRoleConsolePolicy).toHaveBeenCalledWith(
        'member',
        {
          groups: [
            {
              kind: 'settings_feature',
              group_id: 'settings.restore-full',
              enabled: true,
              strategy: 'full',
              operations: [
                {
                  operation_id: 'restore-full.publish',
                  kind: 'simple',
                  enabled: false
                }
              ]
            }
          ]
        },
        'csrf-123'
      );
    });
  });

  test("AC-004/009 renders only a row operation's backend-supplied allowed scopes", async () => {
    const limitedRowScopeOptions = [
      { value: 'disabled', label: '目录关闭', description: '不授予访问' },
      { value: 'own', label: '目录中的本人记录', description: '仅自己的记录' }
    ];
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue(
      consolePolicyCatalog([
        {
          kind: 'other',
          group_id: 'other.limited-scopes',
          label: '受限范围',
          description: '',
          operations: [
            {
              operation_id: 'limited-scope.read',
              summary: 'Read limited records',
              description: '',
              order: 1,
              route: { method: 'GET', path: '/api/console/test' },
              full_profile: { kind: 'row', scope: 'own' },
              allowed_row_scopes: limitedRowScopeOptions,
              authorization: {
                kind: 'resource_action',
                resource_code: 'limited_scope_resource',
                action_code: 'read'
              }
            }
          ]
        }
      ])
    );
    rolesApi.fetchSettingsRoleConsolePolicy.mockResolvedValue({
      role_code: 'member',
      groups: [
        {
          kind: 'other',
          group_id: 'other.limited-scopes',
          enabled: true,
          strategy: 'custom',
          operations: [
            {
              operation_id: 'limited-scope.read',
              kind: 'row',
              scope: 'own'
            }
          ]
        }
      ]
    });

    const { container } = renderPanel();
    fireEvent.click(await screen.findByRole('tab', { name: '其他' }));
    fireEvent.click(screen.getByRole('button', { name: '详细配置 受限范围' }));
    const drawer = await screen.findByRole('dialog');

    fireEvent.mouseDown(
      within(drawer).getByRole('combobox', {
        name: 'Read limited records 作用域'
      })
    );
    expect((await screen.findAllByTitle('目录关闭')).length).toBeGreaterThan(0);
    expect(
      (await screen.findAllByTitle('目录中的本人记录')).length
    ).toBeGreaterThan(0);
    expect(screen.queryByTitle('当前空间')).not.toBeInTheDocument();
    expect(screen.queryByTitle('本空间')).not.toBeInTheDocument();
    expect(container.innerHTML).not.toContain('other.limited-scopes');
    expect(container.innerHTML).not.toContain('limited-scope.read');
    expect(container.innerHTML).not.toContain('limited_scope_resource');
  });

  test('AC-004 dynamic route save reloads persisted selection without persisting the group id', async () => {
    renderPanel();

    fireEvent.click(await screen.findByRole('tab', { name: '动态路由' }));

    expect(await screen.findByText('工作台')).toBeInTheDocument();
    expect(screen.queryByText(/pgr3083h/)).not.toBeInTheDocument();

    const savedRoutes = await rolesApi.fetchSettingsRoleFrontstageRoutes();
    rolesApi.replaceSettingsRoleFrontstageRoutes.mockImplementation(
      async () => {
        rolesApi.fetchSettingsRoleFrontstageRoutes.mockResolvedValue({
          ...savedRoutes,
          checked_page_ids: ['child-page'],
          checked_tab_ids: ['child-tab']
        });
      }
    );
    rolesApi.fetchSettingsRoleFrontstageRoutes.mockClear();

    fireEvent.click(screen.getByRole('checkbox', { name: /工作台/ }));

    await waitFor(() => {
      expect(rolesApi.replaceSettingsRoleFrontstageRoutes).toHaveBeenCalledWith(
        'member',
        {
          page_ids: ['child-page'],
          tab_ids: ['child-tab']
        },
        'csrf-123'
      );
    });
    await waitFor(() =>
      expect(rolesApi.fetchSettingsRoleFrontstageRoutes).toHaveBeenCalled()
    );
    expect(screen.getByRole('checkbox', { name: /工作台/ })).toBeChecked();
  });

  test('AC-003 permission catalog failure is explicit and can be retried', async () => {
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockRejectedValue(
      new Error('forbidden')
    );
    renderPanel(false);
    expect(await screen.findByRole('alert')).toHaveTextContent(
      '权限数据加载失败'
    );
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue(
      consolePolicyCatalog([])
    );
    fireEvent.click(screen.getByRole('button', { name: /重\s*试/ }));
    expect(
      await screen.findByRole('tab', { name: '动态路由' })
    ).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('checkbox', { name: /工作台/ }));
    expect(rolesApi.replaceSettingsRoleFrontstageRoutes).not.toHaveBeenCalled();
  });

  test('AC-004 failed route save reloads and restores the persisted selection', async () => {
    rolesApi.replaceSettingsRoleFrontstageRoutes.mockRejectedValue(
      new Error('forbidden')
    );
    renderPanel();
    fireEvent.click(await screen.findByRole('tab', { name: '动态路由' }));
    rolesApi.fetchSettingsRoleFrontstageRoutes.mockClear();
    fireEvent.click(screen.getByRole('checkbox', { name: /工作台/ }));
    await screen.findByText('权限配置更新失败');
    await waitFor(() =>
      expect(rolesApi.fetchSettingsRoleFrontstageRoutes).toHaveBeenCalled()
    );
    await waitFor(() =>
      expect(screen.getByRole('checkbox', { name: /工作台/ })).not.toBeChecked()
    );
  });

  test('submits auto_grant_new_permissions and is_default_member_role from the create and edit dialogs', async () => {
    renderPanel();

    await screen.findByRole('button', { name: /新建角色/ });

    fireEvent.click(screen.getByRole('button', { name: /新建角色/ }));

    const createDialog = await screen.findByRole('dialog');
    fireEvent.change(within(createDialog).getByLabelText('角色名称'), {
      target: { value: 'QA' }
    });
    fireEvent.change(within(createDialog).getByLabelText('角色编码'), {
      target: { value: 'qa' }
    });
    fireEvent.click(
      within(createDialog).getByRole('checkbox', {
        name: '自动接收后续新增权限'
      })
    );
    fireEvent.click(
      within(createDialog).getByRole('button', { name: /确\s*定/u })
    );

    await waitFor(() => {
      expect(rolesApi.createSettingsRole).toHaveBeenCalledWith(
        {
          code: 'qa',
          name: 'QA',
          introduction: '',
          auto_grant_new_permissions: true,
          is_default_member_role: false
        },
        'csrf-123'
      );
    });

    fireEvent.click(screen.getByRole('button', { name: /编辑基本信息/ }));

    const editDialog = await screen.findByRole('dialog');
    expect(
      within(editDialog).getByRole('checkbox', { name: '默认新用户角色' })
    ).toBeChecked();
    expect(
      within(editDialog).getByRole('checkbox', { name: '自动接收后续新增权限' })
    ).not.toBeChecked();

    fireEvent.change(within(editDialog).getByLabelText('角色名称'), {
      target: { value: 'Manager Updated' }
    });
    fireEvent.click(
      within(editDialog).getByRole('checkbox', { name: '自动接收后续新增权限' })
    );
    fireEvent.click(
      within(editDialog).getByRole('checkbox', { name: '默认新用户角色' })
    );
    fireEvent.click(
      within(editDialog).getByRole('button', { name: /确\s*定/u })
    );

    await waitFor(() => {
      expect(rolesApi.updateSettingsRole).toHaveBeenCalledWith(
        'member',
        {
          name: 'Manager Updated',
          introduction: '默认管理角色',
          auto_grant_new_permissions: true,
          is_default_member_role: false
        },
        'csrf-123'
      );
    });
  }, 20000);

  test('submits default data policy without a create scope', async () => {
    renderPanel();

    await screen.findByRole('tab', { name: '动态路由' });
    expect(screen.getAllByRole('tab').map((tab) => tab.textContent)).toEqual([
      '动态路由',
      '表-通用配置',
      '表-单独配置'
    ]);
    expect(screen.getAllByRole('tablist')).toHaveLength(1);
    expect(screen.queryByText('Data Model 数据权限')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('tab', { name: '表-通用配置' }));

    expect(screen.queryByText('Data Model 数据权限')).not.toBeInTheDocument();

    const defaultSection = await screen.findByRole('region', {
      name: '默认策略'
    });
    expect(
      within(defaultSection).queryByRole('radiogroup', { name: '新增范围' })
    ).not.toBeInTheDocument();
    expect(
      within(defaultSection).queryByRole('switch', { name: '新增' })
    ).not.toBeInTheDocument();
    expect(within(defaultSection).queryByRole('tree')).not.toBeInTheDocument();
    expect(within(defaultSection).getByRole('table')).toBeInTheDocument();
    expect(
      within(defaultSection).getByRole('checkbox', { name: '新增 启用' })
    ).toBeInTheDocument();
    expect(
      within(defaultSection).getByRole('combobox', { name: '查看 作用域' })
    ).toBeInTheDocument();

    await waitFor(() => {
      expect(
        within(defaultSection).getByRole('checkbox', {
          name: '新增 启用'
        })
      ).toBeChecked();
    });
    await selectDefaultPolicyScope(defaultSection, '查看', '本空间');
    await selectDefaultPolicyScope(defaultSection, '更新', '仅自己');
    fireEvent.click(screen.getByRole('button', { name: '保存数据权限' }));

    await waitFor(() => {
      expect(rolesApi.replaceSettingsRoleDataPolicy).toHaveBeenCalledWith(
        'member',
        {
          default_policy: {
            can_view: true,
            can_create: true,
            can_update: true,
            can_delete: false,
            default_view_scope: 'scope_all',
            default_update_scope: 'own',
            default_delete_scope: 'own'
          },
          model_policies: [
            {
              data_model_id: 'model-orders',
              can_create_override: null,
              view_scope_override: null,
              update_scope_override: 'own',
              delete_scope_override: 'scope_all'
            },
            {
              data_model_id: 'model-customers',
              can_create_override: null,
              view_scope_override: null,
              update_scope_override: null,
              delete_scope_override: null
            }
          ]
        },
        'csrf-123'
      );
    });
  }, 20000);

  test('submits per-model override payload and hides system_all choices', async () => {
    renderPanel();

    await screen.findByRole('tab', { name: '动态路由' });
    fireEvent.click(screen.getByRole('tab', { name: '表-单独配置' }));

    const ordersRow = await screen.findByRole('row', { name: /Orders orders/ });
    expect(screen.queryByText('所有数据')).not.toBeInTheDocument();
    expect(
      within(ordersRow).queryByRole('radio', { name: '查看 本空间' })
    ).not.toBeInTheDocument();
    expect(
      within(ordersRow).getByRole('combobox', { name: '查看 Orders' })
    ).toBeInTheDocument();
    expect(
      within(ordersRow).getByRole('checkbox', { name: '新增 Orders' })
    ).toBeChecked();

    fireEvent.click(
      within(ordersRow).getByRole('checkbox', { name: '新增 Orders' })
    );
    await selectPolicyCombobox(ordersRow, '查看 Orders', '本空间');
    await selectPolicyCombobox(ordersRow, '更新 Orders', '继承');
    await selectPolicyCombobox(ordersRow, '删除 Orders', '仅自己');
    fireEvent.click(screen.getByRole('button', { name: '保存数据权限' }));

    await waitFor(() => {
      expect(rolesApi.replaceSettingsRoleDataPolicy).toHaveBeenCalledWith(
        'member',
        expect.objectContaining({
          model_policies: expect.arrayContaining([
            {
              data_model_id: 'model-orders',
              can_create_override: false,
              view_scope_override: 'scope_all',
              update_scope_override: null,
              delete_scope_override: 'own'
            }
          ])
        }),
        'csrf-123'
      );
    });
  }, 20000);

  test('disables data policy controls when the user cannot manage roles', async () => {
    renderPanel(false);

    await screen.findByRole('tab', { name: '动态路由' });
    fireEvent.click(screen.getByRole('tab', { name: '表-通用配置' }));

    const defaultSection = await screen.findByRole('region', {
      name: '默认策略'
    });

    expect(
      within(defaultSection).getByRole('checkbox', { name: '查看 启用' })
    ).toBeDisabled();
    expect(
      within(defaultSection).getByRole('combobox', { name: '查看 作用域' })
    ).toBeDisabled();
    expect(screen.getByRole('button', { name: '保存数据权限' })).toBeDisabled();
  }, 20000);

  test('AC-009/013 requests the active locale and renders backend values without local interface mapping', async () => {
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockImplementation(
      async (locale) => {
        const isEnglish = locale === 'en_US';
        return {
          schema_version: '2026-07-14',
          locale,
          group_strategy_options: policyModeOptions,
          groups: [
            {
              kind: 'other' as const,
              group_id: 'other.general',
              label: isEnglish ? 'Other settings' : '其他设置',
              description: null,
              operations: [
                {
                  operation_id: 'audit.export',
                  summary: isEnglish
                    ? 'Export audit records'
                    : '导出审计记录（后端）',
                  description: isEnglish
                    ? 'Export audit records in the system backend.'
                    : '从系统后端导出审计记录。',
                  order: 1,
                  route: { method: 'GET', path: '/api/console/test' },
                  full_profile: { kind: 'simple', enabled: true },
                  allowed_row_scopes: [],
                  authorization: { kind: 'simple' as const }
                }
              ]
            }
          ],
          resources: []
        };
      }
    );
    rolesApi.fetchSettingsRoleConsolePolicy.mockResolvedValue({
      role_code: 'member',
      groups: [
        {
          kind: 'other',
          group_id: 'other.general',
          enabled: false,
          strategy: 'full',
          operations: [
            { operation_id: 'audit.export', kind: 'simple', enabled: false }
          ]
        }
      ]
    });

    setPreferredLocale('en_US');
    await appI18n.changeLanguage('en_US');
    const view = renderPanel();
    await waitFor(() => {
      expect(
        permissionsApi.fetchSettingsConsolePolicyCatalog
      ).toHaveBeenCalledWith('en_US');
    });
    fireEvent.click(await screen.findByRole('tab', { name: 'Others' }));
    expect(await screen.findByText('Other settings')).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole('button', { name: 'Configure Other settings' })
    );
    const englishDrawer = await screen.findByRole('dialog');
    expect(
      within(englishDrawer).getByText('Export audit records')
    ).toBeInTheDocument();
    expect(
      within(englishDrawer).getByText(
        'Export audit records in the system backend.'
      )
    ).toBeInTheDocument();
    expect(screen.queryByText('other.general')).not.toBeInTheDocument();
    expect(screen.queryByText('audit.export')).not.toBeInTheDocument();
    expect(
      permissionsApi.settingsConsolePolicyCatalogQueryKey
    ).toHaveBeenCalledWith('en_US');
    view.unmount();

    setPreferredLocale('zh_Hans');
    await appI18n.changeLanguage('zh_Hans');
    renderPanel();
    await waitFor(() => {
      expect(
        permissionsApi.fetchSettingsConsolePolicyCatalog
      ).toHaveBeenCalledWith('zh_Hans');
    });
    fireEvent.click(await screen.findByRole('tab', { name: '其他' }));
    expect(await screen.findByText('其他设置')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '详细配置 其他设置' }));
    const chineseDrawer = await screen.findByRole('dialog');
    expect(
      within(chineseDrawer).getByText('导出审计记录（后端）')
    ).toBeInTheDocument();
    expect(
      within(chineseDrawer).getByText('从系统后端导出审计记录。')
    ).toBeInTheDocument();
    expect(
      within(chineseDrawer).queryByText('Export audit records')
    ).not.toBeInTheDocument();
    expect(screen.queryByText('other.general')).not.toBeInTheDocument();
    expect(screen.queryByText('audit.export')).not.toBeInTheDocument();
    expect(
      permissionsApi.settingsConsolePolicyCatalogQueryKey
    ).toHaveBeenCalledWith('zh_Hans');
  }, 20000);
});
