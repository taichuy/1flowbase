import '../../../../test/fixtures/rc-util-unique-ids';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { useQuery } from '@tanstack/react-query';
import { afterEach, beforeAll, beforeEach, expect, test, vi } from 'vitest';
import { AppProviders } from '../../../../app/AppProviders';
import { useAuthStore } from '../../../../state/auth-store';
import {
  appI18n,
  loadApplicationI18nResources
} from '../../../../shared/i18n/app-i18n';
import { UiManagementPanel } from '../../components/ui-management/UiManagementPanel';
import { RouteManagementTab } from '../../components/ui-management/RouteManagementTab';
import {
  fetchFrontstagePageTree,
  frontstagePageTreeQueryKey
} from '../../../frontstage/api/page-tree';

const api = vi.hoisted(() => ({
  list: vi.fn(),
  move: vi.fn(),
  create: vi.fn(),
  update: vi.fn(),
  remove: vi.fn(),
  listTabs: vi.fn(),
  createTab: vi.fn(),
  updateTab: vi.fn(),
  deleteTab: vi.fn()
}));
vi.mock('@1flowbase/api-client', async (original) => ({
  ...(await original<typeof import('@1flowbase/api-client')>()),
  createFrontstageGroup: api.create,
  createFrontstagePage: api.create,
  updateFrontstagePageNodeTitle: api.update,
  deleteFrontstagePageNode: api.remove,
  listFrontstagePageTabs: api.listTabs,
  createFrontstagePageTab: api.createTab,
  updateFrontstagePageTab: api.updateTab,
  deleteFrontstagePageTab: api.deleteTab
}));
const router = vi.hoisted(() => ({
  navigate: vi.fn(),
  pathname: '/settings/ui-management/routes'
}));
vi.mock('@tanstack/react-router', async (original) => ({
  ...(await original<typeof import('@tanstack/react-router')>()),
  useNavigate: () => router.navigate,
  useRouterState: ({ select }: { select: (state: unknown) => unknown }) =>
    select({ location: { pathname: router.pathname } })
}));
vi.mock('../../../frontstage/api/page-tree', async (original) => ({
  ...(await original<typeof import('../../../frontstage/api/page-tree')>()),
  fetchFrontstagePageTree: api.list,
  moveFrontstageNode: api.move
}));
vi.mock('../../components/ui-management/CodeTemplatesTab', () => ({
  CodeTemplatesTab: () => null
}));
vi.mock('../../components/ui-management/ComponentRecordsTab', () => ({
  ComponentRecordsTab: () => null
}));
const nodes = [
  {
    id: 'a',
    title: 'Page A',
    kind: 'page',
    placement: 'topbar',
    children: [],
    content_presentation: 'single',
    slug: 'page-alpha',
    is_hidden: false
  },
  {
    id: 'b',
    title: 'Page B',
    kind: 'page',
    placement: 'topbar',
    children: [],
    content_presentation: 'single',
    slug: 'page-alpha',
    is_hidden: false
  }
];
const row = (id: string) =>
  document.querySelector<HTMLElement>(`[data-node-id="${id}"]`)!;
