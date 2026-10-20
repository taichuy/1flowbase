import { describe, test, expect } from 'vitest';
import {
  permissionsApi,
  renderPanel,
  rolesApi
} from './role-permission-panel.fixtures';
import {
  allRowScopeOptions,
  consolePolicyCatalog,
  policyModeOptions,
  selectPolicyCombobox
} from './fixtures';
import {
  findReorderIndices,
  reorderItems
} from '../../components/role-permissions/policy-order';
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import fs from 'node:fs';
import path from 'node:path';

describe('RolePermissionPanel', () => {
  test('AC-008 stacks the role rail above policy content at 390px', async () => {
    renderPanel();

    expect(await screen.findByTestId('role-permission-layout')).toHaveClass(
      'role-permission-layout'
    );
    expect(screen.getByTestId('role-permission-rail')).toHaveClass(
      'role-permission-layout__rail'
    );
    expect(screen.getByTestId('role-permission-content')).toHaveClass(
      'role-permission-layout__content'
    );

    const layoutCss = fs.readFileSync(
      path.resolve(
        import.meta.dirname,
        '../../components/role-permissions/role-permission-panel.css'
      ),
      'utf8'
    );
    const mobileRule = layoutCss.match(
      /@media \(max-width: 767px\) \{([\s\S]*?)\n\}/
    )?.[1];

    expect(mobileRule).toContain(
      '.role-permission-layout {\n    flex-direction: column;'
    );
    expect(mobileRule).toContain(
      '.role-permission-layout__rail {\n    width: 100%;'
    );
    expect(mobileRule).toContain(
      '.role-permission-layout__content {\n    min-width: 0;'
    );
  });

  test('reorders backend settings by drag source and destination indexes', () => {
    const first = {
      kind: 'settings_feature',
      group_id: 'system.roles',
      label: '角色',
      description: '角色设置',
      operations: []
    };
    const second = {
      kind: 'settings_feature',
      group_id: 'system.members',
      label: '成员',
      description: '成员设置',
      operations: []
    };
    expect(
      reorderItems([first, second], 0, 1).map((group) => group.group_id)
    ).toEqual(['system.members', 'system.roles']);
    expect(
      findReorderIndices(
        ['system.roles', 'system.members'],
        'system.roles',
        'system.members'
      )
    ).toEqual([0, 1]);
  });

  test('AC-003/008/009 renders backend-owned Other labels without exposing codes', async () => {
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue({
      schema_version: '2026-07-14',
      locale: 'zh_Hans',
      group_strategy_options: policyModeOptions,
      groups: [
        {
          kind: 'settings_feature',
          group_id: 'settings.applications',
          label: '应用管理',
          description: '管理当前空间中的应用',
          operations: [
            {
              operation_id: 'applications.read',
              summary: 'List applications',
              description: '查看应用记录',
              order: 1,
              route: {
                method: 'GET',
                path: '/api/console/applications'
              },
              full_profile: { kind: 'row', scope: 'scope_all' },
              allowed_row_scopes: allRowScopeOptions,
              authorization: {
                kind: 'resource_action',
                resource_code: 'application',
                action_code: 'read'
              }
            },
            {
              operation_id: 'applications.publish',
              summary: 'Publish application',
              description: null,
              order: 2,
              route: {
                method: 'POST',
                path: '/api/console/applications/:id/publish'
              },
              full_profile: { kind: 'simple', enabled: true },
              allowed_row_scopes: [],
              authorization: { kind: 'simple' }
            }
          ]
        },
        {
          kind: 'other',
          group_id: 'other.general',
          label: '其他',
          description: '其他已注册操作',
          operations: [
            {
              operation_id: 'audit.export',
              summary: 'Export audit records',
              description: null,
              order: 1,
              route: { method: 'GET', path: '/api/console/test' },
              full_profile: { kind: 'simple', enabled: true },
              allowed_row_scopes: [],
              authorization: { kind: 'simple' }
            }
          ]
        }
      ],
      resources: [
        {
          resource_code: 'application',
          label: '应用',
          description: null,
          actions: [{ action_code: 'read', label: '查询', description: null }]
        }
      ]
    });
    rolesApi.fetchSettingsRoleConsolePolicy.mockResolvedValue({
      role_code: 'member',
      groups: [
        {
          kind: 'settings_feature',
          group_id: 'settings.applications',
          enabled: true,
          strategy: 'full',
          operations: [
            {
              operation_id: 'applications.read',
              kind: 'row',
              scope: 'scope_all'
            },
            {
              operation_id: 'applications.publish',
              kind: 'simple',
              enabled: true
            }
          ]
        },
        {
          kind: 'other',
          group_id: 'other.general',
          enabled: false,
          strategy: 'full',
          operations: [
            {
              operation_id: 'audit.export',
              kind: 'simple',
              enabled: false
            }
          ]
        }
      ]
    });

    const { container } = renderPanel();

    const backendTab = await screen.findByRole('tab', { name: '后台设置' });
    expect(backendTab).toHaveAttribute('aria-selected', 'true');
    expect(screen.getAllByRole('tab').map((tab) => tab.textContent)).toEqual([
      '动态路由',
      '表-通用配置',
      '表-单独配置',
      '后台设置',
      '其他'
    ]);

    expect(
      screen.getByRole('columnheader', { name: '后台设置' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('columnheader', { name: '开放权限' })
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '授权策略' })
    ).toBeInTheDocument();
    expect(screen.getByText('应用管理')).toBeInTheDocument();
    expect(
      screen
        .getByRole('combobox', { name: '应用管理 授权策略' })
        .closest('.ant-select')
    ).toHaveTextContent('全部操作');
    expect(screen.queryByText('applications.read')).not.toBeInTheDocument();
    expect(screen.queryByText('application')).not.toBeInTheDocument();
    expect(screen.queryByText(/\{.*operation_id/)).not.toBeInTheDocument();
    expect(container.innerHTML).not.toContain('settings.applications');
    expect(container.innerHTML).not.toContain('applications.read');
    expect(container.innerHTML).not.toContain('application');

    await selectPolicyCombobox(
      document.body,
      '应用管理 授权策略',
      '按操作配置'
    );
    const applicationsDrawer = await screen.findByRole('dialog');
    expect(
      within(applicationsDrawer).getByText('GET /api/console/applications')
    ).toBeInTheDocument();
    expect(
      within(applicationsDrawer).getByText(
        'POST /api/console/applications/:id/publish'
      )
    ).toBeInTheDocument();
    fireEvent.click(
      within(applicationsDrawer).getByRole('button', { name: '关闭' })
    );

    fireEvent.click(screen.getByRole('tab', { name: '其他' }));
    expect(
      screen
        .getByRole('combobox', { name: '其他 授权策略' })
        .closest('.ant-select')
    ).toHaveTextContent('全部操作');
    expect(screen.getByRole('switch', { name: '其他 启用' })).not.toBeChecked();
    expect(screen.queryByText('other.general')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '详细配置 其他' }));
    const otherDrawer = await screen.findByRole('dialog');
    expect(
      within(otherDrawer).getByText('Export audit records')
    ).toBeInTheDocument();

    expect(
      screen.queryByRole('tab', { name: '基础通用' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: '系统管理' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: 'Agent 应用' })
    ).not.toBeInTheDocument();
  });

  test('AC-003 selects Other when the catalog has no SettingsFeature group', async () => {
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue(
      consolePolicyCatalog([
        {
          kind: 'other',
          group_id: 'other.general',
          label: '其他',
          description: '其他已注册操作',
          operations: []
        }
      ])
    );

    renderPanel();

    expect(await screen.findByRole('tab', { name: '其他' })).toHaveAttribute(
      'aria-selected',
      'true'
    );
    expect(screen.getByText('其他已注册操作')).toBeInTheDocument();
  });

  test('AC-004 toggles group mode and saves simple/resource scopes from the detail drawer', async () => {
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue({
      schema_version: '2026-07-14',
      locale: 'zh_Hans',
      group_strategy_options: policyModeOptions,
      groups: [
        {
          kind: 'settings_feature',
          group_id: 'settings.applications',
          label: '应用管理',
          description: null,
          operations: [
            {
              operation_id: 'applications.read',
              summary: 'List applications',
              description: null,
              order: 1,
              route: { method: 'GET', path: '/api/console/test' },
              full_profile: { kind: 'row', scope: 'scope_all' },
              allowed_row_scopes: allRowScopeOptions,
              authorization: {
                kind: 'resource_action',
                resource_code: 'application',
                action_code: 'read'
              }
            },
            {
              operation_id: 'applications.publish',
              summary: 'Publish application',
              description: null,
              order: 2,
              route: { method: 'GET', path: '/api/console/test' },
              full_profile: { kind: 'simple', enabled: true },
              allowed_row_scopes: [],
              authorization: { kind: 'simple' }
            }
          ]
        }
      ],
      resources: [
        {
          resource_code: 'application',
          label: '应用',
          description: null,
          actions: [{ action_code: 'read', label: '查询', description: null }]
        }
      ]
    });
    rolesApi.fetchSettingsRoleConsolePolicy.mockResolvedValue({
      role_code: 'member',
      groups: [
        {
          kind: 'settings_feature',
          group_id: 'settings.applications',
          enabled: true,
          strategy: 'custom',
          operations: [
            {
              operation_id: 'applications.read',
              kind: 'row',
              scope: 'own'
            },
            {
              operation_id: 'applications.publish',
              kind: 'simple',
              enabled: false
            }
          ]
        }
      ]
    });

    renderPanel();

    const policySelect = await screen.findByRole('combobox', {
      name: '应用管理 授权策略'
    });
    expect(policySelect.closest('.ant-select')).toHaveTextContent('按操作配置');

    fireEvent.click(screen.getByRole('button', { name: '详细配置 应用管理' }));
    const drawer = await screen.findByRole('dialog');
    expect(within(drawer).getByText('List applications')).toBeInTheDocument();
    expect(
      within(drawer).getByRole('switch', { name: 'Publish application' })
    ).not.toBeChecked();

    fireEvent.mouseDown(
      within(drawer).getByRole('combobox', { name: 'List applications 作用域' })
    );
    fireEvent.click((await screen.findAllByTitle('当前空间')).at(-1)!);
    fireEvent.click(
      within(drawer).getByRole('switch', { name: 'Publish application' })
    );
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
              group_id: 'settings.applications',
              enabled: true,
              strategy: 'custom',
              operations: [
                {
                  operation_id: 'applications.read',
                  kind: 'row',
                  scope: 'scope_all'
                },
                {
                  operation_id: 'applications.publish',
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

    fireEvent.click(screen.getByRole('switch', { name: '应用管理 启用' }));
    await waitFor(() => {
      expect(rolesApi.replaceSettingsRoleConsolePolicy).toHaveBeenCalledWith(
        'member',
        {
          groups: [
            expect.objectContaining({
              group_id: 'settings.applications',
              enabled: false
            })
          ]
        },
        'csrf-123'
      );
    });
  });

  test('batch selection changes only selected simple operations after save', async () => {
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue(
      consolePolicyCatalog([
        {
          kind: 'settings_feature',
          group_id: 'settings.batch',
          label: '批量操作',
          description: null,
          operations: [
            {
              operation_id: 'batch.read',
              summary: 'Read records',
              description: null,
              order: 1,
              route: { method: 'GET', path: '/api/console/test' },
              full_profile: { kind: 'row', scope: 'scope_all' },
              allowed_row_scopes: allRowScopeOptions,
              authorization: {
                kind: 'resource_action',
                resource_code: 'record',
                action_code: 'read'
              }
            },
            {
              operation_id: 'batch.create',
              summary: 'Create record',
              description: null,
              order: 2,
              route: { method: 'POST', path: '/api/console/test' },
              full_profile: { kind: 'simple', enabled: true },
              allowed_row_scopes: [],
              authorization: { kind: 'simple' }
            },
            {
              operation_id: 'batch.delete',
              summary: 'Delete record',
              description: null,
              order: 3,
              route: { method: 'DELETE', path: '/api/console/other' },
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
          group_id: 'settings.batch',
          enabled: true,
          strategy: 'custom',
          operations: [
            { operation_id: 'batch.read', kind: 'row', scope: 'own' },
            { operation_id: 'batch.create', kind: 'simple', enabled: false },
            { operation_id: 'batch.delete', kind: 'simple', enabled: false }
          ]
        }
      ]
    });

    renderPanel();
    fireEvent.click(
      await screen.findByRole('button', { name: '详细配置 批量操作' })
    );
    const drawer = await screen.findByRole('dialog');
    const resizeHandle = within(drawer).getByRole('separator', {
      name: '调整权限详情抽屉宽度'
    });
    const initialWidth = Number(resizeHandle.getAttribute('aria-valuenow'));
    fireEvent.keyDown(resizeHandle, { key: 'ArrowLeft' });
    expect(resizeHandle).toHaveAttribute(
      'aria-valuenow',
      String(initialWidth + 40)
    );
    const enableSelected = within(drawer).getByRole('button', {
      name: '启用所选'
    });
    const disableSelected = within(drawer).getByRole('button', {
      name: '关闭所选'
    });
    expect(enableSelected).toBeDisabled();
    expect(
      within(drawer).queryByRole('checkbox', { name: '选择 Read records' })
    ).not.toBeInTheDocument();
    expect(
      within(drawer).getByRole('checkbox', { name: '全选开关项' }).closest('th')
    ).not.toBeNull();

    fireEvent.click(
      within(drawer).getByRole('checkbox', { name: '选择 Create record' })
    );
    fireEvent.click(enableSelected);
    expect(
      within(drawer).getByRole('switch', { name: 'Create record' })
    ).toBeChecked();
    expect(
      within(drawer).getByRole('switch', { name: 'Delete record' })
    ).not.toBeChecked();
    expect(rolesApi.replaceSettingsRoleConsolePolicy).not.toHaveBeenCalled();

    fireEvent.click(
      within(drawer).getByRole('checkbox', { name: '全选开关项' })
    );
    expect(
      within(drawer).getByRole('checkbox', { name: '选择 Delete record' })
    ).toBeChecked();
    fireEvent.click(enableSelected);
    expect(
      within(drawer).getByRole('switch', { name: 'Delete record' })
    ).toBeChecked();
    fireEvent.click(disableSelected);
    expect(
      within(drawer).getByRole('switch', { name: 'Create record' })
    ).not.toBeChecked();
    expect(
      within(drawer).getByRole('switch', { name: 'Delete record' })
    ).not.toBeChecked();

    fireEvent.click(
      within(drawer).getByRole('checkbox', { name: '全选开关项' })
    );
    expect(enableSelected).toBeDisabled();
    expect(disableSelected).toBeDisabled();

    const pathSearch = within(drawer).getByRole('textbox', {
      name: '搜索路径'
    });
    fireEvent.change(pathSearch, { target: { value: 'Create record' } });
    expect(within(drawer).queryByText('Create record')).not.toBeInTheDocument();
    fireEvent.change(pathSearch, { target: { value: 'OTHER' } });
    expect(within(drawer).queryByText('Create record')).not.toBeInTheDocument();
    expect(within(drawer).getByText('Delete record')).toBeInTheDocument();
    fireEvent.click(
      within(drawer).getByRole('checkbox', { name: '全选开关项' })
    );
    fireEvent.click(enableSelected);
    fireEvent.change(pathSearch, { target: { value: '' } });
    expect(enableSelected).toBeDisabled();
    expect(
      within(drawer).getByRole('switch', { name: 'Create record' })
    ).not.toBeChecked();
    expect(
      within(drawer).getByRole('switch', { name: 'Delete record' })
    ).toBeChecked();
    fireEvent.click(
      within(drawer).getByRole('checkbox', { name: '选择 Delete record' })
    );
    fireEvent.click(disableSelected);
    expect(
      within(drawer).getByRole('switch', { name: 'Delete record' })
    ).not.toBeChecked();

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
              group_id: 'settings.batch',
              enabled: true,
              strategy: 'custom',
              operations: [
                { operation_id: 'batch.read', kind: 'row', scope: 'own' },
                {
                  operation_id: 'batch.create',
                  kind: 'simple',
                  enabled: false
                },
                { operation_id: 'batch.delete', kind: 'simple', enabled: false }
              ]
            }
          ]
        },
        'csrf-123'
      );
    });

    fireEvent.click(screen.getByRole('button', { name: '详细配置 批量操作' }));
    expect(
      within(await screen.findByRole('dialog')).getByRole('button', {
        name: '启用所选'
      })
    ).toBeDisabled();
  });
});
