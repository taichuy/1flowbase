import * as AntIcons from '@ant-design/icons';
import { Button, Typography, Dropdown, Tooltip, Switch } from 'antd';
import { useState } from 'react';
import type { ElementType } from 'react';
import type { DragEvent, FocusEvent } from 'react';
import type { MenuProps } from 'antd';

import {
  canMoveNode,
  findNodeById,
  type FrontStageTreeNode
} from '../lib/page-tree';
import { FrontstageNodeActionButton } from './FrontstageNodeActionButton';
import './frontstage-page-tree-sidebar.css';
import './frontstage-add-action.css';
import { i18nText } from '../../../shared/i18n/text';

type FrontStagePageTreeSidebarProps = {
  pageTree: FrontStageTreeNode[];
  selectedPageId: string | null;
  canEdit: boolean;
  isOperationPending: boolean;
  onAddGroup: () => void;
  onAddPage: () => void;
  onAddPageInGroup: (groupId: string) => void;
  onAddNodeAtPosition?: (
    kind: 'page' | 'group',
    targetNodeId: string,
    position: 'before' | 'after'
  ) => void;
  onRenameNode: (node: FrontStageTreeNode) => void;
  onUpdateNodeMetadata: (
    nodeId: string,
    input: { tooltip?: string | null; isHidden?: boolean }
  ) => void;
  onEditNodeTooltip: (nodeId: string, currentTooltip: string | null) => void;
  onMoveNode: (nodeId: string, direction: -1 | 1) => void;
  onMoveNodeToPosition: (
    nodeId: string,
    targetNodeId: string,
    position: 'before' | 'inside' | 'after'
  ) => void;
  onMovePageToGroup?: (
    nodeId: string,
    currentParentId: string | null,
    nextParentId: string | null
  ) => void;
  onDeleteNode: (nodeId: string) => void;
  onSelectPage: (nodeId: string) => void;
};

const PAGE_TREE_DRAG_DATA_TYPE = 'application/x-frontstage-page-tree-node';
const ROOT_PAGE_GROUP_VALUE = '__frontstage_root__';

type MenuClickInfo = Parameters<NonNullable<MenuProps['onClick']>>[0];

type AntIconComponent = ElementType<{ className?: string }>;
type PageTreeDropIndicator = {
  targetNodeId: string;
  position: 'before' | 'inside' | 'after';
};

const antIconComponents = AntIcons as Record<string, unknown>;
const pageTreeIconMap = Object.fromEntries(
  Object.entries(antIconComponents).filter(
    (entry): entry is [string, AntIconComponent] =>
      /(?:Outlined|Filled|TwoTone)$/.test(entry[0]) &&
      (typeof entry[1] === 'function' ||
        (typeof entry[1] === 'object' && entry[1] !== null))
  )
);
const {
  ArrowDownOutlined,
  ArrowUpOutlined,
  DeleteOutlined,
  DownOutlined,
  DragOutlined,
  EditOutlined,
  EyeInvisibleOutlined,
  EyeOutlined,
  FileAddOutlined,
  FileTextOutlined,
  FolderAddOutlined,
  FolderOutlined,
  InfoCircleOutlined,
  MenuOutlined,
  PlusOutlined,
  RightOutlined
} = pageTreeIconMap as Record<string, AntIconComponent>;