async function dragAfter() {
  await screen.findByText('Page A');
  const dataTransfer = { setData: vi.fn(), effectAllowed: '', dropEffect: '' };
  fireEvent.dragStart(row('a'), { dataTransfer });
  vi.spyOn(row('b'), 'getBoundingClientRect').mockReturnValue({
    top: 100,
    height: 100
  } as DOMRect);
  const event = new Event('dragover', { bubbles: true, cancelable: true });
  Object.defineProperties(event, {
    clientY: { value: 190 },
    dataTransfer: { value: dataTransfer }
  });
  fireEvent(row('b'), event);
  fireEvent.drop(row('b'), { dataTransfer });
}
function NavigationConsumer() {
  const tree = useQuery({
    queryKey: frontstagePageTreeQueryKey('workspace'),
    queryFn: () => fetchFrontstagePageTree('workspace')
  });
  return (
    <output data-testid="navigation-order">
      {tree.data?.map((node) => node.id).join(',')}
    </output>
  );
}
beforeAll(async () => {
  await loadApplicationI18nResources();
  await appI18n.changeLanguage('zh_Hans');
});
beforeEach(() => {
  vi.clearAllMocks();
  api.list.mockResolvedValue(nodes);
  api.move.mockResolvedValue(nodes[0]);
  api.create.mockResolvedValue(nodes[0]);
  api.update.mockResolvedValue(nodes[0]);
  api.remove.mockResolvedValue(undefined);
  api.listTabs.mockResolvedValue([
    {
      id: 'tab-a',
      page_id: 'a',
      title: 'Overview',
      rank: '001000',
      is_default: true,
      route_segment: null,
      document_root_uid: 'root-a'
    }
  ]);
  api.createTab.mockResolvedValue({ id: 'tab-b' });
  api.updateTab.mockResolvedValue({ id: 'tab-a' });
  api.deleteTab.mockResolvedValue(undefined);
  router.pathname = '/settings/ui-management/routes';
  useAuthStore.setState({
    csrfToken: 'csrf',
    actor: {
      id: 'root',
      account: 'root',
      current_workspace_id: 'workspace',
      effective_display_role: 'root'
    } as NonNullable<ReturnType<typeof useAuthStore.getState>['actor']>
  });
});

test('route tab is selected on direct navigation and tab links preserve existing destinations', async () => {
  render(
    <AppProviders>
      <UiManagementPanel canManage />
    </AppProviders>
  );
  expect(screen.getByRole('tab', { name: '路由管理' })).toHaveAttribute(
    'aria-selected',
    'true'
  );
  await screen.findByText('Page A');
  fireEvent.click(screen.getByRole('tab', { name: '组件管理' }));
  expect(router.navigate).toHaveBeenLastCalledWith({
    to: '/settings/ui-management/components'
  });
});

test('entering route management from another tab navigates to its dedicated URL', () => {
  router.pathname = '/settings/ui-management/components';
  render(
    <AppProviders>
      <UiManagementPanel canManage />
    </AppProviders>
  );
  fireEvent.click(screen.getByRole('tab', { name: '路由管理' }));
  expect(router.navigate).toHaveBeenCalledWith({
    to: '/settings/ui-management/routes'
  });
});

test('saving disables further drags and refreshes the shared navigation tree from the backend', async () => {
  let resolve!: () => void;
  api.move.mockImplementation(
    () =>
      new Promise<void>((done) => {
        resolve = done;
      })
  );
  render(
    <AppProviders>
      <NavigationConsumer />
      <RouteManagementTab canManage />
    </AppProviders>
  );
  await dragAfter();
  await waitFor(() =>
    expect(api.move).toHaveBeenCalledWith(
      'workspace',
      'a',
      { parentId: null, after_id: 'b' },
      'csrf'
    )
  );
  await waitFor(() => expect(row('b')).toHaveAttribute('draggable', 'false'));
  api.list.mockResolvedValue([nodes[1], nodes[0]]);
  await act(async () => resolve());
  await waitFor(() =>
    expect(screen.getByTestId('navigation-order')).toHaveTextContent('b,a')
  );
  expect(row('b')).toHaveAttribute('draggable', 'true');
});

test('failed save preserves original order and permits retry', async () => {
  api.move.mockRejectedValueOnce(new Error('rejected'));
  render(
    <AppProviders>
      <NavigationConsumer />
      <RouteManagementTab canManage />
    </AppProviders>
  );
  await dragAfter();
  await screen.findByText('移动失败，请重试。');
  expect(screen.getByTestId('navigation-order')).toHaveTextContent('a,b');
  expect(row('a')).toHaveAttribute('draggable', 'true');
  await dragAfter();
  await waitFor(() => expect(api.move).toHaveBeenCalledTimes(2));
  await waitFor(() =>
    expect(screen.queryByText('移动失败，请重试。')).not.toBeInTheDocument()
  );
});

