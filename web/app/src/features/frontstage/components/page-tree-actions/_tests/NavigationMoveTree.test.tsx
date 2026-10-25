import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, test, vi } from 'vitest';
import { AppProviders } from '../../../../../app/AppProviders';
import { NavigationMoveTree } from '../NavigationMoveTree';
import type { FrontStageTreeNode } from '../../../lib/page-tree';

const tree: FrontStageTreeNode[] = [
  { id: 'a', title: 'Page A', kind: 'page', placement: 'topbar' },
  { id: 'b', title: 'Page B', kind: 'page', placement: 'topbar' },
  {
    id: 'group',
    title: 'Group',
    kind: 'group',
    placement: 'topbar',
    children: [
      {
        id: 'nested',
        title: 'Nested',
        kind: 'group',
        placement: 'sidebar',
        children: []
      }
    ]
  }
];
const row = (id: string) =>
  document.querySelector<HTMLElement>(`[data-node-id="${id}"]`)!;
const transfer = () => ({
  setData: vi.fn(),
  effectAllowed: '',
  dropEffect: ''
});
function hover(
  id: string,
  clientY: number,
  dataTransfer: ReturnType<typeof transfer>
) {
  const target = row(id);
  vi.spyOn(target, 'getBoundingClientRect').mockReturnValue({
    top: 100,
    height: 100
  } as DOMRect);
  const event = new Event('dragover', { bubbles: true, cancelable: true });
  Object.defineProperties(event, {
    clientY: { value: clientY },
    dataTransfer: { value: dataTransfer }
  });
  fireEvent(target, event);
}
function setup(disabled = false) {
  const onSelectMove = vi.fn();
  render(
    <AppProviders>
      <NavigationMoveTree
        pageTree={tree}
        disabled={disabled}
        onSelectMove={onSelectMove}
      />
    </AppProviders>
  );
  return onSelectMove;
}
afterEach(() => vi.restoreAllMocks());

test('all nodes can initiate independent moves; dropping saves immediately and clears the projection', () => {
  const move = setup();
  expect(row('__frontstage_root__')).toHaveAttribute('draggable', 'false');
  const first = transfer();
  fireEvent.dragStart(row('a'), { dataTransfer: first });
  hover('group', 150, first);
  expect(screen.getByText('Nested')).toBeInTheDocument();
  expect(move).not.toHaveBeenCalled();
  fireEvent.drop(screen.getByTestId('move-page-projection'), {
    dataTransfer: first
  });
  expect(move).toHaveBeenLastCalledWith(
    'a',
    {
      parentId: 'group',
      after_id: 'nested'
    },
    { targetNodeId: 'group', position: 'inside' }
  );
  expect(screen.queryByTestId('move-page-projection')).not.toBeInTheDocument();
  const second = transfer();
  fireEvent.dragStart(row('b'), { dataTransfer: second });
  hover('a', 110, second);
  fireEvent.drop(row('a'), { dataTransfer: second });
  expect(move).toHaveBeenLastCalledWith(
    'b',
    {
      parentId: null,
      before_id: 'a'
    },
    { targetNodeId: 'a', position: 'before' }
  );
  expect(move).toHaveBeenCalledTimes(2);
});

test('moving out to an empty placement at root preserves the existing rank contract', () => {
  const move = setup();
  fireEvent.click(row('group'));
  const dataTransfer = transfer();
  fireEvent.dragStart(row('nested'), { dataTransfer });
  hover('__frontstage_root__', 150, dataTransfer);
  fireEvent.drop(row('__frontstage_root__'), { dataTransfer });
  expect(move).toHaveBeenCalledWith(
    'nested',
    {
      parentId: null,
      rank: '001000'
    },
    { targetNodeId: null, position: 'inside' }
  );
});

test('self and descendant drops are rejected, and cancelling a valid drag does not save', () => {
  const move = setup();
  fireEvent.click(row('group'));
  const dataTransfer = transfer();
  fireEvent.dragStart(row('group'), { dataTransfer });
  for (const id of ['group', 'nested']) {
    hover(id, 150, dataTransfer);
    expect(
      screen.queryByTestId('move-page-projection')
    ).not.toBeInTheDocument();
  }
  hover('a', 190, dataTransfer);
  expect(screen.getByTestId('move-page-projection')).toHaveAttribute(
    'data-position',
    'after'
  );
  fireEvent.dragEnd(row('group'));
  expect(screen.queryByTestId('move-page-projection')).not.toBeInTheDocument();
  expect(move).not.toHaveBeenCalled();
});

test('disabled trees cannot stage or submit moves', () => {
  const move = setup(true);
  expect(row('a')).toHaveAttribute('draggable', 'false');
  const dataTransfer = transfer();
  fireEvent.dragStart(row('a'), { dataTransfer });
  hover('b', 190, dataTransfer);
  fireEvent.drop(row('b'), { dataTransfer });
  expect(move).not.toHaveBeenCalled();
});
