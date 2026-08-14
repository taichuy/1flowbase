import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { App } from 'antd';
import type { ComponentProps, ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import type {
  FrontstageBlockNode,
  FrontstageBlockNodeSummary
} from '../../../api/block-tree';
import { WindowWorkspaceWindow } from '../../../../../shared/ui/window-workspace/WindowWorkspaceWindow';
import { BlockSchemaTreePanel } from '../BlockSchemaTreePanel';

const api = vi.hoisted(() => ({
  fetchFrontstageBlockAncestors: vi.fn(),
  fetchFrontstageBlockChildren: vi.fn(),
  fetchFrontstageBlockDeleteImpact: vi.fn(),
  fetchFrontstageBlockNode: vi.fn(),
  fetchFrontstageBlockRoots: vi.fn(),
  searchFrontstageBlocks: vi.fn()
}));

const mutations = vi.hoisted(() => ({
  create: { isPending: false, mutateAsync: vi.fn() },
  update: { isPending: false, mutateAsync: vi.fn() },
  move: { isPending: false, mutateAsync: vi.fn() },
  deleteLeaf: { isPending: false, mutateAsync: vi.fn() },
  deleteSubtree: { isPending: false, mutateAsync: vi.fn() },
  saveCode: { isPending: false, mutateAsync: vi.fn() }
}));

vi.mock('../../../api/block-tree', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../../api/block-tree')>()),
  ...api
}));

vi.mock('../../../hooks/use-frontstage-block-tree-mutations', () => ({
  useFrontstageBlockTreeMutations: () => mutations
}));

function summary(
  block_id: string,
  title: string,
  parent_block_id: string | null,
  presentation: FrontstageBlockNodeSummary['presentation'] = 'page'
): FrontstageBlockNodeSummary {
  return {
    block_id,
    workspace_id: 'workspace-1',
    page_id: 'page-1',
    tab_id: 'tab-1',
    parent_block_id,
    rank: '001000',
    presentation,
    title,
    description: null,
    schema_version: 1,
    created_at: '2026-08-12T00:00:00Z',
    updated_at: '2026-08-12T00:00:00Z'
  };
}

function detail(node: FrontstageBlockNodeSummary): FrontstageBlockNode {
  return {
    ...node,
    input_mapping: {},
    output_mapping: {},
    runtime_descriptor: null
  };
}

const root = summary('root-page', 'Root page', null);
const child = summary('child-page', 'Child page', 'root-page');

function Wrapper({ children }: { children: ReactNode }) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  return (
    <QueryClientProvider client={queryClient}>
      <App>{children}</App>
    </QueryClientProvider>
  );
}

function renderPanel(
  props: Partial<ComponentProps<typeof BlockSchemaTreePanel>> = {}
) {
  const onOpenBlock = vi.fn();
  const onDeletedBlock = vi.fn();
  render(
    <BlockSchemaTreePanel
      workspaceId="workspace-1"
      pageId="page-1"
      currentBlockId="root-page"
      onOpenBlock={onOpenBlock}
      onDeletedBlock={onDeletedBlock}
      {...props}
    />,
    { wrapper: Wrapper }
  );
  return { onDeletedBlock, onOpenBlock };
}