test('UI access without page design permission leaves the tree read only', async () => {
  useAuthStore.setState({
    actor: {
      ...useAuthStore.getState().actor!,
      effective_display_role: 'viewer'
    },
    me: null
  });
  render(
    <AppProviders>
      <RouteManagementTab canManage />
    </AppProviders>
  );
  await screen.findByText('Page A');
  expect(row('a')).toHaveAttribute('draggable', 'false');
  expect(
    screen.getByText('当前仅可查看，移动需要页面设计权限。')
  ).toBeInTheDocument();
  expect(api.move).not.toHaveBeenCalled();
});

test('failed loading offers retry and empty navigation has an explicit empty state', async () => {
  api.list.mockRejectedValueOnce(new Error('offline')).mockResolvedValue([]);
  render(
    <AppProviders>
      <RouteManagementTab canManage />
    </AppProviders>
  );
  await screen.findByText('导航加载失败，请重试。');
  fireEvent.click(screen.getByRole('button', { name: /重\s*试/ }));
  await screen.findByText('暂无页面或分组');
});

test('node settings save actual DTO fields and retain edits on a rejected request', async () => {
  api.update.mockRejectedValueOnce(new Error('rejected'));
  render(
    <AppProviders>
      <RouteManagementTab canManage />
    </AppProviders>
  );
  fireEvent.click(await screen.findByRole('button', { name: '编辑 Page A' }));
  const dialog = await screen.findByRole('dialog', { name: '菜单配置' });
  const name = within(dialog).getByLabelText('名称');
  fireEvent.change(name, { target: { value: 'Renamed page' } });
  fireEvent.click(within(dialog).getByLabelText('在导航中隐藏'));
  fireEvent.click(within(dialog).getByRole('button', { name: /保\s*存/ }));
  await within(dialog).findByText('操作失败，请重试。');
  expect(name).toHaveValue('Renamed page');
  expect(api.update).toHaveBeenCalledWith(
    'a',
    expect.objectContaining({
      title: 'Renamed page',
      is_hidden: true,
      slug: 'page-alpha',
      content_presentation: 'single'
    }),
    'csrf',
    expect.any(String)
  );
  fireEvent.click(within(dialog).getByRole('button', { name: /保\s*存/ }));
  await waitFor(() =>
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  );
  expect(api.update).toHaveBeenCalledTimes(2);
});

test('root adds a real topbar group without offering root edit or delete', async () => {
  render(
    <AppProviders>
      <RouteManagementTab canManage />
    </AppProviders>
  );
  await screen.findByText('Page A');
  const root = row('__frontstage_root__');
  expect(
    within(root).queryByRole('button', { name: /编辑|删除/ })
  ).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: /添加菜单/ }));
  fireEvent.click(await screen.findByRole('menuitem', { name: '添加分组' }));
  const dialog = await screen.findByRole('dialog', { name: '添加分组' });
  fireEvent.change(within(dialog).getByLabelText('名称'), {
    target: { value: 'New group' }
  });
  fireEvent.change(within(dialog).getByLabelText('访问路径'), {
    target: { value: 'new-group' }
  });
  fireEvent.click(within(dialog).getByRole('button', { name: /保\s*存/ }));
  await waitFor(() =>
    expect(api.create).toHaveBeenCalledWith(
      expect.objectContaining({
        title: 'New group',
        parent_id: null,
        placement: 'topbar',
        slug: 'new-group'
      }),
      'csrf',
      expect.any(String)
    )
  );
});

test('delete is explicit, cancellable, and calls the existing endpoint only after confirmation', async () => {
  render(
    <AppProviders>
      <RouteManagementTab canManage />
    </AppProviders>
  );
  fireEvent.click(await screen.findByRole('button', { name: '删除 Page A' }));
  expect(api.remove).not.toHaveBeenCalled();
  const dialog = await screen.findByRole('dialog');
  expect(
    within(dialog).getByText('页面及其所有标签页和内容将被删除，无法撤销。')
  ).toBeInTheDocument();
  fireEvent.click(within(dialog).getByRole('button', { name: /删\s*除/ }));
  await waitFor(() =>
    expect(api.remove).toHaveBeenCalledWith('a', 'csrf', expect.any(String))
  );
});

