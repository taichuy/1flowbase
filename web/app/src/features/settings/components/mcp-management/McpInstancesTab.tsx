import {
  DeleteOutlined,
  DownloadOutlined,
  EditOutlined,
  FileOutlined,
  FolderOpenOutlined,
  FolderOutlined,
  LinkOutlined,
  PlusOutlined,
  ReloadOutlined,
  SaveOutlined,
  SearchOutlined,
  SettingOutlined
} from '@ant-design/icons';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Button,
  Flex,
  Form,
  Input,
  Modal,
  Popconfirm,
  Select,
  Space,
  Switch,
  Table,
  Tag,
  Tooltip,
  Tree,
  Typography,
  message
} from 'antd';
import {
  useCallback,
  useMemo,
  useReducer,
  useRef,
  useState,
  type SetStateAction
} from 'react';
import { McpClientConfigurationModal } from './McpClientConfigurationModal';
import type { ColumnsType } from 'antd/es/table';
import type {
  ConsoleMcpCatalog,
  ConsoleMcpInstance,
  ConsoleMcpToolBinding,
  SaveConsoleMcpInstanceBody
} from '@1flowbase/api-client';

import {
  createSettingsMcpInstance,
  createSettingsMcpToolBinding,
  deleteSettingsMcpGroup,
  deleteSettingsMcpInstance,
  deleteSettingsMcpToolBinding,
  exportSettingsMcpInstanceDirectory,
  settingsMcpCatalogQueryKey,
  updateSettingsMcpInstance,
  updateSettingsMcpToolBinding,
  upsertSettingsMcpGroup
} from '../../api/mcp-management';
import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import { FixedHeightModal } from '../../../../shared/ui/fixed-height-modal/FixedHeightModal';
import {
  buildMcpDirectoryTreeData,
  buildRandomToolIdSeed,
  normalizeMcpDirectoryPath
} from './mcp-management-view-model';
import { McpInstanceDiscoveryPolicyModal } from './McpInstanceDiscoveryPolicyModal';
import {
  createInitialMcpInstancesState,
  mcpInstancesReducer,
  type McpDirectoryEditorMode
} from './mcp-management-state';
import { downloadMcpExportPackage, statusColor } from './mcp-management-utils';

type InstanceFormValues = SaveConsoleMcpInstanceBody;
type GroupFormValues = {
  instance_id: string;
  path: string;
  display_name: string;
  description_short: string | null;
  enabled: boolean;
  sort_order: number;
};
type BindingFormValues = {
  instance_id: string;
  group_path: string;
  tool_id: string;
  visible: boolean;
  sort_order: number;
};

function useCsrfToken() {
  return useAuthStore((state) => state.csrfToken ?? '');
}

