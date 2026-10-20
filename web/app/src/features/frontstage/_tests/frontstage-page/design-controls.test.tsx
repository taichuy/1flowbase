import { describe, test, expect, vi } from 'vitest';
import {
  FrontStagePageHarness,
  activateDesignMode,
  authenticate,
  createBackendPage,
  createCatalogEntry,
  createPageContent,
  exitDesignMode,
  hoverAddMenuAndFlush,
  mockFrontstageBlockCatalog,
  mockPageContentSaveState,
  renderPage
} from './design-controls.fixtures';
import { AppProviders } from '../../../../app/AppProviders';
import { type SaveFrontstageTabDocumentInput } from '../../api/page-content';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';

describe('FrontStagePage - design controls', () => {
  test('shows page context and design mode is unavailable without permission', () => {
    authenticate([]);
    renderPage('page-1');

    expect(screen.getAllByText('页面 page-1').length).toBeGreaterThan(0);
    expect(
      screen.queryByRole('button', { name: '进入设计模式' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '创建区块' })
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('heading', { name: '页面 page-1' })
    ).toBeInTheDocument();
  });

  test('shows design controls from shared design mode state', async () => {
    authenticate(['frontstage.page.design']);
    renderPage('page-1');

    expect(
      screen.queryByRole('button', { name: '创建区块' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '代码区块试运行' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('页面树已同步')).not.toBeInTheDocument();

    activateDesignMode();
    expect(
      screen.getByRole('button', { name: '创建区块' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '代码区块试运行' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('页面树已同步')).not.toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '添加菜单' })
    ).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: '添加菜单' })).toHaveLength(1);
    expect(
      screen.queryByRole('menuitem', { name: '新增分组' })
    ).not.toBeInTheDocument();
    await hoverAddMenuAndFlush();
    expect(
      await screen.findByRole('menuitem', { name: '新增分组' })
    ).toBeInTheDocument();
    expect(
      await screen.findByRole('menuitem', { name: '新增页面' })
    ).toBeInTheDocument();
    fireEvent.mouseLeave(screen.getByRole('button', { name: '添加菜单' }));
    expect(
      screen.queryByRole('menuitem', { name: '新增分组' })
    ).not.toBeInTheDocument();
    exitDesignMode();
    expect(
      screen.queryByRole('button', { name: '创建区块' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '代码区块试运行' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('页面树已同步')).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '添加菜单' })
    ).not.toBeInTheDocument();
  });

  test('AC-001 renders only Ant Empty when no page is bound', () => {
    authenticate(['frontstage.page.design']);
    renderPage();

    activateDesignMode();

    const workspace = screen.getByTestId('frontstage-page-workspace');
    expect(workspace.querySelector('.ant-empty')).toBeInTheDocument();
    expect(
      workspace.querySelector('.ant-empty-description')
    ).not.toBeInTheDocument();
    expect(
      workspace.querySelector('.frontstage-page-workspace__header')
    ).not.toBeInTheDocument();
    expect(workspace.querySelector('.ant-divider')).not.toBeInTheDocument();
    expect(
      within(workspace).queryByRole('button', { name: '创建区块' })
    ).not.toBeInTheDocument();
    expect(
      within(workspace).queryByText('未选择页面内容')
    ).not.toBeInTheDocument();
    expect(
      within(workspace).queryByText('选择页面后将显示页面预览。')
    ).not.toBeInTheDocument();
  });

  test('AC-002 keeps Add menu outside the scrollable tree when the tree is empty', () => {
    authenticate(['frontstage.page.design']);
    renderPage();

    activateDesignMode();

    const sidebar = document.querySelector('.frontstage-page-tree-sidebar');
    expect(sidebar).toBeInTheDocument();
    expect(sidebar?.querySelector('.ant-empty')).not.toBeInTheDocument();

    const tree = sidebar?.querySelector('.frontstage-page-tree-sidebar__tree');
    expect(tree).toBeInTheDocument();
    expect(tree?.children).toHaveLength(0);
    expect(
      within(tree as HTMLElement).queryByRole('button', { name: '添加菜单' })
    ).not.toBeInTheDocument();
    expect(
      within(sidebar as HTMLElement).getByRole('button', { name: '添加菜单' })
    ).toHaveClass(
      'frontstage-add-action-button',
      'frontstage-add-action-button--full'
    );
  });

  test('AC-003 keeps Add menu fixed outside existing top-level nodes', () => {
    authenticate(['frontstage.page.design']);
    renderPage('page-1');

    activateDesignMode();

    const tree = document.querySelector('.frontstage-page-tree-sidebar__tree');
    const sidebar = document.querySelector('.frontstage-page-tree-sidebar');
    expect(tree).toBeInTheDocument();
    expect(tree?.children).toHaveLength(1);
    expect(
      within(tree as HTMLElement).queryByRole('button', { name: '添加菜单' })
    ).not.toBeInTheDocument();
    expect(
      within(sidebar as HTMLElement).getByRole('button', {
        name: '添加菜单'
      })
    ).toBeInTheDocument();
  });

  test('AC-001/009 opens page settings and persists the selected layout mode', async () => {
    authenticate(['frontstage.page.design']);
    const saveState = mockPageContentSaveState();
    render(
      <AppProviders>
        <FrontStagePageHarness
          pageId="page-1"
          tabId="tab-1"
          initialPageTree={[createBackendPage('page-1')]}
          pageContent={createPageContent()}
        />
      </AppProviders>
    );

    activateDesignMode();

    const workspace = screen.getByTestId('frontstage-page-workspace');
    expect(workspace).toHaveAttribute('data-design-selected', 'true');
    const configurePage = within(workspace).getByRole('button', {
      name: '配置页面'
    });
    expect(configurePage).toBeInTheDocument();
    expect(
      screen.getByRole('heading', { name: '页面 page-1' }).parentElement
    ).toHaveClass('frontstage-page-workspace__header');

    fireEvent.click(configurePage);

    const pageMenu = await screen.findByRole('menu');
    expect(within(pageMenu).getAllByRole('menuitem')).toHaveLength(4);
    expect(within(pageMenu).getByText('编辑')).toBeInTheDocument();
    expect(within(pageMenu).getByText('刷新')).toBeInTheDocument();
    const layoutMode = within(pageMenu).getByRole('combobox', {
      name: '布局方式'
    });
    expect(within(pageMenu).getByText('自动布局')).toBeInTheDocument();
    expect(
      within(pageMenu).getByRole('switch', { name: '开启 Tabs' })
    ).not.toBeChecked();

    fireEvent.mouseDown(layoutMode);
    fireEvent.click(await screen.findByText('自由网格'));
    await waitFor(() => expect(saveState.save).toHaveBeenCalledTimes(1));
    const [layoutModeSaveInput] = saveState.save.mock.calls[0] as [
      SaveFrontstageTabDocumentInput
    ];
    expect(layoutModeSaveInput.payload).toMatchObject({
      'x-layout-mode': 'free'
    });

    fireEvent.click(within(pageMenu).getByRole('menuitem', { name: /编辑/ }));
    expect(await screen.findByText('配置页面')).toBeInTheDocument();
    expect(
      screen.queryByRole('combobox', { name: '内容呈现方式' })
    ).not.toBeInTheDocument();
  });

  test('#1975 AC-003/004 keeps current content while refresh is pending or fails', async () => {
    authenticate(['frontstage.page.design']);
    mockPageContentSaveState();
    let rejectRefresh: (error: Error) => void = () => undefined;
    const onRefreshPage = vi.fn(
      () =>
        new Promise<void>((_resolve, reject) => {
          rejectRefresh = reject;
        })
    );
    render(
      <AppProviders>
        <FrontStagePageHarness
          pageId="page-1"
          tabId="tab-1"
          initialPageTree={[createBackendPage('page-1')]}
          pageContent={createPageContent()}
          onRefreshPage={onRefreshPage}
        />
      </AppProviders>
    );

    activateDesignMode();
    const configurePage = screen.getByRole('button', {
      name: '配置页面'
    });
    fireEvent.click(configurePage);
    fireEvent.click(
      await screen.findByRole('menuitem', {
        name: /刷新/
      })
    );

    expect(await screen.findByText('正在刷新当前页面……')).toBeInTheDocument();
    expect(
      screen.getByRole('heading', { name: '页面 page-1' })
    ).toBeInTheDocument();

    fireEvent.click(configurePage);
    expect(
      await screen.findByRole('menuitem', {
        name: /刷新/
      })
    ).toHaveAttribute('aria-disabled', 'true');

    await act(async () => {
      rejectRefresh(new Error('refresh unavailable'));
    });

    expect(await screen.findByText('刷新当前页面失败')).toBeInTheDocument();
    expect(screen.getByText('refresh unavailable')).toBeInTheDocument();
    expect(
      screen.getByRole('heading', { name: '页面 page-1' })
    ).toBeInTheDocument();
  });

  test('#1975 AC-004 ignores a stale refresh failure after the page scope changes', async () => {
    authenticate(['frontstage.page.design']);
    mockPageContentSaveState();
    let rejectRefresh: (error: Error) => void = () => undefined;
    const onRefreshPage = vi.fn(
      () =>
        new Promise<void>((_resolve, reject) => {
          rejectRefresh = reject;
        })
    );
    const pages = [createBackendPage('page-1'), createBackendPage('page-2')];
    const view = render(
      <AppProviders>
        <FrontStagePageHarness
          pageId="page-1"
          tabId="tab-1"
          initialPageTree={pages}
          pageContent={createPageContent({ page: { id: 'page-1' } })}
          onRefreshPage={onRefreshPage}
        />
      </AppProviders>
    );

    activateDesignMode();
    fireEvent.click(screen.getByRole('button', { name: '配置页面' }));
    fireEvent.click(await screen.findByRole('menuitem', { name: /刷新/ }));
    expect(await screen.findByText('正在刷新当前页面……')).toBeInTheDocument();

    view.rerender(
      <AppProviders>
        <FrontStagePageHarness
          pageId="page-2"
          tabId="tab-2"
          initialPageTree={pages}
          pageContent={createPageContent({
            page: { id: 'page-2' },
            tab: { id: 'tab-2', pageId: 'page-2' }
          })}
          onRefreshPage={vi.fn(() => Promise.resolve())}
        />
      </AppProviders>
    );
    expect(
      await screen.findByRole('heading', { name: '页面 page-2' })
    ).toBeInTheDocument();

    await act(async () => {
      rejectRefresh(new Error('stale refresh failure'));
    });

    await waitFor(() => {
      expect(screen.queryByText('刷新当前页面失败')).not.toBeInTheDocument();
      expect(
        screen.queryByText('stale refresh failure')
      ).not.toBeInTheDocument();
    });
  });

  test('#1300 keeps the canvas and Add Block action inside the active tab container', async () => {
    authenticate(['frontstage.page.design']);
    render(
      <AppProviders>
        <FrontStagePageHarness
          pageId="page-1"
          tabId="tab-1"
          onNavigateTab={vi.fn()}
          initialPageTree={[createBackendPage('page-1')]}
          pageContent={createPageContent({
            page: { contentPresentation: 'tabs' }
          })}
        />
      </AppProviders>
    );

    activateDesignMode();

    const tabContent = await screen.findByTestId('frontstage-tab-content');
    expect(tabContent).toHaveAttribute('data-design-selected', 'true');
    expect(
      within(tabContent).getByRole('button', { name: '创建区块' })
    ).toBeInTheDocument();
    expect(
      within(tabContent).getByTestId('page-canvas-design-empty-state')
    ).toBeEmptyDOMElement();
  });

  test('opens configuration and code in the same resizable JSX Studio', async () => {
    authenticate(['frontstage.page.design']);
    mockFrontstageBlockCatalog([createCatalogEntry()]);
    const blockPayload = {
      id: 'orders-block',
      renderer_version: 'v1',
      codeRef: 'orders-code',
      catalog: {
        providerCode: '1flowbase',
        installationId: 'builtin-installation'
      },
      contribution: {
        pluginId: 'builtin-frontstage',
        pluginVersion: '1.0.0',
        code: 'frontstage.js-ui-block'
      },
      props: { title: 'Orders' },
      'x-layout': { order: 0, region: 'main' },
      runtime: { kind: 'native_react', entry: 'index.js', hint: 'native_react' }
    };

    render(
      <AppProviders>
        <FrontStagePageHarness
          pageId="page-1"
          tabId="tab-1"
          onNavigateTab={vi.fn()}
          initialPageTree={[createBackendPage('page-1')]}
          pageContent={createPageContent({
            schema: {
              rootUid: 'root-1',
              payload: { blocks: [blockPayload] }
            },
            root: {
              uid: 'root-1',
              payload: { blocks: [blockPayload] }
            }
          })}
        />
      </AppProviders>
    );

    activateDesignMode();
    const blockSlot = await screen.findByTestId('block-slot-orders-block');
    fireEvent.mouseEnter(blockSlot);
    fireEvent.click(
      within(blockSlot).getByRole('button', { name: '编辑区块' })
    );

    const studio = await screen.findByRole('dialog', { name: 'TSX 编辑器' });
    const configurationButton = within(studio).getByRole('button', {
      name: '区块设置'
    });
    fireEvent.click(configurationButton);
    expect(configurationButton).toHaveClass(
      'frontstage-jsx-studio__rail-button--active'
    );
    expect(
      screen.queryByRole('dialog', { name: '区块配置' })
    ).not.toBeInTheDocument();
  });
});