test('page tabs load lazily and protect the last tab while allowing creation with the real route_segment field', async () => {
  api.list.mockResolvedValue([
    { ...nodes[0], content_presentation: 'tabs' },
    nodes[1]
  ]);
  render(
    <AppProviders>
      <RouteManagementTab canManage />
    </AppProviders>
  );
  fireEvent.click(await screen.findByRole('button', { name: '编辑 Page A' }));
  expect(api.listTabs).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('tab', { name: '页面 Tabs' }));
  await screen.findByText('Overview');
  expect(screen.getByRole('button', { name: '删除 Overview' })).toBeDisabled();
  fireEvent.click(screen.getByRole('button', { name: /新增标签页/ }));
  const dialog = await screen.findByRole('dialog', { name: /新增标签页/ });
  fireEvent.change(within(dialog).getByLabelText('名称'), {
    target: { value: 'Reports' }
  });
  fireEvent.change(within(dialog).getByLabelText('标签页路径'), {
    target: { value: 'reports' }
  });
  fireEvent.click(within(dialog).getByRole('button', { name: /确\s*定/ }));
  await waitFor(() =>
    expect(api.createTab).toHaveBeenCalledWith(
      'a',
      { title: 'Reports', route_segment: 'reports', rank: '002000' },
      'csrf',
      expect.any(String)
    )
  );
});

test('search locates a descendant without filtering out or reordering other nodes', async () => {
  api.list.mockResolvedValue([
    {
      id: 'g',
      title: 'Group',
      kind: 'group',
      placement: 'topbar',
      children: [
        {
          id: 'nested',
          title: 'Nested',
          kind: 'group',
          children: [{ ...nodes[0], title: 'Deep page' }]
        }
      ]
    },
    nodes[1]
  ]);
  render(
    <AppProviders>
      <RouteManagementTab canManage />
    </AppProviders>
  );
  await screen.findByText('Group');
  expect(screen.queryByText('Deep page')).not.toBeInTheDocument();
  fireEvent.change(screen.getByRole('combobox'), { target: { value: 'Deep' } });
  fireEvent.click(await screen.findByText('Group / Nested / Deep page'));
  await waitFor(() => expect(row('a')).toHaveAttribute('data-located', 'true'));
  expect(screen.getByText('Page B')).toBeInTheDocument();
  expect(api.move).not.toHaveBeenCalled();
});

