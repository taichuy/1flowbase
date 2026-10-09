import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import DownOutlined from '@ant-design/icons/es/icons/DownOutlined';
import DragOutlined from '@ant-design/icons/es/icons/DragOutlined';
import EditOutlined from '@ant-design/icons/es/icons/EditOutlined';
import EyeInvisibleOutlined from '@ant-design/icons/es/icons/EyeInvisibleOutlined';
import EyeOutlined from '@ant-design/icons/es/icons/EyeOutlined';
import FileAddOutlined from '@ant-design/icons/es/icons/FileAddOutlined';
import FileTextOutlined from '@ant-design/icons/es/icons/FileTextOutlined';
import FolderAddOutlined from '@ant-design/icons/es/icons/FolderAddOutlined';
import FolderOutlined from '@ant-design/icons/es/icons/FolderOutlined';
import InfoCircleOutlined from '@ant-design/icons/es/icons/InfoCircleOutlined';
import MenuOutlined from '@ant-design/icons/es/icons/MenuOutlined';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import RightOutlined from '@ant-design/icons/es/icons/RightOutlined';
import { Button, Typography, Dropdown, Tooltip, Switch } from 'antd';
import { useState } from 'react';
import type { DragEvent, FocusEvent } from 'react';
import type { MenuProps } from 'antd';

import { findNodeById, type FrontStageTreeNode } from '../lib/page-tree';
import { FrontstageNodeActionButton } from './FrontstageNodeActionButton';
import './frontstage-page-tree-sidebar.css';
import './frontstage-add-action.css';
import { i18nText } from '../../../shared/i18n/text';
import {
  canProjectNavigationMove,
  projectNavigationPosition
} from '../lib/navigation-drag/projection';
import { PageTreeIcon } from '../lib/page-tree-icons/registry';

type FrontStagePageTreeSidebarProps = {
  pageTree: FrontStageTreeNode[];
  translateText?: (text: string) => string;
  selectedPageId: string | null;
  canEdit: boolean;
  isOperationPending: boolean;
  onAddGroup: () => void;
  onAddPage: () => void;
  onAddPageInGroup: (groupId: string, kind?: 'page' | 'group') => void;
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
  onMoveNodeToPosition: (
    nodeId: string,
    targetNodeId: string,
    position: 'before' | 'inside' | 'after'
  ) => void;
  onOpenMovePage?: (nodeId: string) => void;
  onDeleteNode: (nodeId: string) => void;
  onSelectPage: (nodeId: string) => void;
};

const PAGE_TREE_DRAG_DATA_TYPE = 'application/x-frontstage-page-tree-node';

type MenuClickInfo = Parameters<NonNullable<MenuProps['onClick']>>[0];

type PageTreeDropIndicator = {
  targetNodeId: string;
  position: 'before' | 'inside' | 'after';
};

function getNodeTitle(
  node: FrontStageTreeNode,
  translateText: (text: string) => string
) {
  if (node.title) {
    return translateText(node.title);
  }

  return node.kind === 'group'
    ? i18nText('frontstage', 'auto.unnamed_group')
    : i18nText('frontstage', 'auto.unnamed_page');
}

function renderNodeIcon(node: FrontStageTreeNode) {
  return <PageTreeIcon name={node.icon} />;
}

