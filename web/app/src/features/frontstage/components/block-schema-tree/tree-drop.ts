import type { TreeProps } from 'antd';
import type { ReactNode } from 'react';

import type {
  FrontstageBlockNodeSummary,
  MoveFrontstageBlockNodeInput
} from '../../api/block-tree';

export interface BlockSchemaTreeNode {
  key: string;
  title: string;
  summary: FrontstageBlockNodeSummary;
  icon?: ReactNode;
  children?: BlockSchemaTreeNode[];
  isLeaf?: boolean;
}

export type BlockSchemaTreeDropInfo = Parameters<
  NonNullable<TreeProps<BlockSchemaTreeNode>['onDrop']>
>[0];

export function toBlockTreeMoveInput(
  info: BlockSchemaTreeDropInfo
): MoveFrontstageBlockNodeInput {
  if (!info.dropToGap) {
    return {
      parent_block_id: info.node.summary.block_id,
      before_block_id: null,
      after_block_id: null
    };
  }

  const positions = info.node.pos.split('-');
  const relativePosition =
    info.dropPosition - Number(positions[positions.length - 1]);

  return {
    parent_block_id: info.node.summary.parent_block_id,
    before_block_id:
      relativePosition < 0 ? info.node.summary.block_id : null,
    after_block_id:
      relativePosition > 0 ? info.node.summary.block_id : null
  };
}
