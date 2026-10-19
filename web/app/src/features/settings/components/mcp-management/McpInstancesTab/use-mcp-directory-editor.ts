import { useMutation, useQueryClient } from '@tanstack/react-query';
import { App, Form } from 'antd';
import {
  useCallback,
  useMemo,
  useReducer,
  useRef,
  useState,
  type SetStateAction
} from 'react';

import {
  applyMcpDirectoryTreeDrop,
  buildMcpDirectoryEditorTreeData,
  type McpDirectoryTreeDropInfo
} from './directory-tree';
import {
  countMcpInstanceDirectoryItems,
  formatMcpDirectoryPath
} from './catalog-view';
import type {
  ConsoleMcpCatalog,
  ConsoleMcpInstance,
  ConsoleMcpToolBinding
} from '@1flowbase/api-client';
import {
  createSettingsMcpToolBinding,
  deleteSettingsMcpGroup,
  deleteSettingsMcpToolBinding,
  moveSettingsMcpGroup,
  settingsMcpCatalogQueryKey,
  updateSettingsMcpToolBinding,
  upsertSettingsMcpGroup
} from '../../../api/mcp-management';
import { useAuthStore } from '../../../../../state/auth-store';
import { i18nText } from '../../../../../shared/i18n/text';

import {
  buildMcpChildGroupPath,
  normalizeMcpDirectoryPath
} from '../mcp-management-view-model';

import {
  createInitialMcpInstancesState,
  mcpInstancesReducer,
  type McpDirectoryEditorMode
} from '../mcp-management-state';

import type { GroupFormValues, BindingFormValues } from './editor-types';