test('tab rename and reorder share the existing tab contract; delete refreshes the tab list', async () => {
  const first = {
    id: 'tab-a',
    page_id: 'a',
    title: 'Overview',
    rank: '001000',
    is_default: true,
    route_segment: null,
    document_root_uid: 'root-a'
  };
  const second = {
    ...first,
    id: 'tab-b',
    title: 'Reports',
    rank: '002000',
    is_default: false,
    route_segment: 'reports'
  };
  api.listTabs.mockResolvedValue([first, second]);
  render(
    <AppProviders>
      <RouteManagementTab canManage />
    </AppProviders>
  );
  fireEvent.click(await screen.findByRole('button', { name: '编辑 Page A' }));
  fireEvent.click(screen.getByRole('tab', { name: '页面 Tabs' }));
  fireEvent.click(
    await screen.findByRole('button', { name: '重命名 Reports' })
  );
  const rename = await screen.findByRole('dialog', { name: '重命名' });
  fireEvent.change(within(rename).getByLabelText('名称'), {
    target: { value: 'Insights' }
  });
  fireEvent.click(within(rename).getByRole('button', { name: /确\s*定/ }));
  await waitFor(() =>
    expect(api.updateTab).toHaveBeenCalledWith(
      'a',
      'tab-b',
      { title: 'Insights' },
      'csrf',
      expect.any(String)
    )
  );
  await waitFor(() =>
    expect(
      screen.queryByRole('dialog', { name: '重命名' })
    ).not.toBeInTheDocument()
  );
  fireEvent.click(screen.getByRole('button', { name: '上移 Reports' }));
  await waitFor(() =>
    expect(api.updateTab).toHaveBeenCalledWith(
      'a',
      'tab-b',
      { rank: '001000' },
      'csrf',
      expect.any(String)
    )
  );
  expect(api.updateTab).toHaveBeenCalledWith(
    'a',
    'tab-a',
    { rank: '002000' },
    'csrf',
    expect.any(String)
  );
  await waitFor(() =>
    expect(screen.getByRole('button', { name: '删除 Reports' })).toBeEnabled()
  );
  fireEvent.click(screen.getByRole('button', { name: '删除 Reports' }));
  await screen.findByText('删除此标签页？');
  api.listTabs.mockResolvedValue([first]);
  fireEvent.click(screen.getByRole('button', { name: /确\s*定/ }));
  await waitFor(() =>
    expect(api.deleteTab).toHaveBeenCalledWith(
      'a',
      'tab-b',
      'csrf',
      expect.any(String)
    )
  );
  await waitFor(() =>
    expect(screen.queryByText('Reports')).not.toBeInTheDocument()
  );
});

afterEach(() => vi.restoreAllMocks());

test('single-page mode explains why a new tab cannot be created until the mode is saved', async () => {
  render(
    <AppProviders>
      <RouteManagementTab canManage />
    </AppProviders>
  );
  fireEvent.click(await screen.findByRole('button', { name: '编辑 Page A' }));
  fireEvent.click(screen.getByRole('tab', { name: '页面 Tabs' }));
  expect(
    await screen.findByRole('button', { name: /新增标签页/ })
  ).toBeDisabled();
  expect(
    screen.getByText(
      '当前为单页模式。请在基本信息中将内容展示切换为页面 Tabs 并保存，再新增标签页。'
    )
  ).toBeInTheDocument();
  expect(api.createTab).not.toHaveBeenCalled();
});

test('a partially rejected tab reorder stays locked until all writes settle, then refetches authoritative order', async () => {
  const first = {
    id: 'tab-a',
    page_id: 'a',
    title: 'Overview',
    rank: '001000',
    is_default: true,
    route_segment: null,
    document_root_uid: 'root-a'
  };
  api.listTabs.mockResolvedValue([
    first,
    {
      ...first,
      id: 'tab-b',
      title: 'Reports',
      rank: '002000',
      is_default: false
    }
  ]);
  let finish!: () => void;
  api.updateTab
    .mockRejectedValueOnce(new Error('rejected'))
    .mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        })
    );
  render(
    <AppProviders>
      <RouteManagementTab canManage />
    </AppProviders>
  );
  fireEvent.click(await screen.findByRole('button', { name: '编辑 Page A' }));
  fireEvent.click(screen.getByRole('tab', { name: '页面 Tabs' }));
  fireEvent.click(await screen.findByRole('button', { name: '上移 Reports' }));
  await waitFor(() => expect(api.updateTab).toHaveBeenCalledTimes(2));
  await waitFor(() =>
    expect(screen.getByRole('button', { name: '上移 Reports' })).toBeDisabled()
  );
  expect(api.listTabs).toHaveBeenCalledTimes(1);
  await act(async () => finish());
  await screen.findByText('操作失败，请重试。');
  expect(api.listTabs).toHaveBeenCalledTimes(2);
  expect(screen.getByRole('button', { name: '上移 Reports' })).toBeEnabled();
});

