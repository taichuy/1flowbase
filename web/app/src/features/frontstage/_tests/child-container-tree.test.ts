import { describe, expect, test } from 'vitest';

import {
  ChildContainerTreeError,
  addChildContainer,
  addSiblingChildContainer,
  deleteChildContainer,
  moveChildContainer,
  normalizeChildContainerTree,
  reorderChildContainer,
  resolveChildContainerPath,
  type ChildContainerNode
} from '../lib/child-container-tree';

const root: ChildContainerNode = {
  id: 'root-drawer',
  ownerBlockId: 'launcher',
  parentId: null,
  rank: '001000',
  presentation: 'drawer',
  title: 'Root',
  blockIds: ['root-content']
};

function serialized(container: ChildContainerNode): Record<string, unknown> {
  return {
    container_id: container.id,
    owner_block_id: container.ownerBlockId,
    parent_container_id: container.parentId,
    rank: container.rank,
    presentation: container.presentation,
    title: container.title,
    block_ids: container.blockIds
  };
}

function expectTreeError(action: () => unknown, code: string) {
  expect(action).toThrowError(ChildContainerTreeError);
  expect(action).toThrowError(expect.objectContaining({ code }));
}

describe('frontstage child container tree', () => {
  test('AC-002 creates stable child/sibling ids, resolves paths, and supports reorder and move', () => {
    const ids = ['child-modal', 'sibling-inline'];
    const idFactory = () => ids.shift() ?? 'unexpected-id';
    const withChild = addChildContainer(
      [root],
      root.id,
      {
        ownerBlockId: 'root-content',
        presentation: 'modal',
        title: 'Child',
        blockIds: ['child-content']
      },
      idFactory
    );
    const withSibling = addSiblingChildContainer(
      withChild,
      'child-modal',
      {
        ownerBlockId: 'other-launcher',
        presentation: 'inline',
        title: 'Sibling',
        blockIds: []
      },
      idFactory
    );

    expect(
      resolveChildContainerPath(withSibling, 'child-modal')?.map(({ id }) => id)
    ).toEqual(['root-drawer', 'child-modal']);
    expect(
      reorderChildContainer(withSibling, 'sibling-inline', 0)
        .filter(({ parentId }) => parentId === root.id)
        .map(({ id }) => id)
    ).toEqual(['sibling-inline', 'child-modal']);
    expect(
      resolveChildContainerPath(
        moveChildContainer(withSibling, 'child-modal', null, 0),
        'child-modal'
      )?.map(({ id }) => id)
    ).toEqual(['child-modal']);

    expectTreeError(
      () => moveChildContainer(withSibling, root.id, 'child-modal', 0),
      'cycle'
    );
  });

  test('AC-002 rejects invalid persisted identity, ancestry, and block ownership', () => {
    const invalidCases: Array<[unknown[], string]> = [
      [[serialized(root), serialized(root)], 'duplicate_container_id'],
      [
        [{ ...serialized(root), parent_container_id: 'missing' }],
        'missing_parent'
      ],
      [
        [
          { ...serialized(root), parent_container_id: 'child' },
          {
            container_id: 'child',
            owner_block_id: 'child-owner',
            parent_container_id: root.id,
            rank: '002000',
            presentation: 'modal',
            title: 'Child',
            block_ids: []
          }
        ],
        'cycle'
      ],
      [
        [
          serialized(root),
          {
            container_id: 'child',
            owner_block_id: 'child-owner',
            parent_container_id: root.id,
            rank: '002000',
            presentation: 'modal',
            title: 'Child',
            block_ids: ['root-content']
          }
        ],
        'duplicate_block_assignment'
      ],
      [
        [{ ...serialized(root), block_ids: ['launcher'] }],
        'owner_self_containment'
      ]
    ];

    for (const [input, code] of invalidCases) {
      const normalized = normalizeChildContainerTree(input);
      expect(normalized.containers).toEqual([]);
      expect(normalized.diagnostics).toEqual(
        expect.arrayContaining([expect.objectContaining({ code })])
      );
    }
  });

  test('AC-009 only deletes empty leaves and rejects explicit target references', () => {
    const leaf = { ...root, blockIds: [] };

    expect(
      deleteChildContainer([leaf], leaf.id, { targetContainerIds: [] })
    ).toEqual([]);
    expectTreeError(
      () =>
        deleteChildContainer([leaf], leaf.id, {
          targetContainerIds: [leaf.id]
        }),
      'container_referenced'
    );
    expectTreeError(
      () => deleteChildContainer([root], root.id, { targetContainerIds: [] }),
      'container_not_empty'
    );
    expectTreeError(
      () =>
        deleteChildContainer(
          [
            leaf,
            {
              ...leaf,
              id: 'child',
              parentId: leaf.id,
              rank: '001000'
            }
          ],
          leaf.id,
          { targetContainerIds: [] }
        ),
      'container_has_children'
    );
  });
});
