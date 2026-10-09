import { Alert, Modal, Tree, Typography, type TreeProps } from 'antd';
import FolderOutlined from '@ant-design/icons/es/icons/FolderOutlined';
import FileTextOutlined from '@ant-design/icons/es/icons/FileTextOutlined';
import { useState } from 'react';
import { i18nText } from '../../../../shared/i18n/text';
import type { FrontStageTreeNode } from '../../lib/page-tree';

const ROOT_PAGE_GROUP_VALUE = '__frontstage_root__';
function getNodeTitle(node: FrontStageTreeNode) {
  return (
    node.title ||
    i18nText(
      'frontstage',
      node.kind === 'group' ? 'auto.unnamed_group' : 'auto.unnamed_page'
    )
  );
}

function findParentId(
  nodes: FrontStageTreeNode[],
  targetNodeId: string,
  parentId: string | null = null
): string | null | undefined {
  for (const node of nodes) {
    if (node.id === targetNodeId) {
      return parentId;
    }

    if (node.children && node.children.length > 0) {
      const childParentId = findParentId(node.children, targetNodeId, node.id);
      if (childParentId !== undefined) {
        return childParentId;
      }
    }
  }

  return undefined;
}

export function MovePageModal({
  node,
  pageTree,
  isOperationPending,
  onMove,
  onCancel
}: {
  node: FrontStageTreeNode;
  pageTree: FrontStageTreeNode[];
  isOperationPending: boolean;
  onMove: (
    nodeId: string,
    currentParentId: string | null,
    nextParentId: string | null
  ) => Promise<boolean>;
  onCancel: () => void;
}) {
  const [moveTargetId, setMoveTargetId] = useState<string | null>(null);
  const [moveError, setMoveError] = useState(false);
  const [isMoving, setIsMoving] = useState(false);
  const currentParentId = findParentId(pageTree, node.id) ?? null;
  const currentTargetId = currentParentId ?? ROOT_PAGE_GROUP_VALUE;

  const moveTreeData = (
    nodes: FrontStageTreeNode[]
  ): NonNullable<TreeProps['treeData']> =>
    nodes.map((node) => ({
      key: node.id,
      title: getNodeTitle(node),
      icon: node.kind === 'group' ? <FolderOutlined /> : <FileTextOutlined />,
      selectable: node.kind === 'group' && node.id !== currentParentId,
      children: node.children?.length ? moveTreeData(node.children) : undefined
    }));

  const confirmMovePage = async () => {
    if (
      !moveTargetId ||
      moveTargetId === currentTargetId ||
      isMoving ||
      isOperationPending
    )
      return;
    setIsMoving(true);
    setMoveError(false);
    try {
      const saved = await onMove(
        node.id,
        currentParentId,
        moveTargetId === ROOT_PAGE_GROUP_VALUE ? null : moveTargetId
      );
      if (saved) onCancel();
      else setMoveError(true);
    } catch {
      setMoveError(true);
    } finally {
      setIsMoving(false);
    }
  };

  return (
    <Modal
      open
      title={i18nText('frontstage', 'auto.move_page_to', {
        value1: getNodeTitle(node)
      })}
      okText={i18nText('frontstage', 'auto.confirm')}
      cancelText={i18nText('frontstage', 'auto.cancel')}
      confirmLoading={isMoving}
      okButtonProps={{
        disabled:
          !moveTargetId ||
          moveTargetId === currentTargetId ||
          isOperationPending
      }}
      cancelButtonProps={{ disabled: isMoving }}
      closable={!isMoving}
      mask={{ closable: !isMoving }}
      keyboard={!isMoving}
      onCancel={() => {
        if (!isMoving) onCancel();
      }}
      onOk={() => void confirmMovePage()}
      destroyOnHidden
    >
      <Typography.Paragraph type="secondary">
        {i18nText('frontstage', 'auto.select_destination_group')}
      </Typography.Paragraph>
      {moveError ? (
        <Alert
          type="error"
          showIcon
          title={i18nText('frontstage', 'auto.operation_failed')}
        />
      ) : null}
      <Tree
        key={node.id}
        showIcon
        blockNode
        defaultExpandAll
        height={320}
        disabled={isMoving || isOperationPending}
        selectedKeys={moveTargetId ? [moveTargetId] : []}
        onSelect={(keys) =>
          setMoveTargetId(keys.length ? String(keys[0]) : null)
        }
        treeData={[
          {
            key: ROOT_PAGE_GROUP_VALUE,
            title: i18nText('frontstage', 'auto.not_grouped'),
            icon: <FolderOutlined />,
            selectable: currentParentId !== null,
            children: moveTreeData(pageTree)
          }
        ]}
      />
    </Modal>
  );
}
