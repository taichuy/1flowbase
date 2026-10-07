import {
  act,
  renderHook,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';
const fetchDepartments = vi.hoisted(() => vi.fn());
vi.mock('../../../api/departments', () => ({
  fetchSettingsDepartments: fetchDepartments,
  settingsDepartmentsQueryKey: ['settings', 'departments']
}));
import { AppProviders } from '../../../../../app/AppProviders';
import { useDepartmentTree } from '../useDepartmentTree';
import { OrganizationSelector } from '../../../components/organization/OrganizationSelector';
const department = (
  id: string,
  name: string,
  parent_id: string | null,
  has_children = false,
  is_match = true
) => ({
  id,
  name,
  parent_id,
  has_children,
  is_match,
  member_count: 3,
  role_codes: []
});
function Harness() {
  const tree = useDepartmentTree();
  return (
    <OrganizationSelector
      departments={tree.items}
      selected="child"
      onSelect={vi.fn()}
      loading={tree.loading}
      error={tree.error}
      onRetry={tree.reload}
      search={tree.prefix}
      onSearch={tree.setPrefix}
      onExpand={tree.loadChildren}
      groups={tree.groups}
    />
  );
}
describe('organization lazy tree', () => {
  beforeEach(() => {
    fetchDepartments.mockReset();
    fetchDepartments.mockImplementation(
      async ({ parent_id, cursor, prefix }) => {
        if (prefix)
          return {
            items: [
              department('root', 'Engineering', null, true, false),
              department('match', 'Development', 'root')
            ],
            has_more: !cursor,
            next_cursor: cursor ? null : 'search-next'
          };
        if (parent_id)
          return {
            items: [
              department(
                cursor ? 'child2' : 'child',
                cursor ? 'Design' : 'Development',
                'root'
              )
            ],
            has_more: !cursor,
            next_cursor: cursor ? null : 'child-next'
          };
        return {
          items: [
            department(
              cursor ? 'root2' : 'root',
              cursor ? 'Support' : 'Engineering',
              null,
              !cursor
            )
          ],
          has_more: !cursor,
          next_cursor: cursor ? null : 'root-next'
        };
      }
    );
  });
  test('keeps spaces while typing a multiword department and normalizes only the request', async () => {
    render(
      <AppProviders>
        <Harness />
      </AppProviders>
    );
    const search = screen.getByRole('textbox', { name: '搜索组织' });
    fireEvent.change(search, { target: { value: 'Marketing ' } });
    expect(search).toHaveValue('Marketing ');
    fireEvent.change(search, { target: { value: 'Marketing Team ' } });
    expect(search).toHaveValue('Marketing Team ');
    await waitFor(() =>
      expect(fetchDepartments).toHaveBeenCalledWith({
        prefix: 'Marketing Team',
        limit: 50
      })
    );
  });
  test('preserves a match repeated as ancestor context and discards loaded cursors on reload', async () => {
    fetchDepartments.mockImplementation(async ({ cursor }) => ({
      items: cursor
        ? [
            department('match', 'Marketing', null, true, false),
            department('child', 'Marketing Team', 'match')
          ]
        : [department('match', 'Marketing', null, true)],
      has_more: !cursor,
      next_cursor: cursor ? null : 'next'
    }));
    const { result } = renderHook(() => useDepartmentTree(), {
      wrapper: AppProviders
    });
    act(() => result.current.setPrefix('Marketing '));
    await waitFor(() => expect(result.current.groups).toHaveLength(1));
    act(() => result.current.groups[0].loadMore());
    await waitFor(() => expect(result.current.items).toHaveLength(2));
    expect(
      result.current.items.find((item) => item.id === 'match')?.is_match
    ).toBe(true);
    act(() => result.current.reload());
    await waitFor(() => expect(result.current.items).toHaveLength(1));
    expect(result.current.prefix).toBe('Marketing ');
    expect(result.current.groups).toHaveLength(1);
    expect(fetchDepartments).toHaveBeenLastCalledWith({
      prefix: 'Marketing',
      limit: 50
    });
  });
  test('loads one root page, lazy children, then independent sibling pages', async () => {
    render(
      <AppProviders>
        <Harness />
      </AppProviders>
    );
    const root = await screen.findByText('Engineering');
    expect(screen.queryByText('Development')).not.toBeInTheDocument();
    expect(fetchDepartments).toHaveBeenCalledTimes(1);
    fireEvent.click(
      root.closest('.ant-tree-treenode')!.querySelector('.ant-tree-switcher')!
    );
    expect(await screen.findByText('Development')).toBeInTheDocument();
    expect(fetchDepartments).toHaveBeenCalledWith({
      parent_id: 'root',
      limit: 50
    });
    const buttons = screen.getAllByRole('button', { name: '加载更多' });
    fireEvent.click(buttons[0]);
    expect(await screen.findByText('Design')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '加载更多' }));
    expect(await screen.findByText('Support')).toBeInTheDocument();
    expect(within(screen.getByRole('tree')).getAllByText('组织')).toHaveLength(
      1
    );
  });
  test('searches the server, merges repeated context, and reloads after a stale cursor', async () => {
    render(
      <AppProviders>
        <Harness />
      </AppProviders>
    );
    await screen.findByText('Engineering');
    fireEvent.change(screen.getByRole('textbox', { name: '搜索组织' }), {
      target: { value: 'Dev' }
    });
    await waitFor(() =>
      expect(fetchDepartments).toHaveBeenCalledWith({
        prefix: 'Dev',
        limit: 50
      })
    );
    expect(await screen.findByText('Development')).toBeInTheDocument();
    fetchDepartments.mockRejectedValueOnce(new Error('stale anchor'));
    fireEvent.click(screen.getByRole('button', { name: '加载更多' }));
    expect(await screen.findByText('组织加载失败')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '重试' }));
    await waitFor(() =>
      expect(screen.queryByText('组织加载失败')).not.toBeInTheDocument()
    );
    expect(screen.getAllByText('Engineering')).toHaveLength(1);
  });
});
