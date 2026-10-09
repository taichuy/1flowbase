import { expect, test } from 'vitest';
import type { FrontStageTreeNode } from '../../page-tree';
import { resolveNavigationMove } from '../move-plan';
const tree: FrontStageTreeNode[] = [
  {
    id: 'origin',
    title: 'Origin',
    kind: 'group',
    placement: 'sidebar',
    children: [
      { id: 'source', title: 'Source', kind: 'page', placement: 'sidebar' }
    ]
  },
  {
    id: 'target',
    title: 'Target',
    kind: 'group',
    placement: 'sidebar',
    children: [
      { id: 'anchor', title: 'Anchor', kind: 'page', placement: 'sidebar' }
    ]
  },
  {
    id: 'topbar',
    title: 'Topbar',
    kind: 'group',
    placement: 'topbar',
    children: []
  }
];
test('cross-parent drafts use destination anchors rather than guessing rank values', () => {
  expect(
    resolveNavigationMove(tree, 'source', {
      targetNodeId: 'anchor',
      position: 'before'
    })
  ).toEqual({ parentId: 'target', before_id: 'anchor' });
  expect(
    resolveNavigationMove(tree, 'source', {
      targetNodeId: 'anchor',
      position: 'after'
    })
  ).toEqual({ parentId: 'target', after_id: 'anchor' });
  expect(
    resolveNavigationMove(tree, 'source', {
      targetNodeId: 'target',
      position: 'inside'
    })
  ).toEqual({ parentId: 'target', after_id: 'anchor' });
});
test('rejects self and page-as-parent drafts and filters root anchors by navigation placement', () => {
  expect(
    resolveNavigationMove(tree, 'source', {
      targetNodeId: 'source',
      position: 'before'
    })
  ).toBeNull();
  expect(
    resolveNavigationMove(tree, 'source', {
      targetNodeId: 'anchor',
      position: 'inside'
    })
  ).toBeNull();
  expect(
    resolveNavigationMove(tree, 'source', {
      targetNodeId: 'topbar',
      position: 'before'
    })
  ).toEqual({ parentId: null, before_id: 'topbar' });
  expect(
    resolveNavigationMove(tree, 'source', {
      targetNodeId: 'topbar',
      position: 'inside'
    })
  ).toEqual({ parentId: 'topbar', rank: '001000' });
  expect(
    resolveNavigationMove(tree, 'source', {
      targetNodeId: null,
      position: 'inside'
    })
  ).toEqual({ parentId: null, after_id: 'target' });
});

test('topbar groups append after sidebar children and group descendants remain invalid targets', () => {
  expect(
    resolveNavigationMove(tree, 'topbar', {
      targetNodeId: 'target',
      position: 'inside'
    })
  ).toEqual({ parentId: 'target', after_id: 'anchor' });
  expect(
    resolveNavigationMove(tree, 'target', {
      targetNodeId: 'anchor',
      position: 'after'
    })
  ).toBeNull();
  expect(
    resolveNavigationMove(tree, 'source', {
      targetNodeId: 'origin',
      position: 'inside'
    })
  ).toEqual({ parentId: 'origin', rank: '001000' });
});
