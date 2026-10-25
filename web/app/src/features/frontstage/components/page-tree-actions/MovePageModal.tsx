import { Alert, Modal, Typography } from 'antd';
import { useState } from 'react';
import { i18nText } from '../../../../shared/i18n/text';
import type { MoveFrontstageNodeInput } from '../../api/page-tree';
import type { FrontStageTreeNode } from '../../lib/page-tree';
import {
  resolveNavigationMove,
  type NavigationMoveDraft
} from '../../lib/navigation-drag/move-plan';
import { NavigationMoveTree } from './NavigationMoveTree';

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
  onMove: (nodeId: string, input: MoveFrontstageNodeInput) => Promise<boolean>;
  onCancel: () => void;
}) {
  const [draft, setDraft] = useState<NavigationMoveDraft | null>(null);
  const moveInput = draft
    ? resolveNavigationMove(pageTree, node.id, draft)
    : null;
  const [isDragging, setIsDragging] = useState(false);
  const [isMoving, setIsMoving] = useState(false);
  const [moveError, setMoveError] = useState(false);
  const pending = isMoving || isOperationPending;
  const confirmMove = async () => {
    if (!moveInput || pending || isDragging) return;
    setIsMoving(true);
    setMoveError(false);
    try {
      if (await onMove(node.id, moveInput)) onCancel();
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
        value1:
          node.title ||
          i18nText(
            'frontstage',
            node.kind === 'group' ? 'auto.unnamed_group' : 'auto.unnamed_page'
          )
      })}
      okText={i18nText('frontstage', 'auto.confirm')}
      cancelText={i18nText('frontstage', 'auto.cancel')}
      confirmLoading={isMoving}
      okButtonProps={{ disabled: !moveInput || pending || isDragging }}
      cancelButtonProps={{ disabled: isMoving }}
      closable={!isMoving}
      mask={{ closable: !isMoving }}
      keyboard={!isMoving}
      onCancel={() => {
        if (!isMoving) onCancel();
      }}
      onOk={() => void confirmMove()}
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
      <NavigationMoveTree
        sourceNode={node}
        pageTree={pageTree}
        disabled={pending}
        onDraggingChange={setIsDragging}
        onSelectMove={(_, _input, nextDraft) => {
          setDraft(nextDraft);
          setMoveError(false);
        }}
      />
    </Modal>
  );
}
