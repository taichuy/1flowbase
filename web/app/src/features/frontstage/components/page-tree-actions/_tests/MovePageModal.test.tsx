import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, expect, test, vi } from 'vitest';
import { AppProviders } from '../../../../../app/AppProviders';
import type { FrontStageTreeNode } from '../../../lib/page-tree';
import { MovePageModal } from '../MovePageModal';

const source: FrontStageTreeNode = {
  id: 'source',
  title: '待移动',
  kind: 'page',
  placement: 'sidebar'
};
const tree: FrontStageTreeNode[] = [
  {
    id: 'origin',
    title: '来源分组',
    kind: 'group',
    placement: 'sidebar',
    children: [source]
  },
  {
    id: 'destination',
    title: '目标分组',
    kind: 'group',
    placement: 'sidebar',
    children: [
      { id: 'first', title: '已有页面一', kind: 'page', placement: 'sidebar' },
      { id: 'second', title: '已有页面二', kind: 'page', placement: 'sidebar' },
      {
        id: 'nested',
        title: '嵌套分组',
        kind: 'group',
        placement: 'sidebar',
        children: [
          { id: 'deep', title: '深层页面', kind: 'page', placement: 'sidebar' }
        ]
      }
    ]
  }
];
function setup(node = source) {
  const onMove = vi.fn().mockResolvedValue(true);
  const onCancel = vi.fn();
  render(
    <AppProviders>
      <MovePageModal
        node={node}
        pageTree={tree}
        isOperationPending={false}
        onMove={onMove}
        onCancel={onCancel}
      />
    </AppProviders>
  );
  return { onMove, onCancel, dialog: screen.getByRole('dialog') };
}
function beginDrag() {
  const dataTransfer = {
    setData: vi.fn(),
    getData: () => source.id,
    effectAllowed: '',
    dropEffect: ''
  };
  fireEvent.dragStart(screen.getByLabelText('拖拽移动节点 待移动'), {
    dataTransfer
  });
  return dataTransfer;
}
function hover(
  targetId: string,
  clientY: number,
  dataTransfer: ReturnType<typeof beginDrag>
) {
  const row = document.querySelector<HTMLElement>(
    `.move-page-modal__row[data-node-id="${targetId}"]`
  )!;
  vi.spyOn(row, 'getBoundingClientRect').mockReturnValue({
    x: 0,
    y: 100,
    top: 100,
    bottom: 200,
    left: 0,
    right: 240,
    width: 240,
    height: 100,
    toJSON: () => ({})
  });
  const event = new Event('dragover', { bubbles: true, cancelable: true });
  Object.defineProperties(event, {
    clientY: { value: clientY },
    dataTransfer: { value: dataTransfer }
  });
  fireEvent(row, event);
  return row;
}
afterEach(() => vi.restoreAllMocks());

test('selecting a page stages an after projection and saves its parent and anchor', async () => {
  const { onMove, dialog } = setup();
  fireEvent.click(
    dialog.querySelector('.move-page-modal__row[data-node-id="destination"]')!
  );
  fireEvent.click(within(dialog).getByText('已有页面一'));
  expect(screen.getByTestId('move-page-projection')).toHaveAttribute(
    'data-position',
    'after'
  );
  expect(screen.getByTestId('move-page-projection')).toHaveAttribute(
    'data-target-id',
    'first'
  );
  expect(onMove).not.toHaveBeenCalled();
  await act(async () =>
    within(dialog)
      .getByRole('button', { name: /确\s*定/ })
      .click()
  );
  expect(onMove).toHaveBeenCalledWith('source', {
    parentId: 'destination',
    after_id: 'first'
  });
});

test('a group is moved as one node and cannot be placed inside its descendants', async () => {
  const { onMove, dialog } = setup(tree[1]);
  fireEvent.click(
    dialog.querySelector('.move-page-modal__row[data-node-id="destination"]')!
  );
  expect(screen.queryByTestId('move-page-projection')).not.toBeInTheDocument();
  fireEvent.click(within(dialog).getByText('来源分组'));
  await act(async () =>
    within(dialog)
      .getByRole('button', { name: /确\s*定/ })
      .click()
  );
  expect(onMove).toHaveBeenCalledWith('destination', {
    parentId: 'origin',
    after_id: 'source'
  });
});

test('starts at root level and selecting a group shows its direct children and append projection', async () => {
  const { onMove, dialog } = setup();
  expect(within(dialog).getByText('顶部导航栏')).toBeInTheDocument();
  expect(within(dialog).queryByText('不分组')).not.toBeInTheDocument();
  expect(within(dialog).getByText('目标分组')).toBeInTheDocument();
  expect(within(dialog).queryByText('已有页面一')).not.toBeInTheDocument();
  expect(screen.queryByTestId('move-page-projection')).not.toBeInTheDocument();
  fireEvent.click(within(dialog).getByText('目标分组'));
  expect(within(dialog).getByText('已有页面一')).toBeInTheDocument();
  expect(within(dialog).getByText('嵌套分组')).toBeInTheDocument();
  expect(within(dialog).queryByText('深层页面')).not.toBeInTheDocument();
  const projection = screen.getByTestId('move-page-projection');
  expect(projection).toHaveAttribute('data-target-id', 'destination');
  expect(projection).toHaveAttribute('data-position', 'inside');
  expect(projection).toHaveAttribute('draggable', 'true');
  expect(onMove).not.toHaveBeenCalled();
  await act(async () =>
    within(dialog)
      .getByRole('button', { name: /确\s*定/ })
      .click()
  );
  expect(onMove).toHaveBeenCalledWith('source', {
    parentId: 'destination',
    after_id: 'nested'
  });
});

