import { findNodeById } from '../../lib/page-tree';
import type { FrontStageTreeNode } from '../../lib/page-tree';

type CreatePageTreeNodeInput = {
  title: string | null;
  icon?: string | null;
  tooltip?: string | null;
  parentId: string | null;
  rank: string;
};

type RenamePageTreeNodeInput = {
  title: string | null;
  icon?: string | null;
  tooltip?: string | null;
  contentPresentation?: 'single' | 'tabs';
};

type UpdatePageTreeNodeMetadataInput = {
  icon?: string | null;
  tooltip?: string | null;
  isHidden?: boolean;
};

type MovePageTreeNodeInput = {
  parentId: string | null;
  rank: string;
};

type PageTreeMutationResult = {
  id: string;
  kind: 'group' | 'page';
};

type PageTreeOperationStatus = 'idle' | 'pending' | 'error';

function rankForAppendIndex(index: number): string {
  return String((index + 1) * 1000).padStart(6, '0');
}

function rankForMoveTarget(index: number, direction: -1 | 1): string {
  if (direction < 0) {
    return index === 0 ? '000000' : String(index * 1000 + 500).padStart(6, '0');
  }

  return String((index + 1) * 1000 + 500).padStart(6, '0');
}

function findSiblingContext(
  nodes: FrontStageTreeNode[],
  targetNodeId: string,
  parentId: string | null = null
): {
  parentId: string | null;
  siblings: FrontStageTreeNode[];
  index: number;
} | null {
  const index = nodes.findIndex((node) => node.id === targetNodeId);
  if (index >= 0) {
    return {
      parentId,
      siblings: nodes,
      index
    };
  }

  for (const node of nodes) {
    if (!node.children) {
      continue;
    }

    const childContext = findSiblingContext(
      node.children,
      targetNodeId,
      node.id
    );
    if (childContext) {
      return childContext;
    }
  }

  return null;
}

function extractNodeFromTree(
  nodes: FrontStageTreeNode[],
  targetNodeId: string
): { nodes: FrontStageTreeNode[]; extractedNode: FrontStageTreeNode | null } {
  let extractedNode: FrontStageTreeNode | null = null;
  const nextNodes: FrontStageTreeNode[] = [];

  for (const node of nodes) {
    if (node.id === targetNodeId) {
      extractedNode = node;
      continue;
    }

    if (node.children) {
      const childResult = extractNodeFromTree(node.children, targetNodeId);
      if (childResult.extractedNode) {
        extractedNode = childResult.extractedNode;
        nextNodes.push({
          ...node,
          children: childResult.nodes
        });
        continue;
      }
    }

    nextNodes.push(node);
  }

  return {
    nodes: nextNodes,
    extractedNode
  };
}

function insertNodeIntoTree(
  nodes: FrontStageTreeNode[],
  parentId: string | null,
  index: number,
  nodeToInsert: FrontStageTreeNode
): FrontStageTreeNode[] {
  if (!parentId) {
    const nextNodes = [...nodes];
    nextNodes.splice(index, 0, nodeToInsert);
    return nextNodes;
  }

  return nodes.map((node) => {
    if (node.id === parentId && node.kind === 'group') {
      const nextChildren = [...(node.children ?? [])];
      nextChildren.splice(index, 0, nodeToInsert);
      return {
        ...node,
        children: nextChildren
      };
    }

    return {
      ...node,
      children: node.children
        ? insertNodeIntoTree(node.children, parentId, index, nodeToInsert)
        : node.children
    };
  });
}

function moveNodeToTreePosition(
  nodes: FrontStageTreeNode[],
  nodeId: string,
  targetNodeId: string,
  position: 'before' | 'inside' | 'after'
): FrontStageTreeNode[] {
  const { nodes: nodesWithoutDragged, extractedNode } = extractNodeFromTree(
    nodes,
    nodeId
  );
  if (!extractedNode) {
    return nodes;
  }

  const targetSiblingContext = findSiblingContext(
    nodesWithoutDragged,
    targetNodeId
  );
  if (!targetSiblingContext) {
    return nodes;
  }

  if (extractedNode.kind === 'group' && targetSiblingContext.parentId) {
    return nodes;
  }

  if (position === 'inside') {
    if (extractedNode.kind !== 'page') {
      return nodes;
    }

    const targetNode = findNodeById(nodesWithoutDragged, targetNodeId);
    if (!targetNode || targetNode.kind !== 'group') {
      return nodes;
    }

    return insertNodeIntoTree(
      nodesWithoutDragged,
      targetNodeId,
      targetNode.children?.length ?? 0,
      extractedNode
    );
  }

  const insertIndex =
    position === 'before'
      ? targetSiblingContext.index
      : targetSiblingContext.index + 1;

  return insertNodeIntoTree(
    nodesWithoutDragged,
    targetSiblingContext.parentId,
    insertIndex,
    extractedNode
  );
}

function isNodeDescendantOf(
  nodes: FrontStageTreeNode[],
  ancestorNodeId: string,
  targetNodeId: string
): boolean {
  const ancestorNode = findNodeById(nodes, ancestorNodeId);
  if (!ancestorNode?.children) {
    return false;
  }

  return Boolean(findNodeById(ancestorNode.children, targetNodeId));
}

function updatePageTreeNode(
  nodes: FrontStageTreeNode[],
  targetNodeId: string,
  patch: Partial<
    Pick<
      FrontStageTreeNode,
      'title' | 'icon' | 'tooltip' | 'is_hidden' | 'content_presentation'
    >
  >
): FrontStageTreeNode[] {
  return nodes.map((node) => {
    if (node.id === targetNodeId) {
      return {
        ...node,
        ...patch
      };
    }

    return {
      ...node,
      children: node.children
        ? updatePageTreeNode(node.children, targetNodeId, patch)
        : node.children
    };
  });
}

function getNodeAppendRank(
  nodes: FrontStageTreeNode[],
  parentId: string | null
): string {
  if (!parentId) {
    return rankForAppendIndex(nodes.length);
  }

  const parentNode = findNodeById(nodes, parentId);
  return rankForAppendIndex(parentNode?.children?.length ?? 0);
}

export type {
  CreatePageTreeNodeInput,
  MovePageTreeNodeInput,
  PageTreeMutationResult,
  PageTreeOperationStatus,
  RenamePageTreeNodeInput,
  UpdatePageTreeNodeMetadataInput,
};

export {
  findSiblingContext,
  getNodeAppendRank,
  isNodeDescendantOf,
  moveNodeToTreePosition,
  rankForMoveTarget,
  updatePageTreeNode,
};