/** Owns group/binding draft sessions, dirty guards, tree selection/drop and catalog mutations. */
export function useMcpDirectoryEditor(catalog: ConsoleMcpCatalog) {
  const { message } = App.useApp();
  const csrfToken = useAuthStore((state) => state.csrfToken ?? '');
  const queryClient = useQueryClient();

  const [groupForm] = Form.useForm<GroupFormValues>();
  const [bindingForm] = Form.useForm<BindingFormValues>();

  const watchedPath = Form.useWatch('path', groupForm);
  Form.useWatch('path_segment', groupForm);
  const watchedDisplayName = Form.useWatch('display_name', groupForm);
  const watchedGroupDescriptionShort = Form.useWatch(
    'description_short',
    groupForm
  );
  Form.useWatch('enabled', groupForm);

  const watchedGroupPath = Form.useWatch('group_path', bindingForm);
  const watchedToolId = Form.useWatch('tool_id', bindingForm);
  Form.useWatch('visible', bindingForm);

  const [parentGroupPath, setParentGroupPath] = useState<string | null>(null);
  const [directoryEditorIntent, setDirectoryEditorIntent] = useState<
    'create' | 'edit'
  >('create');
  const [directoryDraftActive, setDirectoryDraftActive] = useState(false);
  const [directoryDraftVersion, setDirectoryDraftVersion] = useState(0);
  const [expandedDirectoryKeys, setExpandedDirectoryKeys] = useState<string[]>(
    []
  );
  const groupSavedValuesRef = useRef<GroupFormValues | null>(null);
  const bindingSavedValuesRef = useRef<BindingFormValues | null>(null);
  const [discardDirectoryChangesOpen, setDiscardDirectoryChangesOpen] =
    useState(false);
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
  const saveGroupMutation = useMutation({
    mutationFn: (values: GroupFormValues) =>
      upsertSettingsMcpGroup(
        values.instance_id,
        {
          path: values.path,
          display_name: values.display_name?.trim() || null,
          description_short: values.description_short,
          enabled: values.enabled,
          sort_order: Number(values.sort_order)
        },
        csrfToken
      ),
    onSuccess: async (savedGroup) => {
      queryClient.setQueryData<ConsoleMcpCatalog>(
        settingsMcpCatalogQueryKey,
        (currentCatalog) => {
          if (!currentCatalog || !savedGroup) return currentCatalog;
          return {
            ...currentCatalog,
            groups: [
              ...currentCatalog.groups.filter(
                (group) =>
                  group.id !== savedGroup.id &&
                  !(
                    group.instance_record_id ===
                      savedGroup.instance_record_id &&
                    normalizeMcpDirectoryPath(group.path) ===
                      normalizeMcpDirectoryPath(savedGroup.path)
                  )
              ),
              savedGroup
            ]
          };
        }
      );
      message.success(i18nText('settings', 'auto.mcp_saved'));
      groupForm.resetFields();
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    },
    onError: (error) => {
      message.error(error instanceof Error ? error.message : String(error));
    }
  });
  const moveGroupMutation = useMutation({
    mutationFn: ({
      instanceId,
      sourcePath,
      targetParentPath,
      sortOrder
    }: {
      instanceId: string;
      sourcePath: string;
      targetParentPath: string;
      sortOrder: number;
    }) =>
      moveSettingsMcpGroup(
        instanceId,
        sourcePath,
        targetParentPath,
        sortOrder,
        csrfToken
      ),
    onSuccess: async () => {
      message.success(i18nText('settings', 'auto.mcp_saved'));
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

  const { groupCounts, toolCounts } = useMemo(
    () => countMcpInstanceDirectoryItems(catalog),
    [catalog]
  );
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
  const treeData = useMemo(
    () =>
      buildMcpDirectoryEditorTreeData({
        selectedInstance,
        selectedInstanceGroups,
        selectedInstanceBindings,
        tools: catalog.tools,
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
      }),
    [
      selectedInstance,
      selectedInstanceGroups,
      selectedInstanceBindings,
      catalog.tools,
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
    ]
  );

  const handleTreeDrop = (info: McpDirectoryTreeDropInfo) => {
    if (!selectedInstance) return;
    applyMcpDirectoryTreeDrop({
      info,
      selectedInstance,
      selectedInstanceBindings,
      selectedInstanceGroups,
      bindingById,
      groupByPath,
      onSaveBinding: (values, options) =>
        saveBindingMutation.mutate(values, options),
      onMoveGroup: (values, options) =>
        moveGroupMutation.mutate(values, options),
      onSelectKey: setSelectedDirectoryKey
    });
  };

  function applyDirectoryPathToForms(path: string) {
    const normalizedPath = normalizeMcpDirectoryPath(path);
    const group = groupByPath.get(normalizedPath);

    groupForm.setFieldsValue({
      instance_id: selectedInstance?.instance_id ?? '',
      path: normalizedPath,
      path_segment: '',
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

  const getReadablePathFor = (rawPath: string | null | undefined) =>
    formatMcpDirectoryPath(
      selectedInstance?.name || 'mcp',
      rawPath,
      groupByPath
    );

  const discardDirectorySession = () => {
    setDirectoryModalOpen(false);
    setEditingBinding(null);
    setSelectedDirectoryKey('');
    setParentGroupPath(null);
    setDirectoryDraftActive(false);
    setExpandedDirectoryKeys([]);
    groupForm.resetFields();
    bindingForm.resetFields();
  };

  const directorySessionHasChanges = () => {
    if (directoryEditorMode === 'group') {
      const savedValues = groupSavedValuesRef.current;
      if (!savedValues) return false;
      const currentValues = groupForm.getFieldsValue(true);
      return (
        normalizeMcpDirectoryPath(currentValues.path) !==
          normalizeMcpDirectoryPath(savedValues.path) ||
        (currentValues.path_segment ?? '') !==
          (savedValues.path_segment ?? '') ||
        (currentValues.display_name ?? '') !==
          (savedValues.display_name ?? '') ||
        (currentValues.description_short ?? null) !==
          (savedValues.description_short ?? null) ||
        currentValues.enabled !== savedValues.enabled
      );
    }

    const savedValues = bindingSavedValuesRef.current;
    if (!savedValues) return false;
    const currentValues = bindingForm.getFieldsValue(true);
    return (
      normalizeMcpDirectoryPath(currentValues.group_path) !==
        normalizeMcpDirectoryPath(savedValues.group_path) ||
      currentValues.tool_id !== savedValues.tool_id ||
      currentValues.visible !== savedValues.visible
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

  const groupCreationParentPath = () =>
    normalizeMcpDirectoryPath(parentGroupPath ?? selectedDirectoryPath());

  const groupCreationPathPrefix = () => {
    return buildMcpChildGroupPath(groupCreationParentPath(), '');
  };

  const groupCreationPath = (pathSegment: string) =>
    buildMcpChildGroupPath(groupCreationParentPath(), pathSegment);

  const expandDirectoryPath = (path: string) => {
    if (!selectedInstance) return;
    const rootKey = `instance:${selectedInstance.instance_id}:${normalizeMcpDirectoryPath(
      selectedInstance.default_entry_path
    )}`;
    const keys = [rootKey];
    const segments = normalizeMcpDirectoryPath(path).split('/').filter(Boolean);
    let currentPath = '';
    for (const segment of segments) {
      currentPath += `/${segment}`;
      keys.push(`group:${currentPath}`);
    }
    setExpandedDirectoryKeys((currentKeys) =>
      Array.from(new Set([...currentKeys, ...keys]))
    );
  };

  const startChildGroupCreation = (path?: string) => {
    const currentPath = normalizeMcpDirectoryPath(
      path ?? selectedDirectoryPath()
    );

    expandDirectoryPath(currentPath);
    setDirectoryEditorMode('group');
    setDirectoryEditorIntent('create');
    setDirectoryDraftActive(true);
    setDirectoryDraftVersion((version) => version + 1);
    setEditingBinding(null);
    setParentGroupPath(currentPath);
    groupForm.resetFields();
    groupForm.setFieldsValue({
      instance_id: selectedInstance?.instance_id ?? '',
      path: currentPath,
      path_segment: '',
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
    expandDirectoryPath(targetPath);
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
      path_segment: '',
      display_name: '',
      description_short: null,
      enabled: true,
      sort_order: 0
    });
  };
  return {
    groupForm,
    bindingForm,
    watchedGroupPath,
    parentGroupPath,
    setParentGroupPath,
    directoryEditorIntent,
    setDirectoryEditorIntent,
    directoryDraftActive,
    setDirectoryDraftActive,
    directoryDraftVersion,
    expandedDirectoryKeys,
    setExpandedDirectoryKeys,
    groupSavedValuesRef,
    bindingSavedValuesRef,
    discardDirectoryChangesOpen,
    setDiscardDirectoryChangesOpen,
    pendingDirectorySessionChangeRef,
    editingInstance,
    editingBinding,
    instanceModalOpen,
    directoryModalOpen,
    directoryEditorMode,
    selectedDirectoryKey,
    setEditingInstance,
    setEditingBinding,
    setInstanceModalOpen,
    setDirectoryModalOpen,
    setDirectoryEditorMode,
    setRequestedInstanceId,
    setSelectedDirectoryKey,
    saveGroupMutation,
    saveBindingMutation,
    deleteGroupMutation,
    deleteBindingMutation,
    groupCounts,
    toolCounts,
    selectedInstance,
    bindingOptions,
    treeData,
    handleTreeDrop,
    applyDirectoryPathToForms,
    resetBindingFormForCreate,
    applyBindingSelection,
    getReadablePathFor,
    directorySessionHasChanges,
    requestDirectorySessionChange,
    closeDirectoryModal,
    selectedDirectoryPath,
    groupCreationPathPrefix,
    groupCreationPath,
    startChildGroupCreation,
    startToolMount,
    cancelChildGroupCreation
  };
}
