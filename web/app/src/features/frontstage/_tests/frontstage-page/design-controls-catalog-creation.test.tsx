import { describe, test, expect } from 'vitest';
import {
  FrontStagePageHarness,
  SLOW_FRONTSTAGE_TEST_TIMEOUT,
  activateDesignMode,
  authenticate,
  blockTreeApi,
  createBackendPage,
  createCatalogEntry,
  createPageContent,
  exitDesignMode,
  mockFrontstageBlockCatalog,
  mockPageContentSaveState,
  pageContentSaveHook
} from './design-controls.fixtures';
import { AppProviders } from '../../../../app/AppProviders';
import { FrontStagePage } from '../../pages/FrontStagePage';
import { type NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';

describe('FrontStagePage - design controls', () => {
  test('shows real page tree operation states without local draft wording', () => {
    authenticate(['frontstage.page.design']);
    const view = render(
      <AppProviders>
        <FrontStagePage workspaceId="workspace-1" initialPageTree={[]} />
      </AppProviders>
    );

    activateDesignMode();
    expect(screen.queryByText('页面树已同步')).not.toBeInTheDocument();
    expect(screen.queryByText(/本地草稿/)).not.toBeInTheDocument();

    view.rerender(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          initialPageTree={[]}
          isPageTreeMutating
        />
      </AppProviders>
    );
    expect(screen.getByText('保存中')).toBeInTheDocument();

    view.rerender(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          initialPageTree={[]}
          pageTreeMutationError={new Error('failed')}
        />
      </AppProviders>
    );
    expect(screen.getByText('failed')).toBeInTheDocument();
  });

  test('keeps mutation status scoped to design mode controls', () => {
    authenticate(['frontstage.page.design']);
    render(
      <AppProviders>
        <FrontStagePage
          workspaceId="workspace-1"
          initialPageTree={[]}
          isPageTreeMutating
        />
      </AppProviders>
    );

    activateDesignMode();
    expect(screen.getByText('保存中')).toBeInTheDocument();

    exitDesignMode();
    expect(screen.queryByText('保存中')).not.toBeInTheDocument();
    activateDesignMode();

    expect(screen.getByText('保存中')).toBeInTheDocument();
  });

  test('R8-AC-001/005 directly creates the sole Catalog entry atomically', async () => {
    authenticate(['frontstage.page.design']);
    const selectedTemplate = `export default function ReportBlock() {
  return <p>Selected report template</p>;
}`;
    mockFrontstageBlockCatalog([
      createCatalogEntry({
        id: 'third-party:report-block',
        installationId: 'third-party-installation',
        providerCode: 'third-party',
        pluginId: 'third-party-reports',
        pluginVersion: '3.1.0',
        contributionCode: 'report-block',
        title: 'Third-party report',
        codeCapabilities: {
          template: {
            source: selectedTemplate,
            version: '3.1.0',
            language: 'tsx'
          }
        }
      })
    ]);
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
    fireEvent.click(screen.getByRole('button', { name: '创建区块' }));

    await waitFor(() =>
      expect(blockTreeApi.createFrontstageBlockNode).toHaveBeenCalledOnce()
    );
    expect(
      screen.queryByRole('dialog', { name: '新增区块' })
    ).not.toBeInTheDocument();

    expect(
      pageContentSaveHook.useFrontstagePageContentSave
    ).toHaveBeenLastCalledWith({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1'
    });
    expect(blockTreeApi.createFrontstageBlockNode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      expect.objectContaining({
        tab_id: 'tab-1',
        parent_block_id: null,
        source_code: selectedTemplate,
        runtime_descriptor: expect.objectContaining({
          catalog: {
            providerCode: 'third-party',
            installationId: 'third-party-installation'
          },
          contribution: {
            pluginId: 'third-party-reports',
            pluginVersion: '3.1.0',
            code: 'report-block'
          },
          props: {},
          'x-layout': expect.objectContaining({ order: 0 }),
          runtime: expect.objectContaining({
            kind: 'native_react',
            entry: 'index.js'
          })
        })
      }),
      'csrf-123'
    );
  });

  test('AC-011 rejects a catalog entry without a code template before saving', async () => {
    authenticate(['frontstage.page.design']);
    mockFrontstageBlockCatalog([
      createCatalogEntry({ codeCapabilities: undefined })
    ]);
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
    fireEvent.click(screen.getByRole('button', { name: '创建区块' }));

    expect(
      await screen.findByText('Catalog entry 缺少代码模板')
    ).toBeInTheDocument();
    expect(saveState.save).not.toHaveBeenCalled();
    expect(blockTreeApi.createFrontstageBlockNode).not.toHaveBeenCalled();
  });

  test('R8-AC-004 shows Catalog permission failures without a fallback', async () => {
    authenticate(['frontstage.page.design']);
    mockFrontstageBlockCatalog([], {
      error: new Error('Catalog forbidden')
    });

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
    fireEvent.click(screen.getByRole('button', { name: '创建区块' }));

    expect(await screen.findByText('Catalog forbidden')).toBeInTheDocument();
    expect(blockTreeApi.createFrontstageBlockNode).not.toHaveBeenCalled();
  });

  test.each([
    {
      label: 'no entry',
      entries: [] as NormalizedFrontstageBlockCatalogEntry[],
      message: '当前没有可用区块目录项，暂时无法新增区块。'
    },
    {
      label: 'multiple entries',
      entries: [createCatalogEntry(), createCatalogEntry({ id: 'second' })],
      message: '区块目录加载失败'
    }
  ])(
    'R8-AC-004 rejects $label without choosing a fallback',
    async ({ entries, message }) => {
      authenticate(['frontstage.page.design']);
      mockFrontstageBlockCatalog(entries);

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
      fireEvent.click(screen.getByRole('button', { name: '创建区块' }));

      expect(await screen.findByText(message)).toBeInTheDocument();
      expect(blockTreeApi.createFrontstageBlockNode).not.toHaveBeenCalled();
      expect(
        screen.queryByRole('dialog', { name: '新增区块' })
      ).not.toBeInTheDocument();
    }
  );

  test('disables Add Block while page content is saving', () => {
    authenticate(['frontstage.page.design']);
    mockPageContentSaveState({ saving: true, isPending: true });

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

    expect(screen.getByRole('button', { name: '创建区块' })).toBeDisabled();
    expect(screen.getByText('区块保存中')).toBeInTheDocument();
  });

  test('R8-AC-003 disables direct creation while the Catalog is loading', () => {
    authenticate(['frontstage.page.design']);
    mockFrontstageBlockCatalog([createCatalogEntry()], { loading: true });

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
    expect(screen.getByRole('button', { name: '创建区块' })).toBeDisabled();
  });

  test('R8-AC-003 prevents duplicate direct creation while the first request is pending', async () => {
    authenticate(['frontstage.page.design']);
    mockFrontstageBlockCatalog([createCatalogEntry()]);
    let completeCreation: (() => void) | undefined;
    blockTreeApi.createFrontstageBlockNode.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          completeCreation = () =>
            resolve({
              block_id: 'created-root-block',
              tab_id: 'tab-1',
              parent_block_id: null
            });
        })
    );
    mockPageContentSaveState();

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
    const createButton = screen.getByRole('button', { name: '创建区块' });
    fireEvent.click(createButton);
    fireEvent.click(createButton);

    await waitFor(() =>
      expect(blockTreeApi.createFrontstageBlockNode).toHaveBeenCalledOnce()
    );
    expect(createButton).toBeDisabled();
    await act(async () => completeCreation?.());
  });

  test('shows a clear Add Block save error in design mode', async () => {
    authenticate(['frontstage.page.design']);
    mockFrontstageBlockCatalog([createCatalogEntry()]);
    mockPageContentSaveState();
    blockTreeApi.createFrontstageBlockNode.mockRejectedValueOnce(
      new Error('request failed')
    );

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
    fireEvent.click(screen.getByRole('button', { name: '创建区块' }));

    expect(await screen.findByText('区块保存失败')).toBeInTheDocument();
    expect(screen.getByText('request failed')).toBeInTheDocument();
  });

  test('keeps the previous document when atomic block creation fails', async () => {
    authenticate(['frontstage.page.design']);
    mockFrontstageBlockCatalog([createCatalogEntry()]);
    const saveState = mockPageContentSaveState();
    blockTreeApi.createFrontstageBlockNode.mockRejectedValueOnce(
      new Error('create failed')
    );

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
    fireEvent.click(screen.getByRole('button', { name: '创建区块' }));

    expect(await screen.findByText('区块保存失败')).toBeInTheDocument();
    expect(screen.getByText('create failed')).toBeInTheDocument();
    await waitFor(() =>
      expect(blockTreeApi.createFrontstageBlockNode).toHaveBeenCalledTimes(1)
    );
    expect(saveState.save).not.toHaveBeenCalled();
    expect(screen.queryByText('1 个区块')).not.toBeInTheDocument();
  });

  test('hides Add Block without a page and disables it without page content', () => {
    authenticate(['frontstage.page.design']);
    const view = render(
      <AppProviders>
        <FrontStagePageHarness />
      </AppProviders>
    );

    activateDesignMode();
    expect(
      screen.queryByRole('button', { name: '创建区块' })
    ).not.toBeInTheDocument();

    view.unmount();
    render(
      <AppProviders>
        <FrontStagePageHarness
          pageId="page-1"
          initialPageTree={[createBackendPage('page-1')]}
        />
      </AppProviders>
    );

    activateDesignMode();
    expect(screen.getByRole('button', { name: '创建区块' })).toBeDisabled();
  });

  test(
    'creates the root through Block Tree without writing the Page document',
    async () => {
      authenticate(['frontstage.page.design']);
      mockFrontstageBlockCatalog([createCatalogEntry()]);
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
      fireEvent.click(screen.getByRole('button', { name: '创建区块' }));

      await waitFor(() =>
        expect(blockTreeApi.createFrontstageBlockNode).toHaveBeenCalledTimes(1)
      );
      expect(saveState.save).not.toHaveBeenCalled();
      expect(
        screen.queryByRole('dialog', { name: '区块代码' })
      ).not.toBeInTheDocument();
    },
    SLOW_FRONTSTAGE_TEST_TIMEOUT
  );
});
