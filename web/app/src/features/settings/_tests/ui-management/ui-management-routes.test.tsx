import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { useQuery } from '@tanstack/react-query';
import { beforeAll, beforeEach, expect, test, vi } from 'vitest';
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

const api = vi.hoisted(() => ({ list: vi.fn(), move: vi.fn() }));
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
  { id: 'a', title: 'Page A', kind: 'page', placement: 'topbar', children: [] },
  { id: 'b', title: 'Page B', kind: 'page', placement: 'topbar', children: [] }
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
  vi.restoreAllMocks();
  vi.clearAllMocks();
  api.list.mockResolvedValue(nodes);
  api.move.mockResolvedValue(nodes[0]);
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
  expect(row('b')).toHaveAttribute('draggable', 'false');
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