function renderTreeNode({
  node,
  pageTree,
  level,
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
  onMoveNodeToPosition,
  onOpenMovePage,
  onDeleteNode,
  onSelectPage,
  draggedNodeId,
  setDraggedNodeId,
  dropIndicator,
  setDropIndicator,
  translateText
}: {
  node: FrontStageTreeNode;
  translateText: (text: string) => string;
  pageTree: FrontStageTreeNode[];
  level: number;
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
  onAddPageInGroup: (groupId: string, kind?: 'page' | 'group') => void;
  onAddNodeAtPosition?: (
    kind: 'page' | 'group',
    targetNodeId: string,
    position: 'before' | 'after'
  ) => void;
  onRenameNode: (node: FrontStageTreeNode) => void;
  onMoveNodeToPosition: (
    nodeId: string,
    targetNodeId: string,
    position: 'before' | 'inside' | 'after'
  ) => void;
  onOpenMovePage?: (nodeId: string) => void;
  onDeleteNode: (nodeId: string) => void;
  onSelectPage: (nodeId: string) => void;
  draggedNodeId: string | null;
  setDraggedNodeId: (nodeId: string | null) => void;
  dropIndicator: PageTreeDropIndicator | null;
  setDropIndicator: (indicator: PageTreeDropIndicator | null) => void;
}) {
  const isPageNode = node.kind === 'page';
  const isSelected = selectedPageId === node.id;
  const canAddPageToGroup = node.kind === 'group';
  const isCollapsed = collapsedGroupIds.has(node.id);
  const isHidden = Boolean(node.is_hidden);
  const tooltipText = node.tooltip ? translateText(node.tooltip) : '';
  const childNodes = node.children ?? [];
  const title = getNodeTitle(node, translateText);
  const isDragging = draggedNodeId === node.id;
  const isInsideDropTarget =
    dropIndicator?.targetNodeId === node.id &&
    dropIndicator.position === 'inside';
  const getDraggedNodeIdFromEvent = (event: DragEvent<HTMLElement>) =>
    draggedNodeId || event.dataTransfer.getData(PAGE_TREE_DRAG_DATA_TYPE);

  const resolveDropPosition = (
    event: DragEvent<HTMLElement>,
    forcedPosition?: 'before' | 'inside' | 'after'
  ) => {
    if (forcedPosition) return forcedPosition;
    const row = event.currentTarget.matches(
      '.frontstage-page-tree-sidebar__node-row'
    )
      ? event.currentTarget
      : event.currentTarget.querySelector(
          '.frontstage-page-tree-sidebar__node-row'
        );
    if (!row) return null;
    const rect = row.getBoundingClientRect();
    return projectNavigationPosition(
      event.clientY,
      rect.top,
      rect.height,
      node.kind === 'group'
    );
  };

  const updateDropIndicator = (
    event: DragEvent<HTMLElement>,
    forcedPosition?: 'before' | 'inside' | 'after'
  ) => {
    event.stopPropagation();
    const sourceId = getDraggedNodeIdFromEvent(event);
    const position = resolveDropPosition(event, forcedPosition);
    if (
      !canEdit ||
      isOperationPending ||
      !draggedNodeId ||
      !sourceId ||
      !position ||
      !canProjectNavigationMove(pageTree, sourceId, node.id, position)
    ) {
      event.dataTransfer.dropEffect = 'none';
      setDropIndicator(null);
      return;
    }
    event.preventDefault();
    event.dataTransfer.dropEffect = 'move';
    setDropIndicator({ targetNodeId: node.id, position });
  };

  const handleDrop = (event: DragEvent<HTMLElement>) => {
    event.preventDefault();
    event.stopPropagation();
    const sourceId = getDraggedNodeIdFromEvent(event);
    setDraggedNodeId(null);
    setDropIndicator(null);
    if (
      !canEdit ||
      isOperationPending ||
      !sourceId ||
      dropIndicator?.targetNodeId !== node.id ||
      !canProjectNavigationMove(
        pageTree,
        sourceId,
        node.id,
        dropIndicator.position
      )
    )
      return;
    onMoveNodeToPosition(sourceId, node.id, dropIndicator.position);
  };

  const draggedNode = draggedNodeId
    ? findNodeById(pageTree, draggedNodeId)
    : null;
  const projectionContent = (position: 'before' | 'inside' | 'after') => (
    <span role="status">
      {draggedNode ? getNodeTitle(draggedNode, translateText) : ''} → {title} ·{' '}
      {position === 'inside'
        ? i18nText('frontstage', 'drag_projection.inside')
        : position === 'before'
          ? i18nText('frontstage', 'drag_projection.before')
          : i18nText('frontstage', 'drag_projection.after')}
    </span>
  );

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
    ...(onOpenMovePage
      ? [
          {
            key: 'move-to',
            label: i18nText('frontstage', 'auto.move_to'),
            icon: <DragOutlined />,
            onClick: ({ domEvent }: MenuClickInfo) => {
              domEvent.stopPropagation();
              onOpenMovePage(node.id);
            }
          }
        ]
      : []),
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
              onAddPageInGroup(node.id, 'page');
            }
          },
          {
            key: 'insert-group-inside',
            label: `${i18nText('frontstage', 'auto.insert_inside')} · ${i18nText('frontstage', 'auto.group')}`,
            icon: <FolderAddOutlined />,
            disabled: isOperationPending,
            onClick: ({ domEvent }: MenuClickInfo) => {
              domEvent.stopPropagation();
              onAddPageInGroup(node.id, 'group');
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
          className="frontstage-page-tree-sidebar__drop-placeholder frontstage-page-tree-sidebar__drop-placeholder--before"
          onDragOver={(event) => updateDropIndicator(event, 'before')}
          onDrop={handleDrop}
        >
          {projectionContent('before')}
        </div>
      ) : null}
      <Tooltip title={tooltipText} placement="rightTop">
        <div
          className={[
            'frontstage-page-tree-sidebar__node-row',
            isSelected
              ? 'frontstage-page-tree-sidebar__node-row--selected'
              : null,
            isInsideDropTarget
              ? 'frontstage-page-tree-sidebar__node-row--drop-inside'
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
          {nodeContent}
          {canEdit ? (
            <>
              <div
                className="frontstage-page-tree-sidebar__node-actions-visible"
                onClick={(e) => e.stopPropagation()}
              >
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
                    if (!canEdit || isOperationPending) {
                      event.preventDefault();
                      return;
                    }
                    event.stopPropagation();
                    event.dataTransfer.effectAllowed = 'move';
                    event.dataTransfer.setData(
                      PAGE_TREE_DRAG_DATA_TYPE,
                      node.id
                    );
                    const row = event.currentTarget.closest(
                      '.frontstage-page-tree-sidebar__node-row'
                    ) as HTMLElement | null;
                    if (row) {
                      const rect = row.getBoundingClientRect();
                      event.dataTransfer.setDragImage(
                        row,
                        event.clientX - rect.left,
                        event.clientY - rect.top
                      );
                    }
                    setDropIndicator(null);
                    setDraggedNodeId(node.id);
                  }}
                  onClick={(event) => {
                    event.stopPropagation();
                  }}
                />
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
      </Tooltip>
      {(!isCollapsed || isInsideDropTarget) &&
      (childNodes.length > 0 ||
        (canEdit && node.kind === 'group') ||
        isInsideDropTarget) ? (
        <ul className="frontstage-page-tree-sidebar__children">
          {childNodes.map((childNode) =>
            renderTreeNode({
              node: childNode,
              pageTree,
              level: level + 1,
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
              onMoveNodeToPosition,
              onOpenMovePage,
              onDeleteNode,
              onSelectPage,
              draggedNodeId,
              setDraggedNodeId,
              dropIndicator,
              setDropIndicator,
              translateText
            })
          )}
          {isInsideDropTarget ? (
            <li className="frontstage-page-tree-sidebar__drop-placeholder-item">
              <div
                className="frontstage-page-tree-sidebar__drop-placeholder frontstage-page-tree-sidebar__drop-placeholder--inside"
                onDragOver={(event) => updateDropIndicator(event, 'inside')}
                onDrop={handleDrop}
              >
                {projectionContent('inside')}
              </div>
            </li>
          ) : null}
        </ul>
      ) : null}
      {dropIndicator?.targetNodeId === node.id &&
      dropIndicator.position === 'after' ? (
        <div
          className="frontstage-page-tree-sidebar__drop-placeholder frontstage-page-tree-sidebar__drop-placeholder--after"
          onDragOver={(event) => updateDropIndicator(event, 'after')}
          onDrop={handleDrop}
        >
          {projectionContent('after')}
        </div>
      ) : null}
    </li>
  );
}

export function FrontStagePageTreeSidebar({
  pageTree,
  translateText = (text) => text,
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
  onMoveNodeToPosition,
  onOpenMovePage,
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
    <div
      className="frontstage-page-tree-sidebar"
      onDragLeave={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null))
          setDropIndicator(null);
      }}
    >
      <ul className="frontstage-page-tree-sidebar__tree">
        {pageTree.map((node) =>
          renderTreeNode({
            node,
            pageTree,
            level: 0,
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
            onMoveNodeToPosition,
            onOpenMovePage,
            onDeleteNode,
            onSelectPage,
            draggedNodeId,
            setDraggedNodeId,
            dropIndicator:
              canEdit && !isOperationPending ? dropIndicator : null,
            setDropIndicator,
            translateText
          })
        )}
      </ul>
      {canEdit ? (
        <div className="frontstage-page-tree-sidebar__add-row">
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
        </div>
      ) : null}
    </div>
  );
}
