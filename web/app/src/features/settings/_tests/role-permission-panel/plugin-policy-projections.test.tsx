import { describe, expect, test } from 'vitest';
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import type { SettingsRoleConsolePolicy } from '../../api/identity-access/roles';
import type { SettingsConsolePolicyCatalog } from '../../api/identity-access/permissions';
import { permissionsApi, renderPanel, rolesApi } from './role-permission-panel.fixtures';
import { consolePolicyCatalog, selectPolicyCombobox } from './fixtures';

const calendarGroup: SettingsConsolePolicyCatalog['groups'][number] = {
  kind: 'plugin',
  group_id: 'calendar',
  settings_feature_id: 'settings.calendar',
  label: '日历插件',
  description: '管理日历',
  operations: [{
    operation_id: 'calendar.publish',
    summary: '发布日历',
    description: '',
    order: 1,
    route: { method: 'POST', path: '/api/plugins/calendar/events' },
    full_profile: { kind: 'simple', enabled: true },
    allowed_row_scopes: [],
    authorization: { kind: 'simple' }
  }]
};
const workerGroup: SettingsConsolePolicyCatalog['groups'][number] = {
  kind: 'plugin', group_id: 'worker', settings_feature_id: null,
  label: '任务插件', description: '', operations: []
};

async function activePanel(label: string) {
  fireEvent.click(await screen.findByRole('tab', { name: label }));
  return screen.findByRole('tabpanel', { name: label });
}

function persistPolicy() {
  let policy: SettingsRoleConsolePolicy = { role_code: 'member', groups: [] };
  rolesApi.fetchSettingsRoleConsolePolicy.mockImplementation(async () => policy);
  rolesApi.replaceSettingsRoleConsolePolicy.mockImplementation(async (_role, input) => {
    policy = { ...policy, groups: input.groups };
  });
}

async function expectSaved(group: SettingsRoleConsolePolicy['groups'][number]) {
  await waitFor(() => expect(rolesApi.replaceSettingsRoleConsolePolicy).toHaveBeenLastCalledWith(
    'member', { groups: [group] }, 'csrf-123'
  ));
  await waitFor(() => expect(screen.getByRole('switch', { name: '日历插件 启用' })).toBeEnabled());
}