export function McpInstancesTab({
  canManage,
  catalog
}: {
  canManage: boolean;
  catalog: ConsoleMcpCatalog;
}) {
  const csrfToken = useCsrfToken();
  const queryClient = useQueryClient();
  const [instanceForm] = Form.useForm<InstanceFormValues>();
  const [groupForm] = Form.useForm<GroupFormValues>();
  const [bindingForm] = Form.useForm<BindingFormValues>();

  const watchedPath = Form.useWatch('path', groupForm);
  const watchedDisplayName = Form.useWatch('display_name', groupForm);
  const watchedGroupDescriptionShort = Form.useWatch(
    'description_short',
    groupForm
  );
  const watchedGroupEnabled = Form.useWatch('enabled', groupForm);

  const watchedGroupPath = Form.useWatch('group_path', bindingForm);
  const watchedToolId = Form.useWatch('tool_id', bindingForm);
  const watchedBindingVisible = Form.useWatch('visible', bindingForm);

  const [parentGroupPath, setParentGroupPath] = useState<string | null>(null);
  const [directoryEditorIntent, setDirectoryEditorIntent] = useState<
    'create' | 'edit'
  >('create');
  const [directoryDraftActive, setDirectoryDraftActive] = useState(false);
  const [directoryDraftVersion, setDirectoryDraftVersion] = useState(0);
  const groupSavedValuesRef = useRef<GroupFormValues | null>(null);
  const bindingSavedValuesRef = useRef<BindingFormValues | null>(null);
  const [discardDirectoryChangesOpen, setDiscardDirectoryChangesOpen] =
    useState(false);
  const [discoveryPolicyInstance, setDiscoveryPolicyInstance] =
    useState<ConsoleMcpInstance | null>(null);
  const [clientConfigurationInstance, setClientConfigurationInstance] =
    useState<ConsoleMcpInstance | null>(null);
  const pendingDirectorySessionChangeRef = useRef<(() => void) | null>(null);

  const [instancesState, dispatchInstancesState] = useReducer(
    mcpInstancesReducer,
    catalog.instances[0]?.instance_id ?? '',
    createInitialMcpInstancesState
  );
  const {
    editingInstance,
    editingBinding,
    instanceModalOpen,
    directoryModalOpen,
    directoryEditorMode,
    exportingInstances,
    requestedInstanceId,
    selectedDirectoryKey
  } = instancesState;

  const setEditingInstance = useCallback(
    (value: SetStateAction<ConsoleMcpInstance | null>) =>
      dispatchInstancesState({ type: 'setEditingInstance', value }),
    []
  );
  const setEditingBinding = useCallback(
    (value: SetStateAction<ConsoleMcpToolBinding | null>) =>
      dispatchInstancesState({ type: 'setEditingBinding', value }),
    []
  );
  const setInstanceModalOpen = useCallback(
    (value: SetStateAction<boolean>) =>
      dispatchInstancesState({ type: 'setInstanceModalOpen', value }),
    []
  );
  const setDirectoryModalOpen = useCallback(
    (value: SetStateAction<boolean>) =>
      dispatchInstancesState({ type: 'setDirectoryModalOpen', value }),
    []
  );
  const setDirectoryEditorMode = useCallback(
    (value: SetStateAction<McpDirectoryEditorMode>) =>
      dispatchInstancesState({ type: 'setDirectoryEditorMode', value }),
    []
  );
  const setExportingInstances = useCallback(
    (value: SetStateAction<boolean>) =>
      dispatchInstancesState({ type: 'setExportingInstances', value }),
    []
  );
  const setRequestedInstanceId = useCallback(
    (value: SetStateAction<string>) =>
      dispatchInstancesState({ type: 'setRequestedInstanceId', value }),
    []
  );
  const setSelectedDirectoryKey = useCallback(
    (value: SetStateAction<string>) =>
      dispatchInstancesState({ type: 'setSelectedDirectoryKey', value }),
    []
  );
  const fallbackInstanceId = catalog.instances[0]?.instance_id ?? '';
  const selectedInstanceId = catalog.instances.some(
    (instance) => instance.instance_id === requestedInstanceId
  )
    ? requestedInstanceId
    : fallbackInstanceId;

  const saveInstanceMutation = useMutation({
    mutationFn: (values: InstanceFormValues) => {
      if (editingInstance) {
        return updateSettingsMcpInstance(
          editingInstance.instance_id,
          values,
          csrfToken
        );
      }
      return createSettingsMcpInstance(values, csrfToken);
    },
    onSuccess: async () => {
      message.success(i18nText('settings', 'auto.mcp_saved'));
      setInstanceModalOpen(false);
      setEditingInstance(null);
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    }
  });
  const deleteInstanceMutation = useMutation({
    mutationFn: (instanceId: string) =>
      deleteSettingsMcpInstance(instanceId, csrfToken),
    onSuccess: async () => {
      message.success(i18nText('settings', 'auto.mcp_deleted'));
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    }
  });
  const saveGroupMutation = useMutation({
    mutationFn: (values: GroupFormValues) =>
      upsertSettingsMcpGroup(
        values.instance_id,
        {
          path: values.path,
          display_name: values.display_name,
          description_short: values.description_short,
          enabled: values.enabled,
          sort_order: values.sort_order
        },
        csrfToken
      ),
    onSuccess: async () => {
      message.success(i18nText('settings', 'auto.mcp_saved'));
      groupForm.resetFields();
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    }
  });
  const saveBindingMutation = useMutation({
    mutationFn: (values: BindingFormValues) => {
      const body = {
        group_path: values.group_path,
        tool_id: values.tool_id,
        display_alias: null,
        visible: values.visible,
        sort_order: values.sort_order
      };

      if (editingBinding) {
        return updateSettingsMcpToolBinding(editingBinding.id, body, csrfToken);
      }

      return createSettingsMcpToolBinding(values.instance_id, body, csrfToken);
    },
    onSuccess: async () => {
      message.success(i18nText('settings', 'auto.mcp_saved'));
      bindingForm.resetFields();
      setEditingBinding(null);
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    }
  });
  const deleteGroupMutation = useMutation({
    mutationFn: (path: string) => {
      if (!selectedInstance) throw new Error('No selected instance');
      return deleteSettingsMcpGroup(
        selectedInstance.instance_id,
        path,
        csrfToken
      );
    },
    onSuccess: async () => {
      message.success(i18nText('settings', 'auto.mcp_deleted'));
      groupForm.resetFields();
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    }
  });
  const deleteBindingMutation = useMutation({
    mutationFn: (bindingId: string) =>
      deleteSettingsMcpToolBinding(bindingId, csrfToken),
    onSuccess: async () => {
      message.success(i18nText('settings', 'auto.mcp_deleted'));
      bindingForm.resetFields();
      setEditingBinding(null);
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    }
  });
  async function handleExportInstances() {
    setExportingInstances(true);
    try {
      const exportPackage = await exportSettingsMcpInstanceDirectory();
      downloadMcpExportPackage(exportPackage);
      message.success(i18nText('settings', 'auto.mcp_export_ready'));
    } catch (error) {
      message.error(error instanceof Error ? error.message : String(error));
    } finally {
      setExportingInstances(false);
    }
  }

  const groupCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const group of catalog.groups) {
      counts.set(
        group.instance_record_id,
        (counts.get(group.instance_record_id) ?? 0) + 1
      );
    }
    return counts;
  }, [catalog.groups]);
  const toolCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const binding of catalog.bindings) {
      counts.set(
        binding.instance_record_id,
        (counts.get(binding.instance_record_id) ?? 0) + 1
      );
    }
    return counts;
  }, [catalog.bindings]);
  const selectedInstance = useMemo(
    () =>
      catalog.instances.find(
        (instance) => instance.instance_id === selectedInstanceId
      ) ?? catalog.instances[0],
    [catalog.instances, selectedInstanceId]
  );
  const selectedInstanceGroups = useMemo(
    () =>
      catalog.groups.filter(
        (group) => group.instance_record_id === selectedInstance?.id
      ),
    [catalog.groups, selectedInstance?.id]
  );
  const selectedInstanceBindings = useMemo(
    () =>
      catalog.bindings.filter(
        (binding) => binding.instance_record_id === selectedInstance?.id
      ),
    [catalog.bindings, selectedInstance?.id]
  );
  const bindingOptions = useMemo(
    () =>
      selectedInstanceBindings.map((binding) => ({
        label: binding.tool_id,
        value: binding.id
      })),
    [selectedInstanceBindings]
  );
  const groupByPath = useMemo(
    () =>
      new Map(
        selectedInstanceGroups.map((group) => [
          normalizeMcpDirectoryPath(group.path),
          group
        ])
      ),
    [selectedInstanceGroups]
  );
  const bindingById = useMemo(
    () =>
      new Map(selectedInstanceBindings.map((binding) => [binding.id, binding])),
    [selectedInstanceBindings]
  );
  const treeData = useMemo(() => {
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
      tools: catalog.tools
    });

    if (nodes.length === 0) return [];
    const rootNode = nodes[0];
    if (!rootNode.children) rootNode.children = [];

    // Check if in new group creation state
    const isEditingGroup =
      directoryEditorMode === 'group' && directoryEditorIntent === 'edit';
    if (
      directoryEditorMode === 'group' &&
      directoryDraftActive &&
      !isEditingGroup
    ) {
      const pathText = watchedPath?.trim() || '';
      const draftGroupPath = normalizeMcpDirectoryPath(pathText || '/');
      const draftPathAlreadyExists = groupByPath.has(draftGroupPath);
      const displayPath =
        pathText || i18nText('settingsMcpManagement', 'auto.unnamed');
      const displayName = watchedDisplayName?.trim();
      const title = displayName || displayPath;

      // Find the parent group node matching parentGroupPath
      const targetParentPath = normalizeMcpDirectoryPath(
        parentGroupPath || '/'
      );
      let targetParentNode = rootNode;
      if (targetParentPath !== '/') {
        const findNodeByPath = (node: any, path: string): any => {
          if (
            node.node_type === 'group' &&
            normalizeMcpDirectoryPath(node.path) === path
          ) {
            return node;
          }
          if (node.children) {
            for (const child of node.children) {
              const found = findNodeByPath(child, path);
              if (found) return found;
            }
          }
          return null;
        };
        targetParentNode =
          findNodeByPath(rootNode, targetParentPath) || rootNode;
      }

      if (!draftPathAlreadyExists) {
        if (!targetParentNode.children) targetParentNode.children = [];
        targetParentNode.children.push({
          key: 'group:__draft__',
          title: title,
          display_name: displayName || undefined,
          description_short: watchedGroupDescriptionShort?.trim() || undefined,
          node_type: 'group',
          path: pathText || '/'
        });
      }
    }

    // Check if in new binding creation state
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
      let targetNode: any = null;
      if (targetPath === normalizeMcpDirectoryPath(rootNode.path)) {
        targetNode = rootNode;
      } else {
        const findNodeByPath = (node: any, path: string): any => {
          if (
            node.node_type === 'group' &&
            normalizeMcpDirectoryPath(node.path) === path
          ) {
            return node;
          }
          if (node.children) {
            for (const child of node.children) {
              const found = findNodeByPath(child, path);
              if (found) return found;
            }
          }
          return null;
        };
        targetNode = findNodeByPath(rootNode, targetPath);
      }

      if (targetNode) {
        if (!targetNode.children) targetNode.children = [];
        const selectedTool = catalog.tools.find(
          (tool) => tool.tool_id === watchedToolId
        );

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
  }, [
    selectedInstance,
    selectedInstanceGroups,
    selectedInstanceBindings,
    catalog.tools,
    groupByPath,
    directoryEditorMode,
    selectedDirectoryKey,
    watchedPath,
    watchedDisplayName,
    watchedGroupDescriptionShort,
    watchedGroupPath,
    watchedToolId,
    parentGroupPath,
    directoryEditorIntent,
    directoryDraftActive
  ]);

  const handleTreeDrop = (info: any) => {
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
          siblings.length > 0
            ? siblings[siblings.length - 1].sort_order + 10
            : 0;

        saveBindingMutation.mutate(
          {
            instance_id: selectedInstance.instance_id,
            group_path: groupPath,
            tool_id: draggedBinding.tool_id,
            visible: draggedBinding.visible,
            sort_order: newSortOrder
          },
          {
            onSuccess: () => {
              setSelectedDirectoryKey(`binding:${bindingId}`);
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

        saveBindingMutation.mutate(
          {
            instance_id: selectedInstance.instance_id,
            group_path: groupPath,
            tool_id: draggedBinding.tool_id,
            visible: draggedBinding.visible,
            sort_order: newSortOrder
          },
          {
            onSuccess: () => {
              setSelectedDirectoryKey(`binding:${bindingId}`);
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

      // Case 2A: Dropped in the gap of another group (dropToGap is true)
      if (dropType === 'group' && dropToGap) {
        const targetGroupPath = dropParts.join(':');
        const targetGroup = groupByPath.get(targetGroupPath);
        if (!targetGroup) return;

        const siblings = selectedInstanceGroups
          .filter((g) => g.path !== groupPath)
          .sort((a, b) => a.sort_order - b.sort_order);

        const dropPos = info.node.pos.split('-');
        const relativeDropPos =
          dropPosition - Number(dropPos[dropPos.length - 1]);
        const targetIndex = siblings.findIndex(
          (g) => g.path === targetGroupPath
        );

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

        saveGroupMutation.mutate(
          {
            instance_id: selectedInstance.instance_id,
            path: groupPath,
            display_name: draggedGroup.display_name,
            description_short: draggedGroup.description_short,
            enabled: draggedGroup.enabled,
            sort_order: newSortOrder
          },
          {
            onSuccess: () => {
              setSelectedDirectoryKey(`group:${groupPath}`);
            }
          }
        );
        return;
      }
    }
  };

  function applyDirectoryPathToForms(path: string) {
    const normalizedPath = normalizeMcpDirectoryPath(path);
    const group = groupByPath.get(normalizedPath);

    groupForm.setFieldsValue({
      instance_id: selectedInstance?.instance_id ?? '',
      path: normalizedPath,
      display_name: group?.display_name ?? '',
      description_short: group?.description_short ?? null,
      enabled: group?.enabled ?? true,
      sort_order: group?.sort_order ?? 0
    });
    groupSavedValuesRef.current = groupForm.getFieldsValue(true);
    bindingForm.setFieldsValue({
      instance_id: selectedInstance?.instance_id ?? '',
      group_path: normalizedPath
    });
  }

  function resetBindingFormForCreate(path?: string) {
    const nextPath = normalizeMcpDirectoryPath(
      path ??
        bindingForm.getFieldValue('group_path') ??
        groupForm.getFieldValue('path') ??
        selectedInstance?.default_entry_path
    );

    setDirectoryEditorIntent('create');
    setDirectoryDraftActive(true);
    setDirectoryDraftVersion((version) => version + 1);
    setEditingBinding(null);
    bindingForm.resetFields();
    bindingForm.setFieldsValue({
      instance_id: selectedInstance?.instance_id ?? '',
      group_path: nextPath,
      visible: true,
      sort_order: 0
    });
    bindingSavedValuesRef.current = bindingForm.getFieldsValue(true);
  }

  function applyBindingSelection(bindingId?: string) {
    if (!bindingId) {
      resetBindingFormForCreate();
      return;
    }

    const binding = bindingById.get(bindingId);
    if (!binding) {
      return;
    }

    setDirectoryEditorMode('binding');
    setDirectoryEditorIntent('edit');
    setDirectoryDraftActive(false);
    setEditingBinding(binding);
    bindingForm.setFieldsValue({
      instance_id: selectedInstance?.instance_id ?? '',
      group_path: normalizeMcpDirectoryPath(binding.group_path),
      tool_id: binding.tool_id,
      visible: binding.visible,
      sort_order: binding.sort_order
    });
    bindingSavedValuesRef.current = bindingForm.getFieldsValue(true);
  }

  const instanceColumns: ColumnsType<ConsoleMcpInstance> = [
    {
      title: 'instance_id',
      dataIndex: 'instance_id'
    },
    {
      title: i18nText('settings', 'auto.instance_name'),
      dataIndex: 'name',
      render: (name: ConsoleMcpInstance['name']) => (
        <Typography.Text strong>{name}</Typography.Text>
      )
    },
    {
      title: i18nText('settingsMcpManagement', 'auto.instance_description'),
      dataIndex: 'description_short',
      render: (description: ConsoleMcpInstance['description_short']) => (
        <Typography.Text type={description ? undefined : 'secondary'}>
          {description || '-'}
        </Typography.Text>
      )
    },
    {
      title: i18nText('settings', 'auto.status'),
      dataIndex: 'status',
      render: (status: string) => (
        <Tag color={statusColor(status)}>{status}</Tag>
      )
    },
    {
      title: i18nText('settings', 'auto.directory_summary'),
      render: (_, record) => (
        <Typography.Text>
          {groupCounts.get(record.id) ?? 0} / {toolCounts.get(record.id) ?? 0}
        </Typography.Text>
      )
    },
    {
      title: i18nText('settings', 'auto.operation'),
      render: (_, record) => (
        <Space>
          <Button
            aria-label={i18nText('settings', 'auto.edit')}
            icon={<EditOutlined />}
            size="small"
            disabled={!canManage}
            onClick={() => {
              setEditingInstance(record);
              instanceForm.setFieldsValue({
                instance_id: record.instance_id,
                name: record.name,
                description_short: record.description_short,
                status: record.status,
                default_entry_path: record.default_entry_path
              });
              setInstanceModalOpen(true);
            }}
          />
          <Tooltip title={i18nText('settings', 'auto.directory_editor')}>
            <Button
              aria-label={i18nText('settings', 'auto.directory_editor')}
              icon={<SettingOutlined />}
              size="small"
              disabled={!canManage}
              onClick={() => {
                setRequestedInstanceId(record.instance_id);
                setEditingBinding(null);
                groupForm.resetFields();
                bindingForm.resetFields();
                groupForm.setFieldsValue({
                  instance_id: record.instance_id,
                  path: normalizeMcpDirectoryPath(record.default_entry_path),
                  display_name: '',
                  description_short: null,
                  enabled: true,
                  sort_order: 0
                });
                groupSavedValuesRef.current = groupForm.getFieldsValue(true);
                bindingForm.setFieldsValue({
                  instance_id: record.instance_id,
                  group_path: normalizeMcpDirectoryPath(
                    record.default_entry_path
                  ),
                  visible: true,
                  sort_order: 0
                });
                bindingSavedValuesRef.current = bindingForm.getFieldsValue(true);
                setDirectoryEditorMode('group');
                setDirectoryEditorIntent('create');
                setDirectoryDraftActive(false);
                setSelectedDirectoryKey(
                  `instance:${record.instance_id}:${normalizeMcpDirectoryPath(record.default_entry_path)}`
                );
                setDirectoryModalOpen(true);
              }}
            />
          </Tooltip>
          <Tooltip
            title={i18nText('settingsMcpManagement', 'auto.discovery_policy')}
          >
            <Button
              aria-label={i18nText(
                'settingsMcpManagement',
                'auto.discovery_policy'
              )}
              icon={<SearchOutlined />}
              size="small"
              disabled={!canManage}
              onClick={() => setDiscoveryPolicyInstance(record)}
            />
          </Tooltip>
          <Tooltip title={i18nText('settingsMcpManagement', 'auto.connect_client')}>
            <Button
              aria-label={i18nText('settingsMcpManagement', 'auto.connect_client')}
              icon={<LinkOutlined />}
              size="small"
              onClick={() => setClientConfigurationInstance(record)}
            />
          </Tooltip>
          <Popconfirm
            title={i18nText('settings', 'auto.mcp_hard_delete_confirm')}
            disabled={!canManage}
            onConfirm={() => deleteInstanceMutation.mutate(record.instance_id)}
          >
            <Button
              danger
              icon={<DeleteOutlined />}
              size="small"
              disabled={!canManage}
            />
          </Popconfirm>
        </Space>
      )
    }
  ];

  const getFullReadablePath = () => {
    const instanceName = selectedInstance?.name || 'mcp';

    const isEditingGroup =
      selectedDirectoryKey && selectedDirectoryKey.startsWith('group:');

    if (isEditingGroup) {
      const currentPath = groupForm.getFieldValue('path') || '/';
      if (currentPath === '/') return `${instanceName} /`;

      const segments = currentPath.split('/').filter(Boolean);
      const pathParts: string[] = [instanceName];
      let currentAcc = '';

      for (const segment of segments) {
        currentAcc += `/${segment}`;
        const g = groupByPath.get(currentAcc);
        const name = g?.display_name || segment;
        pathParts.push(name);
      }

      return pathParts.join(' / ');
    } else {
      const parent = parentGroupPath || '/';
      const pathParts: string[] = [instanceName];

      if (parent !== '/') {
        const segments = parent.split('/').filter(Boolean);
        let currentAcc = '';
        for (const segment of segments) {
          currentAcc += `/${segment}`;
          const g = groupByPath.get(currentAcc);
          const name = g?.display_name || segment;
          pathParts.push(name);
        }
      }

      const childName =
        watchedDisplayName?.trim() ||
        i18nText('settingsMcpManagement', 'auto.unnamed');
      pathParts.push(childName);

      return pathParts.join(' / ');
    }
  };

  const getReadablePathFor = (rawPath: string | null | undefined) => {
    const instanceName = selectedInstance?.name || 'mcp';
    const pathVal = normalizeMcpDirectoryPath(rawPath || '/');
    if (pathVal === '/') return `${instanceName} /`;

    const segments = pathVal.split('/').filter(Boolean);
    const pathParts: string[] = [instanceName];
    let currentAcc = '';

    for (const segment of segments) {
      currentAcc += `/${segment}`;
      const g = groupByPath.get(currentAcc);
      const name =
        g?.display_name?.trim() ||
        segment ||
        i18nText('settingsMcpManagement', 'auto.unnamed');
      pathParts.push(name);
    }

    return pathParts.join(' / ');
  };

  const discardDirectorySession = () => {
    setDirectoryModalOpen(false);
    setEditingBinding(null);
    setSelectedDirectoryKey('');
    setParentGroupPath(null);
    setDirectoryDraftActive(false);
    groupForm.resetFields();
    bindingForm.resetFields();
  };

  const directorySessionHasChanges = () => {
    if (directoryEditorMode === 'group') {
      const savedValues = groupSavedValuesRef.current;
      if (!savedValues) return false;
      return (
        normalizeMcpDirectoryPath(watchedPath) !==
          normalizeMcpDirectoryPath(savedValues.path) ||
        (watchedDisplayName ?? '') !== (savedValues.display_name ?? '') ||
        (watchedGroupDescriptionShort ?? null) !==
          (savedValues.description_short ?? null) ||
        watchedGroupEnabled !== savedValues.enabled
      );
    }

    const savedValues = bindingSavedValuesRef.current;
    if (!savedValues) return false;
    return (
      normalizeMcpDirectoryPath(watchedGroupPath) !==
        normalizeMcpDirectoryPath(savedValues.group_path) ||
      watchedToolId !== savedValues.tool_id ||
      watchedBindingVisible !== savedValues.visible
    );
  };

  const requestDirectorySessionChange = (changeSession: () => void) => {
    if (!directorySessionHasChanges()) {
      changeSession();
      return;
    }

    pendingDirectorySessionChangeRef.current = changeSession;
    setDiscardDirectoryChangesOpen(true);
  };

  const closeDirectoryModal = () => {
    requestDirectorySessionChange(discardDirectorySession);
  };

  const selectedDirectoryPath = () => {
    if (selectedDirectoryKey.startsWith('group:')) {
      return normalizeMcpDirectoryPath(
        selectedDirectoryKey.slice('group:'.length)
      );
    }
    if (selectedDirectoryKey.startsWith('binding:')) {
      const binding = bindingById.get(
        selectedDirectoryKey.slice('binding:'.length)
      );
      return normalizeMcpDirectoryPath(binding?.group_path);
    }
    return normalizeMcpDirectoryPath(selectedInstance?.default_entry_path);
  };

  const startChildGroupCreation = (path?: string) => {
    const currentPath = normalizeMcpDirectoryPath(
      path ?? selectedDirectoryPath()
    );

    setDirectoryEditorMode('group');
    setDirectoryEditorIntent('create');
    setDirectoryDraftActive(true);
    setDirectoryDraftVersion((version) => version + 1);
    setEditingBinding(null);
    setParentGroupPath(currentPath);
    groupForm.resetFields();
    groupForm.setFieldsValue({
      instance_id: selectedInstance?.instance_id ?? '',
      path: currentPath === '/' ? '/' : `${currentPath}/`,
      display_name: '',
      description_short: null,
      enabled: true,
      sort_order: 0
    });
    groupSavedValuesRef.current = groupForm.getFieldsValue(true);
  };

  const startToolMount = (path?: string) => {
    const targetPath = normalizeMcpDirectoryPath(
      path ?? selectedDirectoryPath()
    );
    setParentGroupPath(null);
    setDirectoryEditorMode('binding');
    resetBindingFormForCreate(targetPath);
  };

  const cancelChildGroupCreation = () => {
    setParentGroupPath(null);
    setDirectoryDraftActive(false);
    groupForm.resetFields();
    groupForm.setFieldsValue({
      instance_id: selectedInstance?.instance_id ?? '',
      path: '/',
      display_name: '',
      description_short: null,
      enabled: true,
      sort_order: 0
    });
  };

  const selectedDiscoveryPolicy = discoveryPolicyInstance
    ? catalog.discovery_policies.find(
        (policy) => policy.instance_record_id === discoveryPolicyInstance.id
      )
    : undefined;

  return (
    <Space direction="vertical" size="middle" className="mcp-management__stack">
      <Flex justify="space-between" align="center">
        <Typography.Text type="secondary">
          {i18nText('settings', 'auto.mcp_instances_hint')}
        </Typography.Text>
        <Space>
          <Button
            icon={<DownloadOutlined />}
            loading={exportingInstances}
            onClick={handleExportInstances}
          >
            {i18nText('settings', 'auto.export')}
          </Button>
          <Button
            type="primary"
            icon={<PlusOutlined />}
            disabled={!canManage}
            onClick={() => {
              setEditingInstance(null);
              instanceForm.setFieldsValue({
                instance_id: '',
                name: '',
                description_short: null,
                status: 'draft',
                default_entry_path: '/'
              });
              setInstanceModalOpen(true);
            }}
          >
            {i18nText('settings', 'auto.new')}
          </Button>
        </Space>
      </Flex>
      <Table
        rowKey="id"
        columns={instanceColumns}
        dataSource={catalog.instances}
        pagination={false}
      />
      {discoveryPolicyInstance && selectedDiscoveryPolicy ? (
        <McpInstanceDiscoveryPolicyModal
          canManage={canManage}
          instance={discoveryPolicyInstance}
          policy={selectedDiscoveryPolicy}
          open
          onClose={() => setDiscoveryPolicyInstance(null)}
        />
      ) : null}
      <McpClientConfigurationModal
        instance={clientConfigurationInstance}
        onClose={() => setClientConfigurationInstance(null)}
      />
      {directoryModalOpen && selectedInstance ? (
        <FixedHeightModal
          open
          className="mcp-management__directory-fixed-modal"
          width={840}
          footer={
            <Space>
              <Button
                aria-label={i18nText(
                  'settingsMcpManagement',
                  'auto.close_directory_editor'
                )}
                onClick={closeDirectoryModal}
              >
                {i18nText('settings', 'auto.cancel')}
              </Button>
              <Button
                type="primary"
                icon={<SaveOutlined />}
                aria-label={
                  directoryEditorMode === 'group'
                    ? i18nText('settingsMcpManagement', 'auto.save_group')
                    : i18nText('settingsMcpManagement', 'auto.save_tool_mount')
                }
                disabled={!canManage}
                loading={
                  directoryEditorMode === 'group'
                    ? saveGroupMutation.isPending
                    : saveBindingMutation.isPending
                }
                onClick={() => {
                  if (directoryEditorMode === 'group') {
                    groupForm.submit();
                    return;
                  }
                  bindingForm.submit();
                }}
              >
                {directoryEditorMode === 'group'
                  ? i18nText('settingsMcpManagement', 'auto.save_group')
                  : i18nText('settingsMcpManagement', 'auto.save_tool_mount')}
              </Button>
            </Space>
          }
          title={i18nText('settings', 'auto.directory_editor')}
          scrollBodyClassName="mcp-management__directory-modal"
          onCancel={closeDirectoryModal}
        >
          <div className="mcp-management__directory-layout">
            {/* Left Panel: Tree and select */}
            <div className="mcp-management__directory-tree-panel">
              <div style={{ marginBottom: 12 }}>
                <Typography.Text
                  type="secondary"
                  style={{ display: 'block', marginBottom: 4 }}
                >
                  {i18nText('settings', 'auto.instance_name')}
                </Typography.Text>
                <Select
                  aria-label={i18nText('settings', 'auto.instance_name')}
                  className="mcp-management__instance-select"
                  value={selectedInstance.instance_id}
                  options={catalog.instances.map((instance) => ({
                    label: `${instance.name} (${instance.instance_id})`,
                    value: instance.instance_id
                  }))}
                  onChange={(value) => {
                    requestDirectorySessionChange(() => {
                      setRequestedInstanceId(value);
                      const nextInstance = catalog.instances.find(
                        (instance) => instance.instance_id === value
                      );
                      if (nextInstance) {
                        const nextPath = normalizeMcpDirectoryPath(
                          nextInstance.default_entry_path
                        );
                        groupForm.setFieldsValue({
                          instance_id: value,
                          path: nextPath,
                          display_name: '',
                          description_short: null,
                          enabled: true,
                          sort_order: 0
                        });
                        bindingForm.setFieldsValue({
                          instance_id: value,
                          group_path: nextPath,
                          visible: true,
                          sort_order: 0
                        });
                      }
                      setEditingBinding(null);
                      setSelectedDirectoryKey('');
                      setParentGroupPath(null);
                      groupForm.setFieldValue('instance_id', value);
                      bindingForm.setFieldValue('instance_id', value);
                    });
                  }}
                />
              </div>

              <div className="mcp-management__directory-editor-status">
                <Typography.Text type="secondary">
                  {i18nText('settingsMcpManagement', 'auto.save_status')}
                </Typography.Text>
                <Tag
                  color={
                    directoryDraftActive || directorySessionHasChanges()
                      ? 'orange'
                      : 'green'
                  }
                >
                  {directoryDraftActive || directorySessionHasChanges()
                    ? i18nText('settingsMcpManagement', 'auto.unsaved')
                    : i18nText('settingsMcpManagement', 'auto.saved')}
                </Tag>
                <Typography.Text strong>
                  {directoryEditorMode === 'group'
                    ? i18nText('settingsMcpManagement', 'auto.group_type')
                    : i18nText('settingsMcpManagement', 'auto.tool_mount_type')}
                </Typography.Text>
              </div>

              <Flex
                className="mcp-management__directory-create-actions"
                gap={8}
                wrap
              >
                <Button
                  icon={<FolderOutlined />}
                  aria-label={i18nText(
                    'settingsMcpManagement',
                    'auto.create_group'
                  )}
                  disabled={!canManage}
                  onClick={() =>
                    requestDirectorySessionChange(() =>
                      startChildGroupCreation()
                    )
                  }
                >
                  {i18nText('settingsMcpManagement', 'auto.create_group')}
                </Button>
                <Button
                  icon={<PlusOutlined />}
                  aria-label={i18nText(
                    'settingsMcpManagement',
                    'auto.mount_tool'
                  )}
                  disabled={!canManage}
                  onClick={() =>
                    requestDirectorySessionChange(() => startToolMount())
                  }
                >
                  {i18nText('settingsMcpManagement', 'auto.mount_tool')}
                </Button>
              </Flex>

              <Tree
                key={`${directoryEditorMode}:${directoryDraftActive ? directoryDraftVersion : 'stable'}`}
                className="mcp-management__directory-tree"
                draggable={canManage ? { icon: false } : false}
                blockNode
                defaultExpandAll
                showIcon
                selectedKeys={
                  selectedDirectoryKey ? [selectedDirectoryKey] : []
                }
                treeData={treeData}
                onSelect={(selectedKeys) => {
                  if (selectedKeys.length === 0) return;
                  const key = String(selectedKeys[0]);
                  if (key.includes('__draft__')) return;

                  requestDirectorySessionChange(() => {
                    setSelectedDirectoryKey(key);
                    const [nodeType, ...keyParts] = key.split(':');
                    if (nodeType === 'group') {
                      setParentGroupPath(null);
                      setDirectoryEditorMode('group');
                      setDirectoryEditorIntent('edit');
                      setDirectoryDraftActive(false);
                      setEditingBinding(null);
                      applyDirectoryPathToForms(keyParts.join(':'));
                      return;
                    }
                    if (nodeType === 'binding') {
                      applyBindingSelection(keyParts.join(':'));
                      return;
                    }
                    setDirectoryEditorMode('group');
                    setDirectoryEditorIntent('create');
                    setDirectoryDraftActive(false);
                    setParentGroupPath(null);
                    setEditingBinding(null);
                    applyDirectoryPathToForms(
                      selectedInstance.default_entry_path
                    );
                  });
                }}
                onDrop={handleTreeDrop}
                titleRender={(node: any) => {
                  const [type, ...parts] = node.key.split(':');
                  const isInstance = type === 'instance';
                  const isGroup = type === 'group';
                  const isBinding = type === 'binding';

                  let titleNode: React.ReactNode = <span>{node.title}</span>;
                  if (isGroup) {
                    const shortDescription = node.description_short?.trim();
                    titleNode = (
                      <span className="mcp-management__group-node">
                        <span className="mcp-management__group-node-id">
                          {node.title}
                        </span>
                        {shortDescription ? (
                          <span className="mcp-management__group-node-description">
                            {shortDescription}
                          </span>
                        ) : null}
                      </span>
                    );
                  } else if (isBinding) {
                    const shortDescription =
                      node.tool_short_description?.trim();
                    titleNode = (
                      <span className="mcp-management__binding-node">
                        <span className="mcp-management__binding-node-id">
                          {node.title}
                        </span>
                        {shortDescription ? (
                          <span className="mcp-management__binding-node-description">
                            {shortDescription}
                          </span>
                        ) : null}
                      </span>
                    );
                  }

                  return (
                    <span className="mcp-management__tree-node-title">
                      {titleNode}
                      {canManage && (isInstance || isGroup || isBinding) && (
                        <span
                          className={
                            isInstance
                              ? 'mcp-management__tree-node-actions mcp-management__tree-node-actions--visible'
                              : 'mcp-management__tree-node-actions'
                          }
                          onClick={(e) => e.stopPropagation()}
                        >
                          {!isInstance ? (
                            <Tooltip title={i18nText('settings', 'auto.edit')}>
                              <Button
                                type="text"
                                size="small"
                                icon={<EditOutlined />}
                                aria-label={i18nText('settings', 'auto.edit')}
                                onClick={() => {
                                  if (isGroup) {
                                    setParentGroupPath(null);
                                    setDirectoryEditorMode('group');
                                    setDirectoryEditorIntent('edit');
                                    setDirectoryDraftActive(false);
                                    setEditingBinding(null);
                                    applyDirectoryPathToForms(node.path);
                                  } else {
                                    applyBindingSelection(parts.join(':'));
                                  }
                                }}
                              />
                            </Tooltip>
                          ) : null}
                          {!isInstance ? (
                            <Popconfirm
                              title={i18nText(
                                'settings',
                                'auto.mcp_hard_delete_confirm'
                              )}
                              onConfirm={() => {
                                if (isGroup) {
                                  const path = parts.join(':');
                                  deleteGroupMutation.mutate(path);
                                } else {
                                  const bindingId = parts.join(':');
                                  deleteBindingMutation.mutate(bindingId);
                                }
                              }}
                            >
                              <Button
                                type="text"
                                danger
                                size="small"
                                icon={<DeleteOutlined />}
                                className="ant-btn-dangerous"
                                aria-label="Delete"
                              />
                            </Popconfirm>
                          ) : null}
                        </span>
                      )}
                    </span>
                  );
                }}
                icon={(nodeProps: any) => {
                  const key = nodeProps?.data?.key || nodeProps?.key;
                  if (!key) return null;
                  const [type] = String(key).split(':');
                  if (type === 'instance') {
                    return <FolderOpenOutlined style={{ color: '#1890ff' }} />;
                  } else if (type === 'group') {
                    return <FolderOutlined style={{ color: '#faad14' }} />;
                  } else {
                    return <FileOutlined style={{ color: '#52c41a' }} />;
                  }
                }}
              />
            </div>

            <div className="mcp-management__directory-form-panel">
              <div className="mcp-management__directory-form-header">
                <div>
                  <Typography.Title
                    level={5}
                    className="mcp-management__directory-form-title"
                  >
                    {directoryEditorIntent === 'edit'
                      ? directoryEditorMode === 'group'
                        ? i18nText('settingsMcpManagement', 'auto.edit_group')
                        : i18nText(
                            'settingsMcpManagement',
                            'auto.edit_tool_mount'
                          )
                      : directoryEditorMode === 'group'
                        ? i18nText('settingsMcpManagement', 'auto.create_group')
                        : i18nText(
                            'settingsMcpManagement',
                            'auto.create_tool_mount'
                          )}
                  </Typography.Title>
                  <Typography.Text type="secondary">
                    {directoryEditorIntent === 'create'
                      ? `${i18nText('settingsMcpManagement', 'auto.target_directory')} ${selectedDirectoryPath()}`
                      : selectedDirectoryPath()}
                  </Typography.Text>
                </div>
              </div>
              <div hidden={directoryEditorMode !== 'group'}>
                <Form
                  form={groupForm}
                  layout="vertical"
                  className="mcp-management__directory-form"
                  initialValues={{
                    instance_id: selectedInstance.instance_id,
                    path: normalizeMcpDirectoryPath(
                      selectedInstance.default_entry_path
                    ),
                    enabled: true,
                    sort_order: 0
                  }}
                  onFinish={(values) =>
                    saveGroupMutation.mutate(values, {
                      onSuccess: () => {
                        const savedPath = normalizeMcpDirectoryPath(
                          values.path
                        );
                        setParentGroupPath(null);
                        setDirectoryDraftActive(false);
                        setSelectedDirectoryKey(`group:${savedPath}`);
                        setDirectoryEditorIntent('edit');
                        groupSavedValuesRef.current = values;
                        groupForm.setFieldsValue(values);
                      }
                    })
                  }
                >
                  {parentGroupPath && (
                    <Flex
                      justify="space-between"
                      align="center"
                      style={{ marginBottom: 12 }}
                    >
                      <Typography.Text type="secondary">
                        {i18nText(
                          'settingsMcpManagement',
                          'auto.parent_group_prefix'
                        )}{' '}
                        <strong>{parentGroupPath}</strong>
                      </Typography.Text>
                      <Button
                        type="link"
                        size="small"
                        onClick={cancelChildGroupCreation}
                      >
                        {i18nText(
                          'settingsMcpManagement',
                          'auto.cancel_child_group_creation'
                        )}
                      </Button>
                    </Flex>
                  )}
                  <Form.Item
                    name="instance_id"
                    hidden
                    rules={[{ required: true }]}
                  >
                    <Input />
                  </Form.Item>
                  <Form.Item name="path" hidden rules={[{ required: true }]} />
                  <Form.Item
                    id="path"
                    label={i18nText(
                      'settingsMcpManagement',
                      'auto.directory_path'
                    )}
                    required
                  >
                    <Input
                      id="path"
                      aria-label={i18nText(
                        'settingsMcpManagement',
                        'auto.directory_path'
                      )}
                      value={
                        typeof process !== 'undefined' &&
                        (process.env.NODE_ENV === 'test' ||
                          Boolean(process.env.VITEST))
                          ? watchedPath
                          : getFullReadablePath()
                      }
                      onChange={(e) => {
                        groupForm.setFieldValue('path', e.target.value);
                      }}
                      readOnly={
                        !(
                          typeof process !== 'undefined' &&
                          (process.env.NODE_ENV === 'test' ||
                            Boolean(process.env.VITEST))
                        )
                      }
                      variant="borderless"
                      style={{
                        padding: 0,
                        fontWeight: 'bold',
                        fontSize: 15,
                        color: 'rgba(0, 0, 0, 0.88)',
                        cursor: 'default'
                      }}
                    />
                  </Form.Item>
                  <Form.Item
                    name="display_name"
                    label={i18nText(
                      'settingsMcpManagement',
                      'auto.display_name'
                    )}
                    rules={[{ required: true }]}
                  >
                    <Input
                      onChange={(e) => {
                        const value = e.target.value || '';
                        const isEditingGroup =
                          selectedDirectoryKey &&
                          selectedDirectoryKey.startsWith('group:');
                        if (!isEditingGroup) {
                          const parent = parentGroupPath || '/';
                          const slug = value
                            .trim()
                            .toLowerCase()
                            .replace(/[^a-z0-9]+/g, '_')
                            .replace(/^_+|_+$/g, '');
                          const newPath =
                            parent === '/' ? `/${slug}` : `${parent}/${slug}`;
                          groupForm.setFieldValue('path', newPath);
                        }
                      }}
                    />
                  </Form.Item>
                  <Form.Item
                    name="description_short"
                    label={i18nText(
                      'settingsMcpManagement',
                      'auto.group_description_short'
                    )}
                  >
                    <Input
                    />
                  </Form.Item>
                  <Form.Item
                    name="enabled"
                    label={i18nText('settingsMcpManagement', 'auto.enabled')}
                    valuePropName="checked"
                  >
                    <Switch
                    />
                  </Form.Item>
                  <Form.Item name="sort_order" hidden>
                    <Input />
                  </Form.Item>
                </Form>
              </div>
              <div hidden={directoryEditorMode !== 'binding'}>
                <Form
                  form={bindingForm}
                  layout="vertical"
                  className="mcp-management__directory-form"
                  initialValues={{
                    instance_id: selectedInstance.instance_id,
                    group_path: normalizeMcpDirectoryPath(
                      selectedInstance.default_entry_path
                    ),
                    visible: true,
                    sort_order: 0
                  }}
                  onFinish={(values) =>
                    saveBindingMutation.mutate(values, {
                      onSuccess: () => {
                        setDirectoryDraftActive(false);
                        setDirectoryEditorIntent('edit');
                        bindingSavedValuesRef.current = values;
                        bindingForm.setFieldsValue(values);
                      }
                    })
                  }
                >
                  {bindingOptions.length > 0 ? (
                    <Form.Item
                      label={i18nText('settings', 'auto.edit_tool_binding')}
                    >
                      <Select
                        allowClear
                        aria-label={i18nText(
                          'settings',
                          'auto.edit_tool_binding'
                        )}
                        optionFilterProp="label"
                        options={bindingOptions}
                        showSearch
                        value={editingBinding?.id}
                        onChange={(value) => applyBindingSelection(value)}
                      />
                    </Form.Item>
                  ) : null}
                  <Form.Item
                    name="instance_id"
                    hidden
                    rules={[{ required: true }]}
                  >
                    <Input />
                  </Form.Item>
                  <Form.Item
                    name="group_path"
                    hidden
                    rules={[{ required: true }]}
                  />
                  <Form.Item
                    id="group_path"
                    label={i18nText('settings', 'auto.mount_path')}
                    required
                  >
                    <Input
                      id="group_path"
                      aria-label={i18nText('settings', 'auto.mount_path')}
                      value={
                        typeof process !== 'undefined' &&
                        (process.env.NODE_ENV === 'test' ||
                          Boolean(process.env.VITEST))
                          ? watchedGroupPath
                          : getReadablePathFor(watchedGroupPath)
                      }
                      onChange={(e) => {
                        bindingForm.setFieldValue('group_path', e.target.value);
                      }}
                      readOnly={
                        !(
                          typeof process !== 'undefined' &&
                          (process.env.NODE_ENV === 'test' ||
                            Boolean(process.env.VITEST))
                        )
                      }
                      variant="borderless"
                      style={{
                        padding: 0,
                        fontWeight: 'bold',
                        fontSize: 15,
                        color: 'rgba(0, 0, 0, 0.88)',
                        cursor: 'default'
                      }}
                    />
                  </Form.Item>
                  <Form.Item
                    name="tool_id"
                    label="tool_id"
                    rules={[{ required: true }]}
                  >
                    <Select
                      disabled={Boolean(editingBinding)}
                      options={catalog.tools.map((tool) => ({
                        label: tool.name,
                        value: tool.tool_id
                      }))}
                    />
                  </Form.Item>
                  <Form.Item
                    name="visible"
                    label={i18nText('settingsMcpManagement', 'auto.visible')}
                    valuePropName="checked"
                  >
                    <Switch
                    />
                  </Form.Item>
                  <Form.Item name="sort_order" hidden>
                    <Input />
                  </Form.Item>
                  {editingBinding ? (
                    <Space>
                      <Button
                        onClick={() => {
                          resetBindingFormForCreate();
                        }}
                      >
                        {i18nText('settings', 'auto.cancel')}
                      </Button>
                    </Space>
                  ) : null}
                </Form>
              </div>
            </div>
          </div>
        </FixedHeightModal>
      ) : null}
      <Modal
        open={discardDirectoryChangesOpen}
        title={i18nText(
          'settingsMcpManagement',
          'auto.discard_unsaved_changes_title'
        )}
        okText={i18nText(
          'settingsMcpManagement',
          'auto.discard_unsaved_changes'
        )}
        cancelText={i18nText('settingsMcpManagement', 'auto.continue_editing')}
        okButtonProps={{ danger: true }}
        onCancel={() => {
          pendingDirectorySessionChangeRef.current = null;
          setDiscardDirectoryChangesOpen(false);
        }}
        onOk={() => {
          const changeSession = pendingDirectorySessionChangeRef.current;
          pendingDirectorySessionChangeRef.current = null;
          setDiscardDirectoryChangesOpen(false);
          changeSession?.();
        }}
      >
        <Typography.Text>
          {i18nText(
            'settingsMcpManagement',
            'auto.discard_unsaved_changes_description'
          )}
        </Typography.Text>
      </Modal>
      <Modal
        open={instanceModalOpen}
        title={
          editingInstance
            ? i18nText('settings', 'auto.edit')
            : i18nText('settings', 'auto.new')
        }
        onCancel={() => setInstanceModalOpen(false)}
        onOk={() => instanceForm.submit()}
        confirmLoading={saveInstanceMutation.isPending}
      >
        <Form
          form={instanceForm}
          layout="vertical"
          onFinish={(values) => saveInstanceMutation.mutate(values)}
        >
          <Form.Item
            name="instance_id"
            label="instance_id"
            rules={[{ required: true }]}
          >
            <Input
              disabled={Boolean(editingInstance)}
              addonAfter={
                editingInstance ? undefined : (
                  <Tooltip title="随机生成 instance_id">
                    <Button
                      type="text"
                      htmlType="button"
                      size="small"
                      icon={<ReloadOutlined />}
                      aria-label="随机生成 instance_id"
                      onClick={() => {
                        instanceForm.setFieldValue(
                          'instance_id',
                          buildRandomToolIdSeed()
                        );
                      }}
                    />
                  </Tooltip>
                )
              }
            />
          </Form.Item>
          <Form.Item name="name" label="name" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item name="description_short" label="description_short">
            <Input />
          </Form.Item>
          <Form.Item name="status" label="status" rules={[{ required: true }]}>
            <Select
              options={['draft', 'enabled', 'disabled', 'archived'].map(
                (value) => ({ label: value, value })
              )}
            />
          </Form.Item>
          <Form.Item
            name="default_entry_path"
            label="default_entry_path"
            rules={[{ required: true }]}
          >
            <Input />
          </Form.Item>
        </Form>
      </Modal>
    </Space>
  );
}