function getNodeTitle(node: FrontStageTreeNode) {
  if (node.title) {
    return node.title;
  }

  return node.kind === 'group'
    ? i18nText('frontstage', 'auto.unnamed_group')
    : i18nText('frontstage', 'auto.unnamed_page');
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

function renderNodeIcon(node: FrontStageTreeNode) {
  if (!node.icon) {
    return null;
  }

  const IconComponent =
    pageTreeIconMap[node.icon as keyof typeof pageTreeIconMap];

  if (!IconComponent) {
    return null;
  }

  return <IconComponent />;
}

function renderTreeNode({
  node,
  pageTree,
  level,
  siblings,
  selectedPageId,
  canEdit,
  isOperationPending,
  collapsedGroupIds,
  toggleGroupCollapse,
  onUpdateNodeMetadata,
  onEditNodeTooltip,
  onAddPageInGroup,
  onAddNodeAtPosition,
  onRenameNode,
  onMoveNode,
  onMoveNodeToPosition,
  onMovePageToGroup,
  onDeleteNode,
  onSelectPage,
  draggedNodeId,
  setDraggedNodeId,
  dropIndicator,
  setDropIndicator
}: {
  node: FrontStageTreeNode;
  pageTree: FrontStageTreeNode[];
  level: number;
  siblings: FrontStageTreeNode[];
  selectedPageId: string | null;
  canEdit: boolean;
  isOperationPending: boolean;
  collapsedGroupIds: Set<string>;
  toggleGroupCollapse: (groupId: string) => void;
  onUpdateNodeMetadata: (
    nodeId: string,
    input: { tooltip?: string | null; isHidden?: boolean }
  ) => void;
  onEditNodeTooltip: (nodeId: string, currentTooltip: string | null) => void;
  onAddPageInGroup: (groupId: string) => void;
  onAddNodeAtPosition?: (
    kind: 'page' | 'group',
    targetNodeId: string,
    position: 'before' | 'after'
  ) => void;
  onRenameNode: (node: FrontStageTreeNode) => void;
  onMoveNode: (nodeId: string, direction: -1 | 1) => void;
  onMoveNodeToPosition: (
    nodeId: string,
    targetNodeId: string,
    position: 'before' | 'inside' | 'after'
  ) => void;
  onMovePageToGroup?: (
    nodeId: string,
    currentParentId: string | null,
    nextParentId: string | null
  ) => void;
  onDeleteNode: (nodeId: string) => void;
  onSelectPage: (nodeId: string) => void;
  draggedNodeId: string | null;
  setDraggedNodeId: (nodeId: string | null) => void;
  dropIndicator: PageTreeDropIndicator | null;
  setDropIndicator: (indicator: PageTreeDropIndicator | null) => void;
}) {
  const isPageNode = node.kind === 'page';
  const isSelected = selectedPageId === node.id;
  const canAddPageToGroup = node.kind === 'group' && level === 0;
  const isCollapsed = collapsedGroupIds.has(node.id);
  const isHidden = Boolean(node.is_hidden);
  const tooltipText = node.tooltip ?? '';
  const { canMoveUp, canMoveDown } = canMoveNode(siblings, node.id);
  const childNodes = node.children ?? [];
  const title = getNodeTitle(node);
  const isDragging = draggedNodeId === node.id;
  const isInsideDropTarget =
    dropIndicator?.targetNodeId === node.id &&
    dropIndicator.position === 'inside';
  const topLevelGroups = pageTree.filter(
    (candidate) => candidate.kind === 'group'
  );
  const currentParentId = findParentId(pageTree, node.id) ?? null;
  const canShowPageGroupSelect = Boolean(
    isPageNode && isSelected && onMovePageToGroup && topLevelGroups.length > 0
  );
  const pageGroupOptions = [
    {
      label: i18nText('frontstage', 'auto.not_grouped'),
      value: ROOT_PAGE_GROUP_VALUE
    },
    ...topLevelGroups.map((groupNode) => ({
      label: groupNode.title || i18nText('frontstage', 'auto.unnamed_group'),
      value: groupNode.id
    }))
  ];
  const pageGroupMenuItems: NonNullable<MenuProps['items']> =
    canShowPageGroupSelect && onMovePageToGroup
      ? [
          { type: 'divider' as const },
          ...pageGroupOptions.map((option) => {
            const optionParentId =
              option.value === ROOT_PAGE_GROUP_VALUE ? null : option.value;

            return {
              key: `move-group-${option.value}`,
              label: option.label,
              disabled:
                optionParentId === currentParentId || isOperationPending,
              onClick: ({ domEvent }: MenuClickInfo) => {
                domEvent.stopPropagation();
                onMovePageToGroup(node.id, currentParentId, optionParentId);
              }
            };
          })
        ]
      : [];
  const getDraggedNodeIdFromEvent = (event: DragEvent<HTMLElement>) =>
    draggedNodeId || event.dataTransfer.getData(PAGE_TREE_DRAG_DATA_TYPE);

  const resolveDropPosition = (
    event: DragEvent<HTMLElement>,
    forcedPosition?: 'before' | 'inside' | 'after'
  ): 'before' | 'inside' | 'after' => {
    if (forcedPosition) {
      return forcedPosition;
    }

    const rect = event.currentTarget.getBoundingClientRect();
    const yRatio = (event.clientY - rect.top) / rect.height;
    const activeDraggedNodeId = getDraggedNodeIdFromEvent(event);
    const activeDraggedNode = activeDraggedNodeId
      ? findNodeById(pageTree, activeDraggedNodeId)
      : null;
    const canDropInsideCurrentGroup =
      node.kind === 'group' &&
      level === 0 &&
      activeDraggedNode?.kind === 'page';

    if (
      canDropInsideCurrentGroup &&
      (!Number.isFinite(yRatio) || (yRatio > 0.28 && yRatio < 0.72))
    ) {
      return 'inside';
    }

    return event.clientY <= rect.top + rect.height / 2 ? 'before' : 'after';
  };

  const updateDropIndicator = (
    event: DragEvent<HTMLElement>,
    forcedPosition?: 'before' | 'inside' | 'after'
  ) => {
    const activeDraggedNodeId = getDraggedNodeIdFromEvent(event);
    if (!canEdit || !activeDraggedNodeId || activeDraggedNodeId === node.id) {
      return;
    }

    event.preventDefault();
    event.dataTransfer.dropEffect = 'move';

    const position = resolveDropPosition(event, forcedPosition);

    setDropIndicator({
      targetNodeId: node.id,
      position
    });
  };

  const handleDrop = (
    event: DragEvent<HTMLElement>,
    forcedPosition?: 'before' | 'inside' | 'after'
  ) => {
    if (!canEdit) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();

    const droppedNodeId =
      event.dataTransfer.getData(PAGE_TREE_DRAG_DATA_TYPE) || draggedNodeId;
    setDraggedNodeId(null);
    setDropIndicator(null);

    if (!droppedNodeId || droppedNodeId === node.id) {
      return;
    }

    const position =
      forcedPosition ??
      (dropIndicator?.targetNodeId === node.id
        ? dropIndicator.position
        : resolveDropPosition(event));

    onMoveNodeToPosition(droppedNodeId, node.id, position);
  };

  const menuItems: MenuProps['items'] = [
    {
      key: 'rename',
      label: i18nText('frontstage', 'auto.edit'),
      icon: <EditOutlined />,
      onClick: ({ domEvent }: MenuClickInfo) => {
        domEvent.stopPropagation();
        onRenameNode(node);
      }
    },
    {
      key: 'tooltip',
      label: i18nText('frontstage', 'auto.edit_description'),
      icon: <InfoCircleOutlined />,
      onClick: ({ domEvent }: MenuClickInfo) => {
        domEvent.stopPropagation();
        onEditNodeTooltip(node.id, node.tooltip ?? null);
      }
    },
    {
      key: 'hide',
      label: (
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            gap: '8px',
            minWidth: '110px'
          }}
        >
          <span>{i18nText('frontstage', 'auto.hide')}</span>
          <Switch
            size="small"
            checked={isHidden}
            onChange={(checked, e) => {
              e.stopPropagation();
              onUpdateNodeMetadata(node.id, { isHidden: checked });
            }}
          />
        </div>
      ),
      icon: isHidden ? <EyeOutlined /> : <EyeInvisibleOutlined />
    },
    {
      key: 'move-to',
      label: i18nText('frontstage', 'auto.move_to'),
      icon: <DragOutlined />,
      children: [
        {
          key: 'move-up',
          label: i18nText('frontstage', 'auto.move_up'),
          icon: <ArrowUpOutlined />,
          disabled: !canMoveUp,
          onClick: ({ domEvent }: MenuClickInfo) => {
            domEvent.stopPropagation();
            onMoveNode(node.id, -1);
          }
        },
        {
          key: 'move-down',
          label: i18nText('frontstage', 'auto.move_down'),
          icon: <ArrowDownOutlined />,
          disabled: !canMoveDown,
          onClick: ({ domEvent }: MenuClickInfo) => {
            domEvent.stopPropagation();
            onMoveNode(node.id, 1);
          }
        },
        ...pageGroupMenuItems
      ]
    },
    {
      key: 'insert-before',
      label: i18nText('frontstage', 'auto.insert_before'),
      icon: <PlusOutlined />,
      children: [
        {
          key: 'insert-before-page',
          label: i18nText('frontstage', 'auto.page'),
          icon: <FileTextOutlined />,
          onClick: ({ domEvent }: MenuClickInfo) => {
            domEvent.stopPropagation();
            onAddNodeAtPosition?.('page', node.id, 'before');
          }
        },
        {
          key: 'insert-before-group',
          label: i18nText('frontstage', 'auto.group'),
          icon: <FolderOutlined />,
          onClick: ({ domEvent }: MenuClickInfo) => {
            domEvent.stopPropagation();
            onAddNodeAtPosition?.('group', node.id, 'before');
          }
        }
      ]
    },
    ...(canAddPageToGroup
      ? [
          {
            key: 'insert-inside',
            label: i18nText('frontstage', 'auto.insert_inside'),
            icon: <FileAddOutlined />,
            disabled: isOperationPending,
            onClick: ({ domEvent }: MenuClickInfo) => {
              domEvent.stopPropagation();
              onAddPageInGroup(node.id);
            }
          }
        ]
      : []),
    {
      key: 'insert-after',
      label: i18nText('frontstage', 'auto.insert_after'),
      icon: <PlusOutlined />,
      children: [
        {
          key: 'insert-after-page',
          label: i18nText('frontstage', 'auto.page'),
          icon: <FileTextOutlined />,
          onClick: ({ domEvent }: MenuClickInfo) => {
            domEvent.stopPropagation();
            onAddNodeAtPosition?.('page', node.id, 'after');
          }
        },
        {
          key: 'insert-after-group',
          label: i18nText('frontstage', 'auto.group'),
          icon: <FolderOutlined />,
          onClick: ({ domEvent }: MenuClickInfo) => {
            domEvent.stopPropagation();
            onAddNodeAtPosition?.('group', node.id, 'after');
          }
        }
      ]
    },
    {
      type: 'divider' as const
    },
    {
      key: 'delete',
      label: i18nText('frontstage', 'auto.delete'),
      icon: <DeleteOutlined />,
      danger: true,
      onClick: ({ domEvent }: MenuClickInfo) => {
        domEvent.stopPropagation();
        onDeleteNode(node.id);
      }
    }
  ];

  const nodeContent = (
    <div className="frontstage-page-tree-sidebar__node-main">
      {node.kind === 'group' && (
        <span
          className="frontstage-page-tree-sidebar__chevron"
          onClick={(e) => {
            e.stopPropagation();
            toggleGroupCollapse(node.id);
          }}
        >
          {isCollapsed ? <RightOutlined /> : <DownOutlined />}
        </span>
      )}
      {renderNodeIcon(node)}
      <span className="frontstage-page-tree-sidebar__node-copy">
        <Typography.Text
          className="frontstage-page-tree-sidebar__node-title"
          ellipsis
        >
          {title}
        </Typography.Text>
        <Typography.Text
          className="frontstage-page-tree-sidebar__node-kind"
          type="secondary"
        >
          {node.kind === 'group'
            ? i18nText('frontstage', 'auto.group_node')
            : i18nText('frontstage', 'auto.page_node')}
        </Typography.Text>
      </span>
      {isHidden && (
        <EyeInvisibleOutlined
          style={{
            fontSize: '12px',
            marginLeft: '4px',
            opacity: 0.7,
            color: '#ff4d4f'
          }}
        />
      )}
    </div>
  );

  return (
    <li
      key={node.id}
      className="frontstage-page-tree-sidebar__node"
      data-testid={`frontstage-tree-node-${node.kind}-${node.title || node.id}`}
      onDragOver={(event) => updateDropIndicator(event)}
      onDrop={handleDrop}
      onClick={(e) => {
        e.stopPropagation();
        if (isPageNode) {
          onSelectPage(node.id);
        } else {
          toggleGroupCollapse(node.id);
        }
      }}
      role={isPageNode ? 'button' : undefined}
      tabIndex={isPageNode ? 0 : -1}
      onKeyDown={(event) => {
        if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault();
          if (isPageNode) {
            onSelectPage(node.id);
          } else {
            toggleGroupCollapse(node.id);
          }
        }
      }}
    >
      {dropIndicator?.targetNodeId === node.id &&
      dropIndicator.position === 'before' ? (
        <div
          className="frontstage-page-tree-sidebar__drop-placeholder"
          onDragOver={(event) => updateDropIndicator(event, 'before')}
          onDrop={(event) => handleDrop(event, 'before')}
        />
      ) : null}
      <div
        className={[
          'frontstage-page-tree-sidebar__node-row',
          isSelected
            ? 'frontstage-page-tree-sidebar__node-row--selected'
            : null,
          isPageNode ? 'frontstage-page-tree-sidebar__node-row--page' : null,
          isHidden ? 'frontstage-page-tree-sidebar__node-row--hidden' : null,
          isDragging
            ? 'frontstage-page-tree-sidebar__node-row--dragging'
            : null,
          canEdit
            ? 'frontstage-page-tree-sidebar__node-row--design'
            : 'frontstage-page-tree-sidebar__node-row--view'
        ]
          .filter(Boolean)
          .join(' ')}
        style={{ paddingLeft: 8 + level * 16 }}
        onDragOver={(event) => updateDropIndicator(event)}
        onDrop={handleDrop}
      >
        {tooltipText ? (
          <Tooltip title={tooltipText}>{nodeContent}</Tooltip>
        ) : (
          nodeContent
        )}
        {canEdit ? (
          <>
            <div
              className="frontstage-page-tree-sidebar__node-actions-visible"
              onClick={(e) => e.stopPropagation()}
            >
              <Tooltip title={i18nText('frontstage', 'auto.drag_sort_hint')}>
                <FrontstageNodeActionButton
                  aria-label={i18nText('frontstage', 'auto.drag_move_node')}
                  disabled={isOperationPending}
                  draggable={!isOperationPending}
                  icon={<DragOutlined />}
                  onDragEnd={(event) => {
                    event.stopPropagation();
                    setDraggedNodeId(null);
                    setDropIndicator(null);
                  }}
                  onDragStart={(event) => {
                    event.stopPropagation();
                    event.dataTransfer.effectAllowed = 'move';
                    event.dataTransfer.setData(
                      PAGE_TREE_DRAG_DATA_TYPE,
                      node.id
                    );
                    setDraggedNodeId(node.id);
                  }}
                  onClick={(event) => {
                    event.stopPropagation();
                  }}
                />
              </Tooltip>
              <Dropdown
                menu={{ items: menuItems }}
                trigger={['click']}
                placement="bottomRight"
              >
                <FrontstageNodeActionButton
                  aria-label={i18nText('frontstage', 'auto.page_action_menu')}
                  disabled={isOperationPending}
                  icon={<MenuOutlined />}
                  onClick={(event) => {
                    event.stopPropagation();
                  }}
                />
              </Dropdown>
            </div>
          </>
        ) : null}
      </div>
      {(!isCollapsed || isInsideDropTarget) &&
      (childNodes.length > 0 ||
        (canEdit && node.kind === 'group' && level === 0) ||
        isInsideDropTarget) ? (
        <ul className="frontstage-page-tree-sidebar__children">
          {childNodes.map((childNode) =>
            renderTreeNode({
              node: childNode,
              pageTree,
              level: level + 1,
              siblings: childNodes,
              selectedPageId,
              canEdit,
              isOperationPending,
              collapsedGroupIds,
              toggleGroupCollapse,
              onUpdateNodeMetadata,
              onEditNodeTooltip,
              onAddPageInGroup,
              onAddNodeAtPosition,
              onRenameNode,
              onMoveNode,
              onMoveNodeToPosition,
              onMovePageToGroup,
              onDeleteNode,
              onSelectPage,
              draggedNodeId,
              setDraggedNodeId,
              dropIndicator,
              setDropIndicator
            })
          )}
          {isInsideDropTarget ? (
            <li className="frontstage-page-tree-sidebar__drop-placeholder-item">
              <div
                className="frontstage-page-tree-sidebar__drop-placeholder frontstage-page-tree-sidebar__drop-placeholder--inside"
                onDragOver={(event) => updateDropIndicator(event, 'inside')}
                onDrop={(event) => handleDrop(event, 'inside')}
              />
            </li>
          ) : null}
        </ul>
      ) : null}
      {dropIndicator?.targetNodeId === node.id &&
      dropIndicator.position === 'after' ? (
        <div
          className="frontstage-page-tree-sidebar__drop-placeholder"
          onDragOver={(event) => updateDropIndicator(event, 'after')}
          onDrop={(event) => handleDrop(event, 'after')}
        />
      ) : null}
    </li>
  );
}

