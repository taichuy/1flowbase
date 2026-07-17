import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { AppProviders } from '../../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import { FrontstagePageTabs } from '../../components/FrontstagePageTabs';

const pageTabsApi = vi.hoisted(() => ({
  createFrontstagePageTab: vi.fn(),
  deleteFrontstagePageTab: vi.fn(),
  fetchFrontstagePageTabs: vi.fn(),
  moveFrontstagePageTab: vi.fn(),
  renameFrontstagePageTab: vi.fn(),
  frontstagePageTabsQueryKey: vi.fn((workspaceId: string, pageId: string) => [
    'frontstage',
    workspaceId,
    'pages',
    pageId,
    'tabs'
  ])
}));

vi.mock('../../api/page-tabs', () => pageTabsApi);

function authenticate() {
  resetAuthStore();
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'actor-1',
      account: 'developer',
      effective_display_role: 'developer',
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'user-1',
      account: 'developer',
      email: 'developer@example.com',
      phone: null,
      nickname: 'Developer',
      name: 'Developer',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'developer',
      permissions: ['frontstage.page.design']
    }
  });
}

function renderTabs(tabId = 'tab-1') {
  const onNavigateTab = vi.fn();
  render(
    <AppProviders>
      <FrontstagePageTabs
        workspaceId="workspace-1"
        pageId="page-1"
        tabId={tabId}
        isDesignMode
        onNavigateTab={onNavigateTab}
      >
        <div data-testid="active-tab-blocks">当前标签页区块</div>
      </FrontstagePageTabs>
    </AppProviders>
  );
  return onNavigateTab;
}

