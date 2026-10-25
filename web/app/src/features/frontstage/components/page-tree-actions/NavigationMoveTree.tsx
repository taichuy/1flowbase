import { ConfigProvider, Tree, type TreeProps } from 'antd';
import DragOutlined from '@ant-design/icons/es/icons/DragOutlined';
import HolderOutlined from '@ant-design/icons/es/icons/HolderOutlined';
import FolderTwoTone from '@ant-design/icons/es/icons/FolderTwoTone';
import FolderOutlined from '@ant-design/icons/es/icons/FolderOutlined';
import FileTextOutlined from '@ant-design/icons/es/icons/FileTextOutlined';
import {
  useEffect,
  useRef,
  useState,
  type ReactNode,
  type DragEvent
} from 'react';
import { i18nText } from '../../../../shared/i18n/text';
import type { MoveFrontstageNodeInput } from '../../api/page-tree';
import { FRONTSTAGE_DESIGN_BLUE } from '../../lib/design-mode-theme';
import { findNodeById, type FrontStageTreeNode } from '../../lib/page-tree';
import { projectNavigationPosition } from '../../lib/navigation-drag/projection';
import {
  resolveNavigationMove,
  type NavigationMoveDraft
} from '../../lib/navigation-drag/move-plan';
import { PageTreeIcon } from '../../lib/page-tree-icons/registry';
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