export function FrontStagePageTreeSidebar({
  pageTree,
  selectedPageId,
  canEdit,
  isOperationPending,
  onAddGroup,
  onAddPage,
  onAddPageInGroup,
  onAddNodeAtPosition,
  onRenameNode,
  onUpdateNodeMetadata,
  onEditNodeTooltip,
  onMoveNode,
  onMoveNodeToPosition,
  onMovePageToGroup,
  onDeleteNode,
  onSelectPage
}: FrontStagePageTreeSidebarProps) {
  const [collapsedGroupIds, setCollapsedGroupIds] = useState<Set<string>>(
    () => {
      try {
        const stored = localStorage.getItem('frontstage_collapsed_groups');
        return stored ? new Set(JSON.parse(stored)) : new Set();
      } catch {
        return new Set();
      }
    }
  );

  const [isAddMenuOpen, setIsAddMenuOpen] = useState(false);
  const [draggedNodeId, setDraggedNodeId] = useState<string | null>(null);
  const [dropIndicator, setDropIndicator] =
    useState<PageTreeDropIndicator | null>(null);

  const toggleGroupCollapse = (groupId: string) => {
    setCollapsedGroupIds((prev) => {
      const next = new Set(prev);
      if (next.has(groupId)) {
        next.delete(groupId);
      } else {
        next.add(groupId);
      }
      localStorage.setItem(
        'frontstage_collapsed_groups',
        JSON.stringify(Array.from(next))
      );
      return next;
    });
  };

  const handleAddGroup = () => {
    setIsAddMenuOpen(false);
    onAddGroup();
  };

  const handleAddPage = () => {
    setIsAddMenuOpen(false);
    onAddPage();
  };

  const handleAddMenuBlur = (event: FocusEvent<HTMLDivElement>) => {
    const nextFocusTarget = event.relatedTarget;
    if (
      nextFocusTarget instanceof Node &&
      event.currentTarget.contains(nextFocusTarget)
    ) {
      return;
    }

    setIsAddMenuOpen(false);
  };

  return (
    <div className="frontstage-page-tree-sidebar">
      <ul className="frontstage-page-tree-sidebar__tree">
        {pageTree.map((node) =>
          renderTreeNode({
            node,
            pageTree,
            level: 0,
            siblings: pageTree,
            selectedPageId,
            canEdit,
            isOperationPending,
            collapsedGroupIds,
            toggleGroupCollapse,
            onUpdateNodeMetadata,
            onEditNodeTooltip,
            onAddPageInGroup,
            onAddNodeAtPosition,
            onRenameNode,
            onMoveNode,
            onMoveNodeToPosition,
            onMovePageToGroup,
            onDeleteNode,
            onSelectPage,
            draggedNodeId,
            setDraggedNodeId,
            dropIndicator,
            setDropIndicator
          })
        )}
        {canEdit ? (
          <li className="frontstage-page-tree-sidebar__add-row">
            <div
              className="frontstage-page-tree-sidebar__actions"
              onBlur={handleAddMenuBlur}
              onMouseEnter={() => setIsAddMenuOpen(true)}
              onMouseLeave={() => setIsAddMenuOpen(false)}
            >
              <Button
                aria-expanded={isAddMenuOpen}
                aria-haspopup="menu"
                aria-label={i18nText('frontstage', 'auto.add_menu')}
                className="frontstage-page-tree-sidebar__add-item-btn frontstage-add-action-button frontstage-add-action-button--full"
                disabled={isOperationPending}
                icon={<PlusOutlined />}
                onClick={() => setIsAddMenuOpen(true)}
                onFocus={() => setIsAddMenuOpen(true)}
                size="small"
              >
                {i18nText('frontstage', 'auto.add_menu')}
              </Button>
              {isAddMenuOpen ? (
                <div
                  className="frontstage-page-tree-sidebar__add-menu"
                  role="menu"
                >
                  <button
                    className="frontstage-page-tree-sidebar__add-menu-item"
                    onClick={handleAddGroup}
                    role="menuitem"
                    type="button"
                  >
                    <FolderAddOutlined aria-hidden />
                    {i18nText('frontstage', 'auto.add_group')}
                  </button>
                  <button
                    className="frontstage-page-tree-sidebar__add-menu-item"
                    onClick={handleAddPage}
                    role="menuitem"
                    type="button"
                  >
                    <FileAddOutlined aria-hidden />
                    {i18nText('frontstage', 'auto.add_page')}
                  </button>
                </div>
              ) : null}
            </div>
          </li>
        ) : null}
      </ul>
    </div>
  );
}
