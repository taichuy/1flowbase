import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import { FrontStagePageTreeSidebar } from '../../FrontStagePageTreeSidebar';
import type { FrontStageTreeNode } from '../../../lib/page-tree';
import { i18nText } from '../../../../../shared/i18n/text';

const nodes: FrontStageTreeNode[] = [
  { id: 'group', kind: 'group', title: 'Group', children: [
    { id: 'nested', kind: 'group', title: 'Nested', children: [
      { id: 'child', kind: 'page', title: 'Child' }
    ] }
  ] },
  { id: 'page', kind: 'page', title: 'Page' }
];
const selectPage = vi.fn();
const noop = () => {};

function setup(selectedPageId: string | null = null) {
  render(<FrontStagePageTreeSidebar pageTree={nodes} selectedPageId={selectedPageId}
    canEdit isOperationPending={false} onAddGroup={noop} onAddPage={noop}
    onAddPageInGroup={noop} onRenameNode={noop} onUpdateNodeMetadata={noop}
    onEditNodeTooltip={noop} onMoveNodeToPosition={noop} onDeleteNode={noop}
    onSelectPage={selectPage} />);
}
function item(name: string) { return screen.getByRole('treeitem', { name }); }
function focus(element: HTMLElement) { act(() => element.focus()); }
function key(element: HTMLElement, value: string) { fireEvent.keyDown(element, { key: value }); }

beforeEach(() => { localStorage.clear(); selectPage.mockClear(); });

test('exposes named tree/group/treeitems and one roving tab stop', () => {
  setup('child');
  expect(screen.getByRole('tree')).toBeInTheDocument();
  expect(screen.getAllByRole('group')).toHaveLength(2);
  expect(item('Group')).toHaveAttribute('aria-expanded', 'true');
  expect(item('Nested')).toHaveAttribute('aria-level', '2');
  expect(item('Child')).toHaveAttribute('aria-selected', 'true');
  expect(item('Child')).not.toHaveAttribute('aria-expanded');
  expect(screen.getAllByRole('treeitem').filter((node) => node.tabIndex === 0)).toEqual([item('Child')]);
  focus(item('Group'));
  expect(item('Group')).toHaveAttribute('tabindex', '0');
  expect(item('Child')).toHaveAttribute('tabindex', '-1');
});

test('moves with arrows, Home and End through currently rendered items only', () => {
  setup();
  focus(item('Group'));
  key(item('Group'), 'ArrowDown');
  expect(item('Nested')).toHaveFocus();
  key(item('Nested'), 'ArrowDown');
  expect(item('Child')).toHaveFocus();
  key(item('Child'), 'End');
  expect(item('Page')).toHaveFocus();
  key(item('Page'), 'ArrowUp');
  expect(item('Child')).toHaveFocus();
  key(item('Child'), 'Home');
  expect(item('Group')).toHaveFocus();
  key(item('Group'), 'ArrowLeft');
  expect(item('Group')).toHaveAttribute('aria-expanded', 'false');
  expect(screen.queryByRole('treeitem', { name: 'Child' })).not.toBeInTheDocument();
  key(item('Group'), 'ArrowDown');
  expect(item('Page')).toHaveFocus();
  key(item('Page'), 'ArrowDown');
  expect(item('Page')).toHaveFocus();
  key(item('Page'), 'Home');
  key(item('Group'), 'ArrowUp');
  expect(item('Group')).toHaveFocus();
});

test('Right expands then enters a group and Left collapses then returns to its parent', () => {
  localStorage.setItem('frontstage_collapsed_groups:v1', JSON.stringify(['group']));
  setup();
  focus(item('Group'));
  key(item('Group'), 'ArrowRight');
  expect(item('Group')).toHaveFocus();
  expect(item('Group')).toHaveAttribute('aria-expanded', 'true');
  key(item('Group'), 'ArrowRight');
  expect(item('Nested')).toHaveFocus();
  key(item('Nested'), 'ArrowRight');
  expect(item('Child')).toHaveFocus();
  key(item('Child'), 'ArrowLeft');
  expect(item('Nested')).toHaveFocus();
  key(item('Nested'), 'ArrowLeft');
  expect(item('Nested')).toHaveAttribute('aria-expanded', 'false');
  expect(item('Nested')).toHaveFocus();
  key(item('Nested'), 'ArrowLeft');
  expect(item('Group')).toHaveFocus();
  expect(selectPage).not.toHaveBeenCalled();
});

test('Enter and Space activate only the targeted treeitem once', () => {
  setup();
  focus(item('Page'));
  key(item('Page'), 'Enter');
  key(item('Page'), ' ');
  expect(selectPage.mock.calls).toEqual([['page'], ['page']]);
  key(item('Group'), 'Enter');
  expect(item('Group')).toHaveAttribute('aria-expanded', 'false');
  key(item('Group'), ' ');
  expect(item('Group')).toHaveAttribute('aria-expanded', 'true');
  key(item('Child'), 'Enter');
  expect(selectPage).toHaveBeenLastCalledWith('child');
  expect(item('Group')).toHaveAttribute('aria-expanded', 'true');
  expect(item('Nested')).toHaveAttribute('aria-expanded', 'true');
});

test('action buttons retain native activation without selecting or collapsing ancestor rows', () => {
  setup();
  const group = item('Group');
  const row = group.querySelector('.frontstage-page-tree-sidebar__node-row') as HTMLElement;
  const action = within(row).getByRole('button', { name: i18nText('frontstage', 'auto.page_action_menu') });
  key(action, 'Enter');
  fireEvent.keyUp(action, { key: 'Enter' });
  key(action, 'ArrowLeft');
  expect(group).toHaveAttribute('aria-expanded', 'true');
  expect(selectPage).not.toHaveBeenCalled();
  const collapse = within(row).getByRole('button', { name: 'Group' });
  fireEvent.click(collapse);
  expect(group).toHaveAttribute('aria-expanded', 'false');
  expect(selectPage).not.toHaveBeenCalled();
  fireEvent.click(collapse);
  expect(group).toHaveAttribute('aria-expanded', 'true');
});