describe('BlockSchemaTreePanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.fetchFrontstageBlockRoots.mockResolvedValue([root]);
    api.fetchFrontstageBlockNode.mockResolvedValue(detail(root));
    api.fetchFrontstageBlockAncestors.mockResolvedValue([]);
    api.fetchFrontstageBlockChildren.mockResolvedValue([child]);
    api.searchFrontstageBlocks.mockResolvedValue([
      { node: child, ancestors: [root] }
    ]);
    api.fetchFrontstageBlockDeleteImpact.mockResolvedValue({
      affected_count: 2
    });
    mutations.create.mutateAsync.mockResolvedValue(detail(child));
    mutations.update.mutateAsync.mockResolvedValue(detail(root));
    mutations.deleteLeaf.mutateAsync.mockResolvedValue(undefined);
    mutations.deleteSubtree.mutateAsync.mockResolvedValue({ deleted_count: 2 });
    mutations.move.mutateAsync.mockResolvedValue(detail(child));
  });

  test('AC-004 keeps search above the lazy tree and selects the current real root', async () => {
    const { onOpenBlock } = renderPanel();

    expect(
      screen.getByRole('searchbox', { name: /搜索区块|Search blocks/u })
    ).toBeInTheDocument();
    expect(await screen.findByText('Root page')).toBeInTheDocument();
    const rootTreeItem = screen
      .getByText('Root page')
      .closest('[role="treeitem"]');
    expect(rootTreeItem).toHaveAttribute('aria-selected', 'true');
    expect(rootTreeItem?.querySelector('.ant-tree-iconEle')).toBeNull();
    expect(screen.queryByText(/根容器|Root container/u)).not.toBeInTheDocument();

    const switcher = rootTreeItem?.querySelector('.ant-tree-switcher');
    expect(switcher).not.toBeNull();
    fireEvent.click(switcher as Element);
    await waitFor(() => {
      expect(api.fetchFrontstageBlockChildren).toHaveBeenCalledWith(
        'workspace-1',
        'page-1',
        'root-page'
      );
    });
    expect(await screen.findByText('Child page')).toBeInTheDocument();

    fireEvent.click(screen.getByText('Child page'));
    expect(onOpenBlock).toHaveBeenCalledWith('child-page');
  });

  test('AC-005 keeps expanded branches when the current block moves to an ancestor and uses compact indentation', async () => {
    api.fetchFrontstageBlockAncestors.mockImplementation(
      async (_workspaceId: string, _pageId: string, blockId: string) =>
        blockId === 'child-page' ? [root] : []
    );
    api.fetchFrontstageBlockNode.mockImplementation(
      async (_workspaceId: string, _pageId: string, blockId: string) =>
        detail(blockId === 'child-page' ? child : root)
    );
    const view = render(
      <BlockSchemaTreePanel
        workspaceId="workspace-1"
        pageId="page-1"
        currentBlockId="child-page"
        onOpenBlock={vi.fn()}
      />,
      { wrapper: Wrapper }
    );

    expect(await screen.findByText('Child page')).toBeInTheDocument();
    const childIndent = screen
      .getByText('Child page')
      .closest('[role="treeitem"]')
      ?.querySelector('.ant-tree-indent-unit');
    expect(childIndent).not.toBeNull();
    expect(getComputedStyle(childIndent as Element).width).toBe(
      'var(--ant-tree-indent-size)'
    );
    expect(document.head).toHaveTextContent('--ant-tree-indent-size:12px');

    view.rerender(
      <BlockSchemaTreePanel
        workspaceId="workspace-1"
        pageId="page-1"
        currentBlockId="root-page"
        onOpenBlock={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(
        screen.getByText('Root page').closest('[role="treeitem"]')
      ).toHaveAttribute('aria-expanded', 'true');
    });
    expect(screen.getByText('Child page')).toBeInTheDocument();
  });

  test('AC-004 searches through the backend and preserves ancestor context', async () => {
    const { onOpenBlock } = renderPanel();
    fireEvent.change(
      screen.getByRole('searchbox', { name: /搜索区块|Search blocks/u }),
      { target: { value: 'child' } }
    );

    await waitFor(() => {
      expect(api.searchFrontstageBlocks).toHaveBeenCalledWith(
        'workspace-1',
        'page-1',
        { query: 'child' }
      );
    });
    expect(await screen.findByText('Root page / Child page')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /Child page/u }));
    expect(onOpenBlock).toHaveBeenCalledWith('child-page');
  });

  test('AC-004 creates a Page child with inherited tab and stable valid source', async () => {
    const { onOpenBlock } = renderPanel();
    await screen.findByText('Root page');
    fireEvent.click(
      screen.getByRole('button', { name: /新增子区块|Create child block/u })
    );
    fireEvent.change(screen.getByLabelText(/标题|Title/u), {
      target: { value: 'Nested page' }
    });
    fireEvent.change(screen.getByLabelText(/描述|Description/u), {
      target: { value: 'Nested page description' }
    });
    fireEvent.mouseDown(screen.getByLabelText(/展示方式|Presentation/u));
    fireEvent.click(await screen.findByText(/页面|Page/u, { selector: '.ant-select-item-option-content' }));
    const createDialog = await screen.findByRole('dialog');
    fireEvent.click(
      within(createDialog).getByRole('button', {
        name: /创\s*建|Create/u
      })
    );

    await waitFor(() => {
      expect(mutations.create.mutateAsync).toHaveBeenCalledWith({
        tab_id: 'tab-1',
        title: 'Nested page',
        description: 'Nested page description',
        presentation: 'page',
        parent_block_id: 'root-page',
        before_block_id: null,
        after_block_id: null,
        code: 'export default function Block() { return null; }\n',
        runtime_descriptor: null
      });
      expect(onOpenBlock).toHaveBeenCalledWith('child-page');
    });
  });

  test('AC-002/003 edits a legacy untitled root with its block id and optional description', async () => {
    const legacyRoot = { ...root, title: null, description: null };
    api.fetchFrontstageBlockRoots.mockResolvedValue([legacyRoot]);
    api.fetchFrontstageBlockNode.mockResolvedValue(detail(legacyRoot));
    renderPanel();

    await screen.findByText('root-page');
    fireEvent.click(screen.getByRole('button', { name: /编辑|Edit/u }));

    expect(screen.getByLabelText(/标题|Title/u)).toHaveValue('root-page');
    expect(screen.getByLabelText(/描述|Description/u)).toHaveValue('');
    fireEvent.change(screen.getByLabelText(/描述|Description/u), {
      target: { value: 'Root block description' }
    });
    fireEvent.click(
      within(await screen.findByRole('dialog')).getByRole('button', {
        name: /更\s*新|Update/u
      })
    );

    await waitFor(() => {
      expect(mutations.update.mutateAsync).toHaveBeenCalledWith({
        block_id: 'root-page',
        input: {
          title: 'root-page',
          description: 'Root block description',
          presentation: 'page'
        }
      });
    });
  });

  test('AC-001 keeps the create and edit form layers above their editor window', async () => {
    render(
      <WindowWorkspaceWindow
        active
        dragHandleSelector="[data-window-drag-handle='true']"
        initialRect={() => ({ left: 40, top: 40, width: 900, height: 700 })}
        resizeLabel={() => 'Resize editor'}
        testId="editor-window"
        title="TSX editor"
        zIndex={1123}
        onActivate={vi.fn()}
      >
        <BlockSchemaTreePanel
          workspaceId="workspace-1"
          pageId="page-1"
          currentBlockId="root-page"
          onOpenBlock={vi.fn()}
        />
      </WindowWorkspaceWindow>,
      { wrapper: Wrapper }
    );

    await screen.findByText('Root page');
    fireEvent.click(
      screen.getByRole('button', { name: /新增子区块|Create child block/u })
    );

    const createDialog = await screen.findByRole('dialog', {
      name: /新增子区块|Create child block/u
    });
    expect(createDialog.closest('.ant-modal-wrap')).toHaveStyle({
      zIndex: '1124'
    });

    fireEvent.mouseDown(screen.getByLabelText(/展示方式|Presentation/u));
    expect(
      (await screen.findByRole('listbox')).closest('.ant-select-dropdown')
    ).toHaveStyle({ zIndex: '1125' });

    fireEvent.click(
      within(createDialog).getByRole('button', { name: /取\s*消|Cancel/u })
    );
    fireEvent.click(screen.getByRole('button', { name: /编辑|Edit/u }));

    const editDialog = await screen.findByRole('dialog', {
      name: /编辑区块|Edit block/u
    });
    expect(editDialog.closest('.ant-modal-wrap')).toHaveStyle({
      zIndex: '1124'
    });
  });

  test('AC-004 edits title/presentation and confirms subtree impact', async () => {
    const { onDeletedBlock } = renderPanel();
    await screen.findByText('Root page');
    fireEvent.click(screen.getByRole('button', { name: /编辑|Edit/u }));
    fireEvent.change(screen.getByLabelText(/标题|Title/u), {
      target: { value: 'Edited root' }
    });
    fireEvent.mouseDown(screen.getByLabelText(/展示方式|Presentation/u));
    fireEvent.click(
      await screen.findByText(/抽屉|Drawer/u, {
        selector: '.ant-select-item-option-content'
      })
    );
    const editDialog = await screen.findByRole('dialog');
    fireEvent.click(
      within(editDialog).getByRole('button', {
        name: /更\s*新|Update/u
      })
    );
    await waitFor(() => {
      expect(mutations.update.mutateAsync).toHaveBeenCalledWith({
        block_id: 'root-page',
        input: { title: 'Edited root', description: '', presentation: 'drawer' }
      });
    });

    fireEvent.click(
      screen.getAllByRole('button', { name: /删除|Delete/u }).at(-1) as Element
    );
    expect(api.fetchFrontstageBlockDeleteImpact).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'root-page'
    );
    const deleteDialog = await screen.findByRole('dialog');
    expect(
      within(deleteDialog).getByText(/2 个区块|2 blocks/u)
    ).toBeInTheDocument();
    fireEvent.click(
      within(deleteDialog).getByRole('button', {
        name: /删\s*除|Delete/u
      })
    );
    await waitFor(() => {
      expect(mutations.deleteSubtree.mutateAsync).toHaveBeenCalledWith({
        block_id: 'root-page',
        parent_block_id: null,
        input: { expected_affected_count: 2 }
      });
      expect(onDeletedBlock).toHaveBeenCalledWith({
        block_id: 'root-page',
        subtree: true
      });
    });
  });

  test('AC-004 uses leaf delete when impact is exactly one', async () => {
    api.fetchFrontstageBlockDeleteImpact.mockResolvedValueOnce({
      affected_count: 1
    });
    const { onDeletedBlock } = renderPanel();
    await screen.findByText('Root page');
    fireEvent.click(screen.getByRole('button', { name: /删除|Delete/u }));
    const deleteDialog = await screen.findByRole('dialog');
    expect(
      within(deleteDialog).getByText(/叶子区块|leaf block/u)
    ).toBeInTheDocument();
    fireEvent.click(
      within(deleteDialog).getByRole('button', {
        name: /删\s*除|Delete/u
      })
    );
    await waitFor(() => {
      expect(mutations.deleteLeaf.mutateAsync).toHaveBeenCalledWith({
        block_id: 'root-page',
        parent_block_id: null
      });
      expect(onDeletedBlock).toHaveBeenCalledWith({
        block_id: 'root-page',
        subtree: false
      });
    });
  });

  test('AC-004 renders honest empty, error and permission states', async () => {
    api.fetchFrontstageBlockRoots.mockResolvedValueOnce([]);
    const { onOpenBlock: emptyOpenBlock } = renderPanel();
    expect(await screen.findByText(/当前页面没有区块|no blocks/u)).toBeInTheDocument();
    emptyOpenBlock.mockClear();

    api.fetchFrontstageBlockRoots.mockRejectedValueOnce(new Error('offline'));
    const { onOpenBlock: errorOpenBlock } = renderPanel({
      currentBlockId: 'error-root'
    });
    expect(
      await screen.findByText(/区块树加载失败|Block tree loading failed/u)
    ).toBeInTheDocument();
    errorOpenBlock.mockClear();

    api.fetchFrontstageBlockRoots.mockRejectedValueOnce({ status: 403 });
    renderPanel({ currentBlockId: 'forbidden-root' });
    await waitFor(() => {
      expect(document.querySelector('.ant-result-403')).not.toBeNull();
    });
  });

  test('AC-004 renders an honest loading state', () => {
    api.fetchFrontstageBlockRoots.mockReturnValueOnce(new Promise(() => {}));
    renderPanel({ currentBlockId: 'loading-root' });
    expect(
      screen.getByText(/正在加载区块树|Loading block tree/u)
    ).toBeInTheDocument();
  });
});