export function NavigationMoveTree({
  sourceNode,
  pageTree,
  disabled = false,
  onSelectMove,
  onDraggingChange,
  height = 320,
  management = false,
  locateNodeId,
  renderActions
}: {
  sourceNode?: FrontStageTreeNode;
  pageTree: FrontStageTreeNode[];
  disabled?: boolean;
  onSelectMove: (
    nodeId: string,
    input: MoveFrontstageNodeInput,
    draft: NavigationMoveDraft
  ) => void;
  onDraggingChange?: (dragging: boolean) => void;
  height?: number;
  management?: boolean;
  locateNodeId?: string;
  renderActions?: (node: FrontStageTreeNode | null) => ReactNode;
}) {
  const treeRef = useRef<React.ComponentRef<typeof Tree>>(null);
  const [dragSource, setDragSource] = useState<FrontStageTreeNode | null>(null);
  const node = sourceNode ?? dragSource;
  const [expandedKeys, setExpandedKeys] = useState<string[]>(() => [
    ROOT_KEY,
    ...(management
      ? pageTree.filter((item) => item.kind === 'group').map((item) => item.id)
      : [])
  ]);
  useEffect(() => {
    if (!locateNodeId) return;
    const ancestors: string[] = [ROOT_KEY];
    const visit = (nodes: FrontStageTreeNode[]): boolean =>
      nodes.some((item) => {
        if (item.id === locateNodeId) return true;
        if (visit(item.children ?? [])) {
          ancestors.push(item.id);
          return true;
        }
        return false;
      });
    if (visit(pageTree))
      setExpandedKeys((keys) => [...new Set([...keys, ...ancestors])]);
  }, [locateNodeId, pageTree]);
  useEffect(() => {
    if (locateNodeId)
      treeRef.current?.scrollTo({ key: locateNodeId, align: 'auto' });
  }, [locateNodeId, expandedKeys]);
  const [draft, setDraft] = useState<NavigationMoveDraft | null>(null);
  const [hoverDraft, setHoverDraft] = useState<NavigationMoveDraft | null>(
    null
  );
  const [isDragging, setIsDragging] = useState(false);
  const projection = hoverDraft ?? draft;
  const pending = disabled;
  const selectMove = (next: NavigationMoveDraft) => {
    if (!node || pending) return;
    const input = resolveNavigationMove(pageTree, node.id, next);
    if (!input) return;
    if (sourceNode) setDraft(next);
    onSelectMove(node.id, input, next);
  };

  const expandDestination = (targetId: string | null) => {
    const key = targetId ?? ROOT_KEY;
    setExpandedKeys((keys) => (keys.includes(key) ? keys : [...keys, key]));
  };
  const startDrag = (event: DragEvent<HTMLElement>, source = node) => {
    if (pending || !source) {
      event.preventDefault();
      return;
    }
    event.stopPropagation();
    event.dataTransfer.setData(DRAG_TYPE, source.id);
    setDragSource(source);
    event.dataTransfer.effectAllowed = 'move';
    setIsDragging(true);
    onDraggingChange?.(true);
  };
  const endDrag = () => {
    setIsDragging(false);
    onDraggingChange?.(false);
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
      !node ||
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
      node &&
      isDragging &&
      hoverDraft?.targetNodeId === targetId &&
      resolveNavigationMove(pageTree, node.id, hoverDraft)
    ) {
      selectMove(hoverDraft);
    }
    endDrag();
  };
  const projectionBlock = (
    position: 'before' | 'inside' | 'after',
    targetId: string | null
  ) => (
    <div
      className={`move-page-modal__projection move-page-modal__projection--${position}`}
      style={{
        borderColor: FRONTSTAGE_DESIGN_BLUE.primary,
        background: FRONTSTAGE_DESIGN_BLUE.bgSelected
      }}
      data-testid="move-page-projection"
      data-target-id={targetId ?? ROOT_KEY}
      data-position={position}
      draggable={!pending}
      onDragStart={(event) => startDrag(event)}
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
        page: node ? getNodeTitle(node) : '',
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
    isGroup: boolean,
    item?: FrontStageTreeNode
  ) => (
    <div
      className={`move-page-modal__row${management ? ' navigation-management-tree__row' : ''}`}
      data-kind={isGroup ? 'group' : 'page'}
      data-located={targetId === locateNodeId || undefined}
      data-node-id={targetId ?? ROOT_KEY}
      draggable={Boolean(
        targetId && (!sourceNode || targetId === sourceNode.id) && !pending
      )}
      onDragStart={(event) => {
        const source = targetId ? findNodeById(pageTree, targetId) : null;
        if (!sourceNode || source?.id === sourceNode.id)
          startDrag(event, source);
      }}
      onDragEnd={endDrag}
      onDragOver={(event) => dragOver(event, targetId, isGroup)}
      onDrop={(event) => drop(event, targetId)}
    >
      {management ? (
        <span className="navigation-management-tree__handle" aria-hidden>
          {targetId ? <HolderOutlined /> : null}
        </span>
      ) : null}
      {management ? (
        <span className="navigation-management-tree__node-icon">
          <PageTreeIcon
            name={item?.icon}
            fallback={
              isGroup ? (
                <FolderTwoTone
                  twoToneColor={[
                    'var(--color-primary)',
                    'color-mix(in srgb, var(--color-primary) 25%, white)'
                  ]}
                />
              ) : (
                <FileTextOutlined />
              )
            }
          />
        </span>
      ) : null}
      <span
        className={management ? 'navigation-management-tree__title' : undefined}
      >
        {title}
      </span>
      {renderActions ? (
        <span
          className="navigation-management-tree__actions"
          draggable={false}
          onDragStart={(event) => {
            event.preventDefault();
            event.stopPropagation();
          }}
          onClick={(event) => event.stopPropagation()}
        >
          {renderActions(item ?? null)}
        </span>
      ) : null}
    </div>
  );

  // Materialize only the expanded node's direct children, without recursively
  // constructing collapsed descendants from the cached backend navigation tree.
  const expandedKeySet = new Set(expandedKeys);
  const treeNodes = (
    nodes: FrontStageTreeNode[],
    parentId: string | null
  ): NonNullable<TreeProps['treeData']> => {
    const items: NonNullable<TreeProps['treeData']> = [];
    const placeholder = (
      position: NavigationMoveDraft['position'],
      targetId: string | null
    ) => ({
      key: PROJECTION_KEY,
      title: projectionBlock(position, targetId),
      selectable: false,
      isLeaf: true
    });
    for (const item of nodes) {
      const isTarget = projection?.targetNodeId === item.id;
      if (isTarget && projection.position === 'before') {
        items.push(placeholder('before', item.id));
      }
      items.push({
        key: item.id,
        title: rowTitle(
          item.id,
          getNodeTitle(item),
          item.kind === 'group',
          item
        ),
        icon: (
          <PageTreeIcon
            name={item.icon}
            fallback={
              item.kind === 'group' ? (
                <FolderOutlined
                  className={
                    management
                      ? 'navigation-management-tree__folder'
                      : undefined
                  }
                />
              ) : (
                <FileTextOutlined />
              )
            }
          />
        ),
        isLeaf:
          item.kind !== 'group' ||
          (!item.children?.length &&
            !(
              projection?.targetNodeId === item.id &&
              projection.position === 'inside'
            )),
        selectable:
          !sourceNode ||
          Boolean(
            sourceNode &&
            resolveNavigationMove(pageTree, sourceNode.id, {
              targetNodeId: item.id,
              position: item.kind === 'group' ? 'inside' : 'after'
            })
          ),
        children: expandedKeySet.has(item.id)
          ? treeNodes(item.children ?? [], item.id)
          : undefined
      });
      if (isTarget && projection.position === 'after') {
        items.push(placeholder('after', item.id));
      }
    }
    if (
      projection?.targetNodeId === parentId &&
      projection.position === 'inside'
    ) {
      items.push(placeholder('inside', parentId));
    }
    return items;
  };

  return (
    <>
      {sourceNode ? (
        <div
          className="move-page-modal__source"
          draggable={!pending}
          onDragStart={(event) => startDrag(event)}
          onDragEnd={endDrag}
          aria-label={`${i18nText('frontstage', 'auto.drag_move_node')} ${getNodeTitle(sourceNode)}`}
        >
          <DragOutlined /> {getNodeTitle(sourceNode)}
        </div>
      ) : null}
      <div
        className={management ? 'navigation-management-tree' : undefined}
        onDragLeave={(event) => {
          if (!event.currentTarget.contains(event.relatedTarget as Node | null))
            setHoverDraft(null);
        }}
      >
        <ConfigProvider
          theme={{
            components: {
              Tree: {
                indentSize: management ? 28 : 20,
                ...(management
                  ? {
                      titleHeight: 36,
                      nodeHoverBg: 'var(--bg-hover)',
                      nodeSelectedBg: 'var(--bg-selected)'
                    }
                  : {})
              }
            }
          }}
        >
          <Tree
            ref={treeRef}
            classNames={
              management
                ? {
                    item: 'navigation-management-tree__item',
                    itemIcon: 'navigation-management-tree__icon',
                    itemSwitcher: 'navigation-management-tree__switcher'
                  }
                : undefined
            }
            showIcon={!management}
            blockNode
            styles={{
              ...(management
                ? {
                    root: { fontSize: 16 },
                    item: {
                      paddingBottom: 0,
                      marginBottom: 0,
                      paddingInlineStart: 'var(--route-tree-gutter)'
                    }
                  }
                : {}),
              itemTitle: {
                display: 'inline-block',
                verticalAlign: 'top',
                width: management ? '100%' : 'calc(100% - 24px)'
              }
            }}
            height={height}
            itemHeight={management ? 36 : 32}
            disabled={pending}
            expandedKeys={expandedKeys}
            autoExpandParent={false}
            onExpand={(keys) => setExpandedKeys(keys.map(String))}
            selectedKeys={
              draft
                ? [draft.targetNodeId ?? ROOT_KEY]
                : locateNodeId
                  ? [locateNodeId]
                  : []
            }
            onSelect={(_, info) => {
              const targetId =
                info.node.key === ROOT_KEY ? null : String(info.node.key);
              const target = targetId ? findNodeById(pageTree, targetId) : null;
              const next: NavigationMoveDraft = {
                targetNodeId: targetId,
                position: target?.kind === 'page' ? 'after' : 'inside'
              };
              if (
                sourceNode &&
                !pending &&
                resolveNavigationMove(pageTree, sourceNode.id, next)
              ) {
                selectMove(next);
                if (next.position === 'inside') expandDestination(targetId);
              } else if (!sourceNode && target?.kind === 'group') {
                setExpandedKeys((keys) =>
                  keys.includes(target.id)
                    ? keys.filter((key) => key !== target.id)
                    : [...keys, target.id]
                );
              }
            }}
            treeData={[
              {
                key: ROOT_KEY,
                title: rowTitle(
                  null,
                  i18nText('frontstage', 'move_topbar_root'),
                  true
                ),
                icon: (
                  <FolderOutlined
                    className={
                      management
                        ? 'navigation-management-tree__folder'
                        : undefined
                    }
                  />
                ),
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
    </>
  );
}