test.each(['page', 'group'])(
  'visibility button toggles a %s using server state',
  async (kind) => {
    let current = { ...nodes[0], kind };
    api.list.mockImplementation(async () => [current]);
    let finish: (() => void) | undefined;
    api.update.mockImplementation(async (_id, input) => {
      await new Promise<void>((resolve) => {
        finish = resolve;
      });
      current = { ...current, ...input };
      return current;
    });
    render(
      <AppProviders>
        <RouteManagementTab canManage />
      </AppProviders>
    );
    const hide = await screen.findByRole('button', {
      name: '隐藏此节点 Page A'
    });
    fireEvent.click(hide);
    fireEvent.click(hide);
    await waitFor(() => expect(api.update).toHaveBeenCalledTimes(1));
    expect(api.update).toHaveBeenLastCalledWith(
      'a',
      { is_hidden: true },
      'csrf',
      expect.any(String)
    );
    expect(hide).toBeDisabled();
    await act(async () => {
      finish?.();
    });
    fireEvent.click(
      await screen.findByRole('button', { name: '显示此节点 Page A' })
    );
    await waitFor(() => expect(api.update).toHaveBeenCalledTimes(2));
    expect(api.update).toHaveBeenLastCalledWith(
      'a',
      { is_hidden: false },
      'csrf',
      expect.any(String)
    );
    await act(async () => {
      finish?.();
    });
    expect(
      await screen.findByRole('button', { name: '隐藏此节点 Page A' })
    ).toBeEnabled();
  }
);

test('failed visibility update retains server state and reports failure', async () => {
  api.update.mockRejectedValue(new Error('failed'));
  render(
    <AppProviders>
      <RouteManagementTab canManage />
    </AppProviders>
  );
  fireEvent.click(
    await screen.findByRole('button', { name: '隐藏此节点 Page A' })
  );
  expect(await screen.findByText('操作失败，请重试。')).toBeInTheDocument();
  expect(
    screen.getByRole('button', { name: '隐藏此节点 Page A' })
  ).toBeEnabled();
});

test('readonly route visibility is an indicator without a mutation action', async () => {
  render(
    <AppProviders>
      <RouteManagementTab canManage={false} />
    </AppProviders>
  );
  await screen.findByText('Page A');
  expect(
    within(row('a')).getByRole('img', { name: '此节点设置为显示' })
  ).toBeInTheDocument();
  expect(
    screen.queryByRole('button', { name: '隐藏此节点 Page A' })
  ).not.toBeInTheDocument();
  expect(api.update).not.toHaveBeenCalled();
});

test('open links use the topbar route and nested page route in a new tab, including readonly', async () => {
  api.list.mockResolvedValue([
    {
      ...nodes[0],
      kind: 'group',
      slug: 'portal',
      children: [
        {
          ...nodes[1],
          kind: 'group',
          placement: 'sidebar',
          slug: null,
          children: [
            {
              ...nodes[1],
              id: 'nested',
              title: 'Nested',
              placement: 'sidebar',
              slug: null
            }
          ]
        }
      ]
    },
    {
      ...nodes[1],
      id: 'legacy',
      title: 'Legacy',
      placement: 'sidebar',
      slug: null
    }
  ]);
  render(
    <AppProviders>
      <RouteManagementTab canManage={false} />
    </AppProviders>
  );
  const rootLink = await screen.findByRole('link', {
    name: '在新标签页打开 Page A'
  });
  expect(rootLink).toHaveAttribute('href', '/portal');
  expect(rootLink).toHaveAttribute('target', '_blank');
  expect(rootLink).toHaveAttribute('rel', 'noopener noreferrer');
  fireEvent.click(
    row('b').closest('.ant-tree-treenode')!.querySelector('.ant-tree-switcher')!
  );
  expect(
    await screen.findByRole('link', { name: '在新标签页打开 Nested' })
  ).toHaveAttribute('href', '/portal/pages/nested');
  expect(
    screen.queryByRole('link', { name: '在新标签页打开 Page B' })
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole('link', { name: '在新标签页打开 Legacy' })
  ).not.toBeInTheDocument();
  expect(api.update).not.toHaveBeenCalled();
});
