import {
  createEvent,
  fireEvent,
  render,
  screen,
  within
} from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import { FrontStagePageTreeSidebar } from '../../FrontStagePageTreeSidebar';
import type { FrontStageTreeNode } from '../../../lib/page-tree';

const tree: FrontStageTreeNode[] = [
  {
    id: 'group',
    title: 'Group',
    kind: 'group',
    children: [{ id: 'nested', title: 'Nested', kind: 'group', children: [] }]
  },
  { id: 'page', title: 'Page', kind: 'page' }
];
const move = vi.fn();
const noop = () => {};
function setup(pending = false) {
  render(
    <FrontStagePageTreeSidebar
      pageTree={tree}
      selectedPageId={null}
      canEdit
      isOperationPending={pending}
      onAddGroup={noop}
      onAddPage={noop}
      onAddPageInGroup={noop}
      onRenameNode={noop}
      onUpdateNodeMetadata={noop}
      onEditNodeTooltip={noop}
      onMoveNodeToPosition={move}
      onDeleteNode={noop}
      onSelectPage={noop}
    />
  );
  const data = new Map<string, string>();
  return {
    setDragImage: vi.fn(),
    effectAllowed: '',
    dropEffect: '',
    setData: (key: string, value: string) => data.set(key, value),
    getData: (key: string) => data.get(key) ?? ''
  };
}
function item(kind: string, title: string) {
  return screen.getByTestId(`frontstage-tree-node-${kind}-${title}`);
}
function start(node: HTMLElement, dataTransfer: ReturnType<typeof setup>) {
  fireEvent.dragStart(
    within(node).getAllByRole('button', { name: '拖拽移动节点' })[0],
    { dataTransfer }
  );
}
function over(
  node: HTMLElement,
  dataTransfer: ReturnType<typeof setup>,
  y: number
) {
  const row = node.querySelector('.frontstage-page-tree-sidebar__node-row')!;
  vi.spyOn(row, 'getBoundingClientRect').mockReturnValue({
    top: 0,
    height: 100
  } as DOMRect);
  const event = createEvent.dragOver(row, { dataTransfer });
  Object.defineProperty(event, 'clientY', { value: y });
  fireEvent(row, event);
}
beforeEach(() => {
  move.mockClear();
  localStorage.clear();
});

test.each([
  [10, 'before'],
  [50, 'inside'],
  [90, 'after']
] as const)('group interval %s previews and commits %s', (y, position) => {
  const transfer = setup();
  start(item('page', 'Page'), transfer);
  over(item('group', 'Group'), transfer, y);
  expect(screen.getByRole('status')).toHaveTextContent('Page → Group');
  fireEvent.drop(item('group', 'Group'), { dataTransfer: transfer });
  expect(move).toHaveBeenCalledWith('page', 'group', position);
  expect(screen.queryByRole('status')).not.toBeInTheDocument();
});

test('nested hover owns the projection without its ancestor overwriting it', () => {
  const transfer = setup();
  start(item('page', 'Page'), transfer);
  over(item('group', 'Nested'), transfer, 50);
  expect(screen.getByRole('status')).toHaveTextContent('Page → Nested');
  fireEvent.drop(item('group', 'Nested'), { dataTransfer: transfer });
  expect(move).toHaveBeenCalledWith('page', 'nested', 'inside');
});

test('self and descendant targets neither preview nor submit', () => {
  const transfer = setup();
  start(item('group', 'Group'), transfer);
  over(item('group', 'Nested'), transfer, 50);
  expect(screen.queryByRole('status')).not.toBeInTheDocument();
  expect(transfer.dropEffect).toBe('none');
  fireEvent.drop(item('group', 'Nested'), { dataTransfer: transfer });
  expect(move).not.toHaveBeenCalled();
});

test('collapsed groups expose a child destination and cancellation clears it', () => {
  localStorage.setItem(
    'frontstage_collapsed_groups',
    JSON.stringify(['group'])
  );
  // setup preserves stored collapsed state.
  const transfer = setup();
  expect(
    screen.queryByTestId('frontstage-tree-node-group-Nested')
  ).not.toBeInTheDocument();
  const source = item('page', 'Page');
  start(source, transfer);
  over(item('group', 'Group'), transfer, 50);
  expect(screen.getByRole('status')).toHaveTextContent('放入分组');
  fireEvent.dragEnd(
    within(source).getByRole('button', { name: '拖拽移动节点' }),
    { dataTransfer: transfer }
  );
  expect(screen.queryByRole('status')).not.toBeInTheDocument();
  expect(
    screen.queryByTestId('frontstage-tree-node-group-Nested')
  ).not.toBeInTheDocument();
  expect(move).not.toHaveBeenCalled();
});

test('leaving the sidebar invalidates the destination', () => {
  const transfer = setup();
  start(item('page', 'Page'), transfer);
  over(item('group', 'Group'), transfer, 50);
  fireEvent.dragLeave(document.querySelector('.frontstage-page-tree-sidebar')!);
  fireEvent.drop(item('group', 'Group'), { dataTransfer: transfer });
  expect(move).not.toHaveBeenCalled();
});

test('pending operations cannot start a move or display a destination', () => {
  const transfer = setup(true);
  const source = item('page', 'Page');
  expect(
    within(source).getByRole('button', { name: '拖拽移动节点' })
  ).toBeDisabled();
  start(source, transfer);
  over(item('group', 'Group'), transfer, 50);
  fireEvent.drop(item('group', 'Group'), { dataTransfer: transfer });
  expect(screen.queryByRole('status')).not.toBeInTheDocument();
  expect(move).not.toHaveBeenCalled();
});
