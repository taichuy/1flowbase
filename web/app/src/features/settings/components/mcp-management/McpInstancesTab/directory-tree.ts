import type {
  ConsoleMcpGroup,
  ConsoleMcpInstance,
  ConsoleMcpTool,
  ConsoleMcpToolBinding
} from '@1flowbase/api-client';
import type { TreeProps } from 'antd';

import {
  buildMcpDirectoryTreeData,
  type McpDirectoryTreeNode,
  normalizeMcpDirectoryPath
} from '../mcp-management-view-model';
import { i18nText } from '../../../../../shared/i18n/text';

export type McpDirectoryTreeDropInfo = Parameters<
  NonNullable<TreeProps<McpDirectoryTreeNode>['onDrop']>
>[0];

type SaveBindingValues = {
  instance_id: string;
  group_path: string;
  tool_id: string;
  visible: boolean;
  sort_order: number;
};

type MoveGroupValues = {
  instanceId: string;
  sourcePath: string;
  targetParentPath: string;
  sortOrder: number;
};

export function applyMcpDirectoryTreeDrop({
  info,
  selectedInstance,
  selectedInstanceBindings,
  selectedInstanceGroups,
  bindingById,
  groupByPath,
  onSaveBinding,
  onMoveGroup,
  onSelectKey
}: {
  info: McpDirectoryTreeDropInfo;
  selectedInstance: ConsoleMcpInstance;
  selectedInstanceBindings: ConsoleMcpToolBinding[];
  selectedInstanceGroups: ConsoleMcpGroup[];
  bindingById: Map<string, ConsoleMcpToolBinding>;
  groupByPath: Map<string, ConsoleMcpGroup>;
  onSaveBinding: (
    values: SaveBindingValues,
    options: { onSuccess: () => void }
  ) => void;
  onMoveGroup: (
    values: MoveGroupValues,
    options: { onSuccess: (group: ConsoleMcpGroup) => void }
  ) => void;
  onSelectKey: (key: string) => void;
}) {
  const dragKey = String(info.dragNode.key);
  const dropKey = String(info.node.key);
  const dropPosition = info.dropPosition;
  const dropToGap = info.dropToGap;

  if (dragKey.includes('__draft__') || dropKey.includes('__draft__')) {
    return;
  }

  const [dragType, ...dragParts] = dragKey.split(':');
  const [dropType, ...dropParts] = dropKey.split(':');

  // 1. Dragging a Binding (Tool)
  if (dragType === 'binding') {
    const bindingId = dragParts.join(':');
    const draggedBinding = bindingById.get(bindingId);
    if (!draggedBinding) return;

    // Case 1A: Dropped ON a group (dropToGap is false)
    if (dropType === 'group' && !dropToGap) {
      const groupPath = dropParts.join(':');
      const siblings = selectedInstanceBindings
        .filter((b) => normalizeMcpDirectoryPath(b.group_path) === groupPath)
        .sort((a, b) => a.sort_order - b.sort_order);

      const newSortOrder =
        siblings.length > 0 ? siblings[siblings.length - 1].sort_order + 10 : 0;

      onSaveBinding(
        {
          instance_id: selectedInstance.instance_id,
          group_path: groupPath,
          tool_id: draggedBinding.tool_id,
          visible: draggedBinding.visible,
          sort_order: newSortOrder
        },
        {
          onSuccess: () => {
            onSelectKey(`binding:${bindingId}`);
          }
        }
      );
      return;
    }

    // Case 1B: Dropped in the gap of another binding (dropToGap is true)
    if (dropType === 'binding' && dropToGap) {
      const targetBindingId = dropParts.join(':');
      const targetBinding = bindingById.get(targetBindingId);
      if (!targetBinding) return;

      const groupPath = normalizeMcpDirectoryPath(targetBinding.group_path);

      const siblings = selectedInstanceBindings
        .filter(
          (b) =>
            normalizeMcpDirectoryPath(b.group_path) === groupPath &&
            b.id !== bindingId
        )
        .sort((a, b) => a.sort_order - b.sort_order);

      const dropPos = info.node.pos.split('-');
      const relativeDropPos =
        dropPosition - Number(dropPos[dropPos.length - 1]);
      const targetIndex = siblings.findIndex((b) => b.id === targetBindingId);

      let insertIndex = targetIndex;
      if (relativeDropPos === 1) {
        insertIndex = targetIndex + 1;
      }

      let newSortOrder = 0;
      if (siblings.length === 0) {
        newSortOrder = 0;
      } else if (insertIndex <= 0) {
        newSortOrder = siblings[0].sort_order - 10;
      } else if (insertIndex >= siblings.length) {
        newSortOrder = siblings[siblings.length - 1].sort_order + 10;
      } else {
        newSortOrder = Math.round(
          (siblings[insertIndex - 1].sort_order +
            siblings[insertIndex].sort_order) /
            2
        );
      }

      onSaveBinding(
        {
          instance_id: selectedInstance.instance_id,
          group_path: groupPath,
          tool_id: draggedBinding.tool_id,
          visible: draggedBinding.visible,
          sort_order: newSortOrder
        },
        {
          onSuccess: () => {
            onSelectKey(`binding:${bindingId}`);
          }
        }
      );
      return;
    }
  }

  // 2. Dragging a Group
  if (dragType === 'group') {
    const groupPath = dragParts.join(':');
    const draggedGroup = groupByPath.get(groupPath);
    if (!draggedGroup) return;

    const parentPathOf = (path: string) => {
      const segments = normalizeMcpDirectoryPath(path)
        .split('/')
        .filter(Boolean);
      return segments.length <= 1 ? '/' : `/${segments.slice(0, -1).join('/')}`;
    };
    const moveGroup = (targetParentPath: string, sortOrder: number) => {
      if (
        targetParentPath === groupPath ||
        targetParentPath.startsWith(`${groupPath}/`)
      ) {
        return;
      }
      onMoveGroup(
        {
          instanceId: selectedInstance.instance_id,
          sourcePath: groupPath,
          targetParentPath,
          sortOrder
        },
        {
          onSuccess: (group) => {
            onSelectKey(`group:${group.path}`);
          }
        }
      );
    };

    if (!dropToGap && (dropType === 'group' || dropType === 'instance')) {
      const targetParentPath = dropType === 'group' ? dropParts.join(':') : '/';
      const siblings = selectedInstanceGroups
        .filter(
          (group) =>
            group.path !== groupPath &&
            parentPathOf(group.path) === targetParentPath
        )
        .sort((left, right) => left.sort_order - right.sort_order);
      const sortOrder =
        siblings.length > 0 ? siblings[siblings.length - 1].sort_order + 10 : 0;
      moveGroup(targetParentPath, sortOrder);
      return;
    }

    if (dropType === 'group' && dropToGap) {
      const targetGroupPath = dropParts.join(':');
      const targetGroup = groupByPath.get(targetGroupPath);
      if (!targetGroup) return;
      const targetParentPath = parentPathOf(targetGroupPath);
      const siblings = selectedInstanceGroups
        .filter(
          (group) =>
            group.path !== groupPath &&
            parentPathOf(group.path) === targetParentPath
        )
        .sort((left, right) => left.sort_order - right.sort_order);
      const dropPos = info.node.pos.split('-');
      const relativeDropPos =
        dropPosition - Number(dropPos[dropPos.length - 1]);
      const targetIndex = siblings.findIndex(
        (group) => group.path === targetGroupPath
      );
      if (targetIndex < 0) return;
      const insertIndex = relativeDropPos === 1 ? targetIndex + 1 : targetIndex;
      let sortOrder = 0;
      if (siblings.length === 0) {
        sortOrder = 0;
      } else if (insertIndex <= 0) {
        sortOrder = siblings[0].sort_order - 10;
      } else if (insertIndex >= siblings.length) {
        sortOrder = siblings[siblings.length - 1].sort_order + 10;
      } else {
        sortOrder = Math.round(
          (siblings[insertIndex - 1].sort_order +
            siblings[insertIndex].sort_order) /
            2
        );
      }
      moveGroup(targetParentPath, sortOrder);
    }
  }
}

