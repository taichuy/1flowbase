import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';

const api = vi.hoisted(() => ({
  settingsFileStoragesQueryKey: ['settings', 'files', 'storages'],
  settingsFileTablesQueryKey: ['settings', 'files', 'tables'],
  fetchSettingsFileStorages: vi.fn(), fetchSettingsFileTables: vi.fn(),
  createSettingsFileStorage: vi.fn(), updateSettingsFileStorage: vi.fn(), deleteSettingsFileStorage: vi.fn(),
  createSettingsFileTable: vi.fn(), updateSettingsFileTableBinding: vi.fn(), deleteSettingsFileTable: vi.fn()
}));
vi.mock('../../../api/file-management', () => api);
import { AppProviders } from '../../../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../../../state/auth-store';
import { SettingsFilesSection } from '../SettingsFilesSection';

const storage = { id: 'storage-1', code: 'shared', title: 'Shared storage', driver_type: 'local', enabled: true, is_default: false, config_json: { root_path: '/files' }, rule_json: {}, health_status: 'ready' };
const table = { id: 'table-1', code: 'uploads', title: 'Uploads', scope_kind: 'workspace', scope_id: 'workspace-1', bound_storage_id: storage.id, bound_storage_title: storage.title, is_builtin: false, status: 'active', record_permissions: { can_create: false, can_update: false, can_delete: false, can_share: false } };
function authenticate(permissions: string[], role = 'member') {
  useAuthStore.setState({ csrfToken: 'csrf',
    actor: { id: 'user-1', account: 'member', effective_display_role: role, current_workspace_id: 'workspace-1' },
    me: { id: 'user-1', account: 'member', email: '', phone: null, nickname: '', name: '', avatar_url: null, introduction: '', effective_display_role: role, permissions }
  });
}
function show(permissions: string[], role = 'member') {
  authenticate(permissions, role);
  return render(<AppProviders><SettingsFilesSection isRoot={role === 'root'} permissions={permissions} /></AppProviders>);
}
beforeEach(() => {
  resetAuthStore();
  vi.clearAllMocks();
  api.fetchSettingsFileStorages.mockResolvedValue([storage]);
  api.fetchSettingsFileTables.mockResolvedValue([table]);
  api.updateSettingsFileTableBinding.mockResolvedValue({});
  api.deleteSettingsFileTable.mockResolvedValue(undefined);
  api.createSettingsFileTable.mockResolvedValue({});
});

test('read operations show storage and table data while storage writes remain root-only', async () => {
  show(['file_storages.list', 'file_tables.list', 'file_storages.create', 'file_storages.update', 'file_storages.delete']);
  const row = await screen.findByRole('row', { name: /shared.*Shared storage/ });
  expect(screen.getByRole('tab', { name: '存储配置' })).toBeInTheDocument();
  expect(screen.getByRole('tab', { name: '文件表' })).toBeInTheDocument();
  expect(screen.getByRole('button', { name: /新增/ })).toBeDisabled();
  expect(within(row).getByRole('button', { name: /编辑/ })).toBeDisabled();
  expect(within(row).getByRole('button', { name: /删除/ })).toBeDisabled();
  fireEvent.click(within(row).getByRole('button', { name: /查看/ }));
  expect(await screen.findByDisplayValue('/files')).toBeDisabled();
  expect(api.createSettingsFileStorage).not.toHaveBeenCalled();
  expect(api.updateSettingsFileStorage).not.toHaveBeenCalled();
  expect(api.deleteSettingsFileStorage).not.toHaveBeenCalled();
});

test('a table binding grant permits only binding configuration', async () => {
  show(['file_storages.list', 'file_tables.list', 'file_tables.storage.bind']);
  fireEvent.click(screen.getByRole('tab', { name: '文件表' }));
  const row = await screen.findByRole('row', { name: /uploads.*Uploads/ });
  expect(within(row).queryByRole('button', { name: /删除/ })).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: /新增/ })).not.toBeInTheDocument();
  fireEvent.click(within(row).getByRole('button', { name: /编辑/ }));
  const dialog = await screen.findByRole('dialog');
  expect(within(dialog).getByLabelText('名称')).toBeDisabled();
  expect(within(dialog).getByLabelText('绑定存储')).toBeEnabled();
  fireEvent.click(within(dialog).getByRole('button', { name: /保存绑定/ }));
  await waitFor(() => expect(api.updateSettingsFileTableBinding).toHaveBeenCalledWith('table-1', { bound_storage_id: 'storage-1' }, 'csrf'));
  expect(api.createSettingsFileTable).not.toHaveBeenCalled();
  expect(api.deleteSettingsFileTable).not.toHaveBeenCalled();
});

test('table deletion does not enable create, binding or storage writes', async () => {
  show(['file_tables.list', 'file_tables.delete']);
  const row = await screen.findByRole('row', { name: /uploads.*Uploads/ });
  expect(screen.queryByRole('tab', { name: '存储配置' })).not.toBeInTheDocument();
  expect(within(row).queryByRole('button', { name: /编辑/ })).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: /新增/ })).not.toBeInTheDocument();
  fireEvent.click(within(row).getByRole('button', { name: /删除/ }));
  const confirm = await screen.findByRole('dialog');
  fireEvent.click(within(confirm).getByRole('button', { name: /删\s*除/ }));
  await waitFor(() => expect(api.deleteSettingsFileTable).toHaveBeenCalledWith('table-1', 'csrf'));
  expect(api.updateSettingsFileTableBinding).not.toHaveBeenCalled();
});

test('create-only operation leaves lists closed and permits table creation', async () => {
  show(['file_tables.create']);
  expect(screen.queryByRole('tab')).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: /新增/ }));
  fireEvent.change(await screen.findByLabelText('表标识'), { target: { value: 'new_uploads' } });
  fireEvent.change(screen.getByLabelText('名称'), { target: { value: 'New uploads' } });
  fireEvent.click(screen.getByRole('button', { name: /创\s*建/ }));
  await waitFor(() => expect(api.createSettingsFileTable).toHaveBeenCalledWith({ code: 'new_uploads', title: 'New uploads', bound_storage_id: null }, 'csrf'));
  expect(api.fetchSettingsFileStorages).not.toHaveBeenCalled();
  expect(api.fetchSettingsFileTables).not.toHaveBeenCalled();
});