describe('plugin policy catalog projections', () => {
  test('plugin-only groups select the plugin fallback without creating a settings tab', async () => {
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue(consolePolicyCatalog([workerGroup]));
    renderPanel();
    expect(await screen.findByRole('tab', { name: '插件', selected: true })).toBeInTheDocument();
    expect(screen.queryByRole('tab', { name: '后台设置' })).not.toBeInTheDocument();
    expect(await screen.findByRole('switch', { name: '任务插件 启用' })).not.toBeChecked();
    expect(screen.queryByRole('button', { name: /排序/ })).not.toBeInTheDocument();
  });

  test('settings projects only plugins with a registered settings feature and keeps native features', async () => {
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue(consolePolicyCatalog([
      calendarGroup, workerGroup,
      { kind: 'settings_feature', group_id: 'settings.native', settings_feature_id: null,
        label: '原生设置', description: '', operations: [] },
      { kind: 'other', group_id: 'other.general', settings_feature_id: null,
        label: '其他操作', description: '', operations: [] }
    ]));
    renderPanel();
    const settings = await screen.findByRole('tabpanel', { name: '后台设置' });
    expect(screen.getAllByRole('tab').map((tab) => tab.textContent)).toEqual([
      '动态路由', '表-通用配置', '表-单独配置', '后台设置', '插件', '其他'
    ]);
    expect(within(settings).getAllByRole('button', { name: /排序/ })).toHaveLength(2);
    expect(within(settings).getByRole('columnheader', { name: '后台设置' })).toBeInTheDocument();
    expect(within(settings).getByText('原生设置')).toBeInTheDocument();
    expect(within(settings).getByText('日历插件')).toBeInTheDocument();
    expect(within(settings).queryByText('任务插件')).not.toBeInTheDocument();
    const plugins = await activePanel('插件');
    expect(within(plugins).getByRole('columnheader', { name: '插件' })).toBeInTheDocument();
    expect(within(plugins).getByText('日历插件')).toBeInTheDocument();
    expect(within(plugins).getByText('任务插件')).toBeInTheDocument();
    expect(within(plugins).queryByText('原生设置')).not.toBeInTheDocument();
    expect(within(plugins).queryByRole('button', { name: /排序/ })).not.toBeInTheDocument();
  });

  test.each([
    { source: '后台设置', target: '插件' },
    { source: '插件', target: '后台设置' }
  ])('$source and $target share activation, strategy and operation details under one plugin identity', async ({ source, target }) => {
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue(consolePolicyCatalog([calendarGroup]));
    persistPolicy();
    renderPanel();
    await activePanel(source);
    const fullGroup: SettingsRoleConsolePolicy['groups'][number] = {
      kind: 'plugin', group_id: 'calendar', enabled: true, strategy: 'full', operations: []
    };
    fireEvent.click(await screen.findByRole('switch', { name: '日历插件 启用' }));
    await expectSaved(fullGroup);
    let panel = await activePanel(target);
    expect(within(panel).getByRole('switch', { name: '日历插件 启用' })).toBeChecked();

    await selectPolicyCombobox(panel, '日历插件 授权策略', '按操作配置');
    let drawer = await screen.findByRole('dialog');
    const operation = within(drawer).getByRole('switch', { name: '发布日历' });
    expect(operation).not.toBeChecked();
    fireEvent.click(operation);
    fireEvent.click(within(drawer).getByRole('button', { name: '保存权限配置' }));
    const customGroup: SettingsRoleConsolePolicy['groups'][number] = {
      ...fullGroup, strategy: 'custom',
      operations: [{ operation_id: 'calendar.publish', kind: 'simple', enabled: true }]
    };
    await expectSaved(customGroup);

    panel = await activePanel(source);
    expect(within(panel).getByRole('combobox', { name: '日历插件 授权策略' }).closest('.ant-select')).toHaveTextContent('按操作配置');
    fireEvent.click(within(panel).getByRole('button', { name: '详细配置 日历插件' }));
    drawer = await screen.findByRole('dialog');
    expect(within(drawer).getByRole('switch', { name: '发布日历' })).toBeChecked();
    fireEvent.click(within(drawer).getByRole('switch', { name: '发布日历' }));
    fireEvent.click(within(drawer).getByRole('button', { name: '保存权限配置' }));
    const editedGroup: SettingsRoleConsolePolicy['groups'][number] = {
      ...customGroup, operations: [{ operation_id: 'calendar.publish', kind: 'simple', enabled: false }]
    };
    await expectSaved(editedGroup);
    panel = await activePanel(target);
    fireEvent.click(within(panel).getByRole('button', { name: '详细配置 日历插件' }));
    drawer = await screen.findByRole('dialog');
    expect(within(drawer).getByRole('switch', { name: '发布日历' })).not.toBeChecked();
    fireEvent.click(within(drawer).getByRole('button', { name: '保存权限配置' }));
    await expectSaved(editedGroup);

    // Disabling either projection preserves its custom operations for the other projection.
    fireEvent.click(screen.getByRole('switch', { name: '日历插件 启用' }));
    await expectSaved({ ...editedGroup, enabled: false });
    panel = await activePanel(source);
    expect(within(panel).getByRole('switch', { name: '日历插件 启用' })).not.toBeChecked();
    await selectPolicyCombobox(panel, '日历插件 授权策略', '全部操作');
    await expectSaved({ ...editedGroup, enabled: false, strategy: 'full' });
    panel = await activePanel(target);
    expect(within(panel).getByRole('combobox', { name: '日历插件 授权策略' }).closest('.ant-select')).toHaveTextContent('全部操作');
    for (const [, input] of rolesApi.replaceSettingsRoleConsolePolicy.mock.calls) {
      expect(input.groups).toHaveLength(1);
      expect(input.groups[0]).toMatchObject({ kind: 'plugin', group_id: 'calendar' });
    }
  }, 20000);
});