function findDirectoryNodeByPath(
  node: McpDirectoryTreeNode,
  path: string
): McpDirectoryTreeNode | null {
  if (
    node.node_type === 'group' &&
    normalizeMcpDirectoryPath(node.path) === path
  ) {
    return node;
  }
  for (const child of node.children ?? []) {
    const found = findDirectoryNodeByPath(child, path);
    if (found) return found;
  }
  return null;
}

export function buildMcpDirectoryEditorTreeData({
  selectedInstance,
  selectedInstanceGroups,
  selectedInstanceBindings,
  tools,
  groupByPath,
  directoryEditorMode,
  watchedPath,
  watchedDisplayName,
  watchedGroupDescriptionShort,
  watchedGroupPath,
  watchedToolId,
  parentGroupPath,
  directoryEditorIntent,
  directoryDraftActive
}: {
  selectedInstance: ConsoleMcpInstance | undefined;
  selectedInstanceGroups: ConsoleMcpGroup[];
  selectedInstanceBindings: ConsoleMcpToolBinding[];
  tools: ConsoleMcpTool[];
  groupByPath: Map<string, ConsoleMcpGroup>;
  directoryEditorMode: 'group' | 'binding';
  watchedPath?: string;
  watchedDisplayName?: string;
  watchedGroupDescriptionShort?: string | null;
  watchedGroupPath?: string;
  watchedToolId?: string;
  parentGroupPath: string | null;
  directoryEditorIntent: 'create' | 'edit';
  directoryDraftActive: boolean;
}) {
  if (!selectedInstance) return [];
  const nodes = buildMcpDirectoryTreeData({
    instance: {
      id: selectedInstance.id,
      instance_id: selectedInstance.instance_id,
      name: selectedInstance.name,
      default_entry_path: selectedInstance.default_entry_path
    },
    groups: selectedInstanceGroups,
    bindings: selectedInstanceBindings,
    tools
  });
  if (nodes.length === 0) return [];
  const rootNode = nodes[0];
  if (!rootNode.children) rootNode.children = [];

  const isEditingGroup =
    directoryEditorMode === 'group' && directoryEditorIntent === 'edit';
  if (
    directoryEditorMode === 'group' &&
    directoryDraftActive &&
    !isEditingGroup
  ) {
    const pathText = watchedPath?.trim() || '';
    const draftGroupPath = normalizeMcpDirectoryPath(pathText || '/');
    const displayPath =
      pathText || i18nText('settingsMcpManagement', 'auto.unnamed');
    const displayName = watchedDisplayName?.trim();
    const targetParentPath = normalizeMcpDirectoryPath(parentGroupPath || '/');
    let targetParentNode = rootNode;
    if (targetParentPath !== '/') {
      targetParentNode =
        findDirectoryNodeByPath(rootNode, targetParentPath) || rootNode;
    }
    if (!groupByPath.has(draftGroupPath)) {
      if (!targetParentNode.children) targetParentNode.children = [];
      targetParentNode.children.push({
        key: 'group:__draft__',
        title: displayName || displayPath,
        display_name: displayName || undefined,
        description_short: watchedGroupDescriptionShort?.trim() || undefined,
        node_type: 'group',
        path: pathText || '/'
      });
    }
  }

  const isEditingBinding =
    directoryEditorMode === 'binding' && directoryEditorIntent === 'edit';
  if (
    directoryEditorMode === 'binding' &&
    directoryDraftActive &&
    !isEditingBinding
  ) {
    const targetPath = normalizeMcpDirectoryPath(
      watchedGroupPath || selectedInstance.default_entry_path
    );
    const targetNode =
      targetPath === normalizeMcpDirectoryPath(rootNode.path)
        ? rootNode
        : findDirectoryNodeByPath(rootNode, targetPath);
    if (targetNode) {
      if (!targetNode.children) targetNode.children = [];
      const selectedTool = tools.find((tool) => tool.tool_id === watchedToolId);
      targetNode.children.push({
        key: 'binding:__draft__',
        title:
          watchedToolId ||
          i18nText('settingsMcpManagement', 'auto.unnamed_tool'),
        tool_short_description: selectedTool?.short_description,
        node_type: 'binding',
        path: targetPath
      });
    }
  }
  return nodes;
}