describe('FrontstagePageTabs', () => {
  beforeEach(() => {
    authenticate();
    vi.clearAllMocks();
    pageTabsApi.fetchFrontstagePageTabs.mockResolvedValue([
      { id: 'tab-1', page_id: 'page-1', title: '概览', rank: '001000', is_default: true },
      { id: 'tab-2', page_id: 'page-1', title: '详情', rank: '002000', is_default: false }
    ]);
    pageTabsApi.createFrontstagePageTab.mockResolvedValue({
      id: 'tab-3',
      page_id: 'page-1',
      title: '分析',
      rank: '003000',
      is_default: false
    });
    pageTabsApi.renameFrontstagePageTab.mockResolvedValue({
      id: 'tab-2',
      page_id: 'page-1',
      title: '明细',
      rank: '002000',
      is_default: false
    });
    pageTabsApi.moveFrontstagePageTab.mockResolvedValue({
      id: 'tab-2',
      page_id: 'page-1',
      title: '详情',
      rank: '000000',
      is_default: false
    });
    pageTabsApi.deleteFrontstagePageTab.mockResolvedValue(undefined);
  });

  test('AC-005 restores the selected tab from the URL and navigates on tab change', async () => {
    const utils = renderTabs('tab-2');

    expect(await screen.findByRole('tab', { name: /详情/ })).toHaveAttribute(
      'aria-selected',
      'true'
    );
    fireEvent.click(screen.getByRole('tab', { name: /概览/ }));
    expect(utils).toHaveBeenCalledWith('tab-1');
  });

  test('#1300 makes the active tab own and select its complete content container', async () => {
    renderTabs('tab-2');

    expect(await screen.findByRole('tab', { name: /详情/ })).toHaveAttribute(
      'aria-selected',
      'true'
    );
    const tabContent = screen.getByTestId('frontstage-tab-content');
    expect(tabContent).toHaveAttribute('data-design-selected', 'true');
    expect(
      within(tabContent).getByTestId('active-tab-blocks')
    ).toHaveTextContent('当前标签页区块');
  });

  test('#1300 reorders tabs from the compact drag handle and persists the affected ranks', async () => {
    renderTabs('tab-2');

    const sourceTab = await screen.findByRole('tab', { name: /详情/ });
    const targetTab = screen.getByRole('tab', { name: /概览/ });
    const targetDropZone = within(targetTab).getByTestId(
      'frontstage-tab-label-tab-1'
    );
    const dragHandle = within(sourceTab).getByRole('button', {
      name: '拖拽排序'
    });
    const dataTransfer = {
      effectAllowed: 'move',
      getData: vi.fn(() => 'tab-2'),
      setData: vi.fn()
    };

    fireEvent.dragStart(dragHandle, { dataTransfer });
    fireEvent.dragOver(targetDropZone, { dataTransfer });
    fireEvent.drop(targetDropZone, { dataTransfer });

    await waitFor(() => {
      expect(pageTabsApi.moveFrontstagePageTab).toHaveBeenCalledTimes(2);
    });
    expect(pageTabsApi.moveFrontstagePageTab.mock.calls).toEqual(
      expect.arrayContaining([
        ['workspace-1', 'page-1', 'tab-2', { rank: '001000' }, 'csrf-123'],
        ['workspace-1', 'page-1', 'tab-1', { rank: '002000' }, 'csrf-123']
      ])
    );
  });

  test('shows one tab only in UI mode and disables last-tab deletion in the settings popover', async () => {
    pageTabsApi.fetchFrontstagePageTabs.mockResolvedValueOnce([
      { id: 'tab-1', page_id: 'page-1', title: '概览', rank: '001000', is_default: true }
    ]);
    renderTabs();

    expect(await screen.findByRole('tab', { name: /概览/ })).toBeInTheDocument();
    fireEvent.click(screen.getAllByRole('button', { name: '配置标签页' })[0]);

    const deleteButton = await screen.findByRole('button', {
      name: /删除当前标签页/
    });
    expect(deleteButton).toBeDisabled();
    expect(
      screen.getByText('页面至少保留一个标签页，最后一个标签页不可删除。')
    ).toBeInTheDocument();
  });

  test('hides a single tab in runtime mode', async () => {
    pageTabsApi.fetchFrontstagePageTabs.mockResolvedValueOnce([
      { id: 'tab-1', page_id: 'page-1', title: '概览', rank: '001000', is_default: true }
    ]);
    render(
      <AppProviders>
        <FrontstagePageTabs
          workspaceId="workspace-1"
          pageId="page-1"
          tabId="tab-1"
          isDesignMode={false}
          onNavigateTab={vi.fn()}
        >
          <div data-testid="runtime-tab-content">运行态标签页内容</div>
        </FrontstagePageTabs>
      </AppProviders>
    );

    await waitFor(() => {
      expect(pageTabsApi.fetchFrontstagePageTabs).toHaveBeenCalled();
    });
    expect(screen.queryByRole('tablist')).not.toBeInTheDocument();
    expect(screen.getByTestId('runtime-tab-content')).toHaveTextContent(
      '运行态标签页内容'
    );
  });

  test('#1300 keeps tab editing controls out of runtime mode', async () => {
    render(
      <AppProviders>
        <FrontstagePageTabs
          workspaceId="workspace-1"
          pageId="page-1"
          tabId="tab-1"
          isDesignMode={false}
          onNavigateTab={vi.fn()}
        >
          <div data-testid="runtime-multi-tab-content">运行态多标签页内容</div>
        </FrontstagePageTabs>
      </AppProviders>
    );

    expect(
      await screen.findByRole('tab', { name: '概览' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '拖拽排序' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '配置标签页' })
    ).not.toBeInTheDocument();
    expect(screen.getByTestId('runtime-multi-tab-content')).toBeInTheDocument();
  });

  test('AC-002 wires UI mode tab create, rename, and delete actions through compact settings', async () => {
    const utils = renderTabs('tab-2');

    await screen.findByRole('tab', { name: /详情/ });
    fireEvent.click(screen.getByRole('button', { name: '新建标签页' }));
    await waitFor(() => {
      expect(pageTabsApi.createFrontstagePageTab).toHaveBeenCalledWith(
        'workspace-1',
        'page-1',
        { title: null, rank: '003000' },
        'csrf-123'
      );
    });
    expect(utils).toHaveBeenCalledWith('tab-3');

    // Open the compact settings popover for the active tab (tab-2).
    const configureButtons = screen.getAllByRole('button', {
      name: '配置标签页'
    });
    fireEvent.click(configureButtons[configureButtons.length - 1]);
    expect(
      screen.queryByRole('dialog', { name: '标签页设置' })
    ).not.toBeInTheDocument();

    fireEvent.change(
      await screen.findByRole('textbox', { name: '标签页名称' }),
      { target: { value: '明细' } }
    );
    fireEvent.click(screen.getByRole('button', { name: '重命名当前标签页' }));
    await waitFor(() => {
      expect(pageTabsApi.renameFrontstagePageTab).toHaveBeenCalledWith(
        'workspace-1',
        'page-1',
        'tab-2',
        { title: '明细' },
        'csrf-123'
      );
    });

    // Re-open the popover: rename success closes it.
    const reopenButtons = screen.getAllByRole('button', { name: '配置标签页' });
    fireEvent.click(reopenButtons[reopenButtons.length - 1]);
    fireEvent.click(
      await screen.findByRole('button', { name: /删除当前标签页/ })
    );
    fireEvent.click(await screen.findByRole('button', { name: '确 定' }));
    await waitFor(() => {
      expect(pageTabsApi.deleteFrontstagePageTab).toHaveBeenCalledWith(
        'workspace-1',
        'page-1',
        'tab-2',
        'csrf-123'
      );
    });
    expect(utils).toHaveBeenCalledWith('tab-1');
  });
});