test.each(['before', 'after'] as const)(
  'drop keeps the visible %s slot even when release coordinates disagree',
  async (position) => {
    const { onMove, dialog } = setup();
    fireEvent.click(within(dialog).getByText('目标分组'));
    const transfer = beginDrag();
    hover('second', position === 'before' ? 110 : 190, transfer);
    const projection = screen.getByTestId('move-page-projection');
    expect(projection).toHaveAttribute('data-position', position);
    expect(projection).toHaveAttribute('data-target-id', 'second');
    expect(projection.closest('.move-page-modal__row')).toBeNull();
    const secondRow = dialog.querySelector(
      '.move-page-modal__row[data-node-id="second"]'
    )!;
    expect(
      secondRow.compareDocumentPosition(projection) &
        (position === 'before'
          ? Node.DOCUMENT_POSITION_PRECEDING
          : Node.DOCUMENT_POSITION_FOLLOWING)
    ).toBeTruthy();
    expect(
      within(dialog).getByRole('button', { name: /确\s*定/ })
    ).toBeDisabled();
    fireEvent.drop(projection, {
      clientY: position === 'before' ? 190 : 110,
      dataTransfer: transfer
    });
    expect(onMove).not.toHaveBeenCalled();
    expect(screen.getByTestId('move-page-projection')).toHaveAttribute(
      'data-position',
      position
    );
    await act(async () =>
      within(dialog)
        .getByRole('button', { name: /确\s*定/ })
        .click()
    );
    expect(onMove).toHaveBeenCalledWith('source', {
      parentId: 'destination',
      [`${position}_id`]: 'second'
    });
  }
);

test('dragging onto a collapsed group opens one child layer and cancellation restores the staged projection', () => {
  const { onMove, onCancel, dialog } = setup();
  const transfer = beginDrag();
  const row = hover('destination', 150, transfer);
  expect(within(dialog).getByText('嵌套分组')).toBeInTheDocument();
  expect(within(dialog).queryByText('深层页面')).not.toBeInTheDocument();
  fireEvent.drop(row, { dataTransfer: transfer });
  const nextTransfer = beginDrag();
  hover('second', 110, nextTransfer);
  fireEvent.dragEnd(screen.getByLabelText('拖拽移动节点 待移动'));
  expect(screen.getByTestId('move-page-projection')).toHaveAttribute(
    'data-target-id',
    'destination'
  );
  expect(screen.getByTestId('move-page-projection')).toHaveAttribute(
    'data-position',
    'inside'
  );
  fireEvent.click(within(dialog).getByRole('button', { name: /取\s*消/ }));
  expect(onCancel).toHaveBeenCalled();
  expect(onMove).not.toHaveBeenCalled();
});

test('the after-group projection follows its visible subtree at the group indentation', async () => {
  const { onMove, dialog } = setup();
  fireEvent.click(within(dialog).getByText('目标分组'));
  const transfer = beginDrag();
  hover('destination', 190, transfer);
  const projection = screen.getByTestId('move-page-projection');
  expect(projection.closest('.move-page-modal__row')).toBeNull();
  const destinationRow = dialog.querySelector(
    '.move-page-modal__row[data-node-id="destination"]'
  )!;
  const nestedRow = dialog.querySelector(
    '.move-page-modal__row[data-node-id="nested"]'
  )!;
  const projectionItem = projection.closest('[role="treeitem"]')!;
  expect(
    projectionItem.querySelector('.ant-tree-indent')?.childElementCount
  ).toBe(
    destinationRow
      .closest('[role="treeitem"]')!
      .querySelector('.ant-tree-indent')?.childElementCount
  );
  expect(
    nestedRow.compareDocumentPosition(projection) &
      Node.DOCUMENT_POSITION_FOLLOWING
  ).toBeTruthy();
  fireEvent.drop(projection, { dataTransfer: transfer });
  await act(async () =>
    within(dialog)
      .getByRole('button', { name: /确\s*定/ })
      .click()
  );
  expect(onMove).toHaveBeenCalledWith('source', {
    parentId: null,
    after_id: 'destination'
  });
});

test('clicking a group after staging a sibling move selects its inside destination', () => {
  const { dialog } = setup();
  fireEvent.click(within(dialog).getByText('目标分组'));
  const transfer = beginDrag();
  hover('destination', 190, transfer);
  fireEvent.drop(screen.getByTestId('move-page-projection'), {
    dataTransfer: transfer
  });
  fireEvent.click(within(dialog).getByText('目标分组'));
  expect(screen.getByTestId('move-page-projection')).toHaveAttribute(
    'data-position',
    'inside'
  );
});
