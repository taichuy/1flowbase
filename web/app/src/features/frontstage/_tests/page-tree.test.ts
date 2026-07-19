import { describe, expect, test } from 'vitest';

import {
  createGroupNode,
  createPageNode,
  getFirstPageId,
  getFirstTopLevelPageId,
  normalizePageTree,
  removeNodeFromTree,
  resolveSelectedPageId
} from '../lib/page-tree';

import type { FrontStageTreeNode } from '../lib/page-tree';

describe('frontstage page tree logic', () => {
  test('AC-001 chooses the first top-level page without descending into groups', () => {
    const tree: FrontStageTreeNode[] = [
      {
        id: 'group-first',
        title: 'Collapsed group',
        kind: 'group',
        children: [
          {
            id: 'page-in-group',
            title: 'Grouped page',
            kind: 'page'
          }
        ]
      },
      {
        id: 'page-top-level-first',
        title: 'First top-level page',
        kind: 'page'
      },
      {
        id: 'page-top-level-second',
        title: 'Second top-level page',
        kind: 'page'
      }
    ];

    expect(getFirstTopLevelPageId(tree)).toBe('page-top-level-first');
    expect(getFirstTopLevelPageId(tree.slice(0, 1))).toBeNull();
  });

  test('normalizes nested groups by preserving root groups and flattening descendant pages', () => {
    const tree: FrontStageTreeNode[] = [
      {
        id: 'group-root',
        title: 'Root group',
        kind: 'group',
        children: [
          {
            id: 'group-nested',
            title: 'Nested group',
            kind: 'group',
            children: [
              {
                id: 'page-nested',
                title: 'Nested page',
                kind: 'page'
              }
            ]
          },
          {
            id: 'page-direct',
            title: 'Direct page',
            kind: 'page'
          }
        ]
      },
      {
        id: 'page-root',
        title: 'Root page',
        kind: 'page'
      }
    ];

    expect(normalizePageTree(tree)).toEqual([
      {
        id: 'group-root',
        title: 'Root group',
        kind: 'group',
        children: [
          {
            id: 'page-nested',
            title: 'Nested page',
            kind: 'page'
          },
          {
            id: 'page-direct',
            title: 'Direct page',
            kind: 'page'
          }
        ]
      },
      {
        id: 'page-root',
        title: 'Root page',
        kind: 'page'
      }
    ]);
  });

  test('resolves missing pageId to the first backend page', () => {
    const tree: FrontStageTreeNode[] = [
      {
        id: 'group-root',
        title: 'Root group',
        kind: 'group',
        children: [
          {
            id: 'page-first',
            title: 'First page',
            kind: 'page'
          }
        ]
      },
      {
        id: 'page-second',
        title: 'Second page',
        kind: 'page'
      }
    ];

    expect(resolveSelectedPageId({ pageTree: tree }).selectedPageId).toBe(
      'page-first'
    );
    expect(resolveSelectedPageId({ pageTree: tree }).navigationTarget).toBe(
      'page-first'
    );
    expect(resolveSelectedPageId({ pageTree: tree }).shouldNavigate).toBe(true);
  });

  test('keeps an explicit deep link pageId that is missing from the backend tree', () => {
    const tree: FrontStageTreeNode[] = [
      {
        id: 'page-first',
        title: 'First page',
        kind: 'page'
      }
    ];

    expect(
      resolveSelectedPageId({ pageTree: tree, pageId: 'missing-page' })
    ).toEqual({
      selectedPageId: 'missing-page',
      navigationTarget: undefined,
      shouldNavigate: false
    });
  });

  test('keeps an explicit deep link pageId when the backend tree is empty', () => {
    expect(
      resolveSelectedPageId({ pageTree: [], pageId: 'missing-page' })
    ).toEqual({
      selectedPageId: 'missing-page',
      navigationTarget: undefined,
      shouldNavigate: false
    });
    expect(getFirstPageId([])).toBeNull();
  });

  test('deleting the selected page falls back to the next first page', () => {
    const tree: FrontStageTreeNode[] = [
      createPageNode('page-selected', 1),
      createPageNode('page-fallback', 2)
    ];

    const nextTree = removeNodeFromTree(tree, 'page-selected');

    expect(
      resolveSelectedPageId({
        pageTree: nextTree,
        currentSelectedPageId: 'page-selected'
      })
    ).toEqual({
      selectedPageId: 'page-fallback',
      navigationTarget: 'page-fallback',
      shouldNavigate: true
    });
  });

  test('creates draft nodes with deterministic titles from caller-provided ids', () => {
    expect(createGroupNode('group-draft', 2)).toEqual({
      id: 'group-draft',
      title: '分组 2',
      kind: 'group',
      children: []
    });
    expect(createPageNode('page-draft', 3)).toEqual({
      id: 'page-draft',
      title: '页面 新建 3',
      kind: 'page'
    });
  });
});
