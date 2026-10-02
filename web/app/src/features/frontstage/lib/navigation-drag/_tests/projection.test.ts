import { expect, test } from 'vitest';
import {
  canProjectNavigationMove,
  projectNavigationPosition
} from '../projection';
import type { FrontStageTreeNode } from '../../page-tree';

const tree: FrontStageTreeNode[] = [
  {
    id: 'group',
    title: 'Group',
    kind: 'group',
    children: [
      {
        id: 'nested',
        title: 'Nested',
        kind: 'group',
        children: [{ id: 'child', title: 'Child', kind: 'page' }]
      }
    ]
  },
  { id: 'page', title: 'Page', kind: 'page' }
];

test('list midpoint and group edge/centre intervals have distinct outcomes', () => {
  expect(projectNavigationPosition(149, 100, 100, false)).toBe('before');
  expect(projectNavigationPosition(151, 100, 100, false)).toBe('after');
  expect(projectNavigationPosition(110, 100, 100, true)).toBe('before');
  expect(projectNavigationPosition(150, 100, 100, true)).toBe('inside');
  expect(projectNavigationPosition(190, 100, 100, true)).toBe('after');
  expect(projectNavigationPosition(NaN, 100, 100, true)).toBeNull();
  expect(projectNavigationPosition(100, 100, 0, true)).toBeNull();
});

test('tree constraints reject cycles and page containment, allow moving out of a group', () => {
  expect(canProjectNavigationMove(tree, 'group', 'nested', 'inside')).toBe(
    false
  );
  expect(canProjectNavigationMove(tree, 'group', 'child', 'before')).toBe(
    false
  );
  expect(canProjectNavigationMove(tree, 'group', 'group', 'after')).toBe(false);
  expect(canProjectNavigationMove(tree, 'nested', 'page', 'inside')).toBe(
    false
  );
  expect(canProjectNavigationMove(tree, 'child', 'group', 'before')).toBe(true);
  expect(canProjectNavigationMove(tree, 'page', 'nested', 'inside')).toBe(true);
  expect(canProjectNavigationMove(tree, 'missing', 'group', 'inside')).toBe(
    false
  );
});
