import { describe, expect, test } from 'vitest';

import type { FrontstageBlockNodeSummary } from '../../../api/block-tree';
import {
  toBlockTreeMoveInput,
  type BlockSchemaTreeDropInfo,
  type BlockSchemaTreeNode
} from '../tree-drop';

function summary(
  block_id: string,
  parent_block_id: string | null
): FrontstageBlockNodeSummary {
  return {
    block_id,
    workspace_id: 'workspace-1',
    page_id: 'page-1',
    tab_id: 'tab-1',
    parent_block_id,
    rank: '001000',
    presentation: 'page',
    title: block_id,
    description: null,
    schema_version: 1,
    created_at: '2026-08-12T00:00:00Z',
    updated_at: '2026-08-12T00:00:00Z'
  };
}

function node(
  block_id: string,
  parent_block_id: string | null,
  pos: string
): BlockSchemaTreeNode & { pos: string } {
  return {
    key: block_id,
    title: block_id,
    summary: summary(block_id, parent_block_id),
    pos
  };
}

describe('block schema tree drop contract', () => {
  test('AC-004 moves Page under Page using only public position fields', () => {
    const target = node('target-page', null, '0-0');
    const input = toBlockTreeMoveInput({
      dragNode: node('dragged-page', null, '0-1'),
      node: target,
      dropToGap: false,
      dropPosition: 0
    } as BlockSchemaTreeDropInfo);

    expect(input).toEqual({
      parent_block_id: 'target-page',
      before_block_id: null,
      after_block_id: null
    });
    expect(input).not.toHaveProperty('rank');
  });

  test('AC-004 translates before and after gaps without sorting locally', () => {
    const target = node('target', 'parent', '0-2-1');
    expect(
      toBlockTreeMoveInput({
        dragNode: node('dragged', 'old-parent', '0-1'),
        node: target,
        dropToGap: true,
        dropPosition: 0
      } as BlockSchemaTreeDropInfo)
    ).toEqual({
      parent_block_id: 'parent',
      before_block_id: 'target',
      after_block_id: null
    });
    expect(
      toBlockTreeMoveInput({
        dragNode: node('dragged', 'old-parent', '0-1'),
        node: target,
        dropToGap: true,
        dropPosition: 2
      } as BlockSchemaTreeDropInfo)
    ).toEqual({
      parent_block_id: 'parent',
      before_block_id: null,
      after_block_id: 'target'
    });
  });
});
