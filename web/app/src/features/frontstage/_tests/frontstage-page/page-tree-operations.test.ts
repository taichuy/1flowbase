import { describe, expect, test } from 'vitest';

import { moveNodeToTreePosition } from '../../pages/frontstage-page/page-tree-operations';
import type { FrontStageTreeNode } from '../../lib/page-tree';

describe('nested frontstage page groups', () => {
  const tree: FrontStageTreeNode[] = [
    {
      id: 'parent',
      title: 'Parent',
      kind: 'group',
      children: [{ id: 'page', title: 'Page', kind: 'page' }]
    },
    { id: 'nested', title: 'Nested', kind: 'group', children: [] }
  ];

  test('moves a group inside another group without flattening its pages', () => {
    expect(moveNodeToTreePosition(tree, 'parent', 'nested', 'inside')).toEqual([
      {
        id: 'nested',
        title: 'Nested',
        kind: 'group',
        children: [tree[0]]
      }
    ]);
  });

  test('rejects moving a group into its own descendant', () => {
    const nested = moveNodeToTreePosition(tree, 'nested', 'parent', 'inside');
    expect(
      moveNodeToTreePosition(nested, 'parent', 'nested', 'inside')
    ).toEqual(nested);
  });
});
