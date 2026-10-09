import {
  Alert,
  ConfigProvider,
  Modal,
  Tree,
  Typography,
  type TreeProps
} from 'antd';
import DragOutlined from '@ant-design/icons/es/icons/DragOutlined';
import FolderOutlined from '@ant-design/icons/es/icons/FolderOutlined';
import FileTextOutlined from '@ant-design/icons/es/icons/FileTextOutlined';
import { useState, type DragEvent } from 'react';
import { i18nText } from '../../../../shared/i18n/text';
import type { MoveFrontstageNodeInput } from '../../api/page-tree';
import { FRONTSTAGE_DESIGN_BLUE } from '../../lib/design-mode-theme';
import { findNodeById, type FrontStageTreeNode } from '../../lib/page-tree';
import { projectNavigationPosition } from '../../lib/navigation-drag/projection';
import {
  resolveNavigationMove,
  type NavigationMoveDraft
} from '../../lib/navigation-drag/move-plan';
import './move-page-modal.css';

const ROOT_KEY = '__frontstage_root__';
const PROJECTION_KEY = '__frontstage_move_projection__';
const DRAG_TYPE = 'application/x-frontstage-move-page';
function getNodeTitle(node: FrontStageTreeNode) {
  return (
    node.title ||
    i18nText(
      'frontstage',
      node.kind === 'group' ? 'auto.unnamed_group' : 'auto.unnamed_page'
    )
  );
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
  onMove: (nodeId: string, input: MoveFrontstageNodeInput) => Promise<boolean>;
  onCancel: () => void;
}) {
  const [expandedKeys, setExpandedKeys] = useState<string[]>([ROOT_KEY]);
  const [draft, setDraft] = useState<NavigationMoveDraft | null>(null);
  const [hoverDraft, setHoverDraft] = useState<NavigationMoveDraft | null>(
    null
  );
  const [isDragging, setIsDragging] = useState(false);
  const [moveError, setMoveError] = useState(false);
  const [isMoving, setIsMoving] = useState(false);
  const projection = hoverDraft ?? draft;
  const pending = isMoving || isOperationPending;
  const moveInput = draft
    ? resolveNavigationMove(pageTree, node.id, draft)
    : null;

  const expandDestination = (targetId: string | null) => {
    const key = targetId ?? ROOT_KEY;
    setExpandedKeys((keys) => (keys.includes(key) ? keys : [...keys, key]));
  };
  const startDrag = (event: DragEvent<HTMLElement>) => {
    if (pending) {
      event.preventDefault();
      return;
    }
    event.stopPropagation();
    event.dataTransfer.setData(DRAG_TYPE, node.id);
    event.dataTransfer.effectAllowed = 'move';
    setIsDragging(true);
  };
  const endDrag = () => {
    setIsDragging(false);
    setHoverDraft(null);
  };
  const dragOver = (
    event: DragEvent<HTMLElement>,
    targetId: string | null,
    isGroup: boolean
  ) => {
    event.stopPropagation();
    const rect = event.currentTarget.getBoundingClientRect();
    const position =
      targetId === null
        ? 'inside'
        : projectNavigationPosition(
            event.clientY,
            rect.top,
            rect.height,
            isGroup
          );
    const next = position ? { targetNodeId: targetId, position } : null;
    if (
      pending ||
      !isDragging ||
      !next ||
      !resolveNavigationMove(pageTree, node.id, next)
    ) {
      event.dataTransfer.dropEffect = 'none';
      setHoverDraft(null);
      return;
    }
    event.preventDefault();
    event.dataTransfer.dropEffect = 'move';
    setHoverDraft(next);
    if (position === 'inside') expandDestination(targetId);
  };
  const drop = (event: DragEvent<HTMLElement>, targetId: string | null) => {
    event.preventDefault();
    event.stopPropagation();
    if (
      !pending &&
      isDragging &&
      hoverDraft?.targetNodeId === targetId &&
      resolveNavigationMove(pageTree, node.id, hoverDraft)
    ) {
      setDraft(hoverDraft);
      setMoveError(false);
    }
    endDrag();
  };
  let projectionRowId = projection?.targetNodeId;
  let projectionIndent = 0;
  if (projection?.position === 'after' && projection.targetNodeId) {
    let tail = findNodeById(pageTree, projection.targetNodeId);
    while (tail && expandedKeys.includes(tail.id) && tail.children?.length) {
      tail = tail.children.at(-1) ?? null;
      projectionIndent += 1;
    }
    projectionRowId = tail?.id;
  }

  const projectionBlock = (
    position: 'before' | 'inside' | 'after',
    targetId: string | null,
    indent = 0
  ) => (
    <div
      className={`move-page-modal__projection move-page-modal__projection--${position}`}
      style={{
        borderColor: FRONTSTAGE_DESIGN_BLUE.primary,
        background: FRONTSTAGE_DESIGN_BLUE.bgSelected,
        insetInlineStart: indent ? -indent * 20 : undefined,
        width: indent ? `calc(100% + ${indent * 20}px)` : undefined
      }}
      data-testid="move-page-projection"
      data-target-id={targetId ?? ROOT_KEY}
      data-position={position}
      draggable={!pending}
      onDragStart={startDrag}
      onDragEnd={endDrag}
      onDragOver={(event) => {
        if (isDragging && !pending) {
          event.preventDefault();
          event.stopPropagation();
        }
      }}
      onDrop={(event) => drop(event, targetId)}
      role="status"
      aria-label={i18nText('frontstage', 'move_projection', {
        page: getNodeTitle(node),
        position:
          position === 'inside'
            ? i18nText('frontstage', 'drag_projection.inside')
            : position === 'before'
              ? i18nText('frontstage', 'drag_projection.before')
              : i18nText('frontstage', 'drag_projection.after')
      })}
    />
  );
  const rowTitle = (
    targetId: string | null,
    title: string,
    isGroup: boolean
  ) => (
    <div
      className="move-page-modal__row"
      data-node-id={targetId ?? ROOT_KEY}
      draggable={targetId === node.id && !pending}
      onDragStart={targetId === node.id ? startDrag : undefined}
      onDragEnd={endDrag}
      onDragOver={(event) => dragOver(event, targetId, isGroup)}
      onDrop={(event) => drop(event, targetId)}
    >
      {title}
      {projectionRowId === targetId &&
      projection &&
      projection.position !== 'inside'
        ? projectionBlock(
            projection.position,
            projection.targetNodeId,
            projection.position === 'after' ? projectionIndent : 0
          )
        : null}
    </div>
  );

  // Materialize only the expanded node's direct children, without recursively
  // constructing collapsed descendants from the cached backend navigation tree.
  const treeNodes = (
    nodes: FrontStageTreeNode[],
    parentId: string | null
  ): NonNullable<TreeProps['treeData']> => {
    const items = nodes.map((item) => ({
      key: item.id,
      title: rowTitle(item.id, getNodeTitle(item), item.kind === 'group'),
      icon: item.kind === 'group' ? <FolderOutlined /> : <FileTextOutlined />,
      isLeaf:
        item.kind !== 'group' ||
        (!item.children?.length &&
          !(
            projection?.targetNodeId === item.id &&
            projection.position === 'inside'
          )),
      selectable: Boolean(
        resolveNavigationMove(pageTree, node.id, {
          targetNodeId: item.id,
          position: item.kind === 'group' ? 'inside' : 'after'
        })
      ),
      children: expandedKeys.includes(item.id)
        ? treeNodes(item.children ?? [], item.id)
        : undefined
    }));
    if (
      projection?.targetNodeId === parentId &&
      projection.position === 'inside'
    ) {
      return [
        ...items,
        {
          key: PROJECTION_KEY,
          title: projectionBlock('inside', parentId),
          selectable: false,
          isLeaf: true
        }
      ];
    }
    return items;
  };

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
        value1: getNodeTitle(node)
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
      <div
        className="move-page-modal__source"
        draggable={!pending}
        onDragStart={startDrag}
        onDragEnd={endDrag}
        aria-label={`${i18nText('frontstage', 'auto.drag_move_node')} ${getNodeTitle(node)}`}
      >
        <DragOutlined /> {getNodeTitle(node)}
      </div>
      {moveError ? (
        <Alert
          type="error"
          showIcon
          title={i18nText('frontstage', 'auto.operation_failed')}
        />
      ) : null}
      <div
        onDragLeave={(event) => {
          if (!event.currentTarget.contains(event.relatedTarget as Node | null))
            setHoverDraft(null);
        }}
      >
        <ConfigProvider theme={{ components: { Tree: { indentSize: 20 } } }}>
          <Tree
            showIcon
            blockNode
            styles={{
              itemTitle: {
                display: 'inline-block',
                verticalAlign: 'top',
                width: 'calc(100% - 24px)'
              }
            }}
            height={320}
            itemHeight={32}
            disabled={pending}
            expandedKeys={expandedKeys}
            autoExpandParent={false}
            onExpand={(keys) => setExpandedKeys(keys.map(String))}
            selectedKeys={draft ? [draft.targetNodeId ?? ROOT_KEY] : []}
            onSelect={(keys) => {
              if (!keys.length) return;
              const targetId = keys[0] === ROOT_KEY ? null : String(keys[0]);
              const target = targetId ? findNodeById(pageTree, targetId) : null;
              const next: NavigationMoveDraft = {
                targetNodeId: targetId,
                position: target?.kind === 'page' ? 'after' : 'inside'
              };
              if (resolveNavigationMove(pageTree, node.id, next)) {
                setDraft(next);
                setMoveError(false);
                if (next.position === 'inside') expandDestination(targetId);
              }
            }}
            treeData={[
              {
                key: ROOT_KEY,
                title: rowTitle(
                  null,
                  i18nText('frontstage', 'auto.not_grouped'),
                  true
                ),
                icon: <FolderOutlined />,
                selectable: true,
                isLeaf: false,
                children: expandedKeys.includes(ROOT_KEY)
                  ? treeNodes(pageTree, null)
                  : undefined
              }
            ]}
          />
        </ConfigProvider>
      </div>
    </Modal>
  );
}
