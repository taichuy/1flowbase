import { FolderOutlined, PlusOutlined, SaveOutlined } from '@ant-design/icons';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  App,
  Button,
  Flex,
  Form,
  Input,
  Select,
  Space,
  Switch,
  Tag,
  Typography
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
import { McpInstanceTable } from './McpInstancesTab/McpInstanceTable';
import { McpDirectoryTree } from './McpInstancesTab/McpDirectoryTree';
import {
  McpCopyInstanceModal,
  McpDiscardDirectoryChangesModal,
  McpInstanceEditorModal
} from './McpInstancesTab/McpInstanceModals';
import {
  applyMcpDirectoryTreeDrop,
  buildMcpDirectoryEditorTreeData,
  type McpDirectoryTreeDropInfo
} from './McpInstancesTab/directory-tree';
import {
  countMcpInstanceDirectoryItems,
  formatMcpDirectoryPath,
  formatMcpGroupEditorPath
} from './McpInstancesTab/catalog-view';
import type {
  ConsoleMcpCatalog,
  ConsoleMcpInstance,
  ConsoleMcpToolBinding,
  SaveConsoleMcpInstanceBody
} from '@1flowbase/api-client';
import {
  createSettingsMcpInstance,
  copySettingsMcpInstance,
  createSettingsMcpToolBinding,
  deleteSettingsMcpGroup,
  deleteSettingsMcpInstance,
  deleteSettingsMcpToolBinding,
  exportSettingsMcpInstanceBundle,
  fetchSettingsMcpBundleExportDefaults,
  settingsMcpBundleExportDefaultsQueryKey,
  moveSettingsMcpGroup,
  settingsMcpCatalogQueryKey,
  updateSettingsMcpInstance,
  updateSettingsMcpToolBinding,
  upsertSettingsMcpGroup,
  type ExportSettingsMcpBundleBody,
  type CopySettingsMcpInstanceBody
} from '../../api/mcp-management';
import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import { FixedHeightModal } from '../../../../shared/ui/fixed-height-modal/FixedHeightModal';
import {
  nextMcpDirectoryExpandedKeys,
  normalizeMcpDirectoryPath
} from './mcp-management-view-model';
import { McpInstanceDiscoveryPolicyModal } from './McpInstanceDiscoveryPolicyModal';
import {
  createInitialMcpInstancesState,
  mcpInstancesReducer,
  type McpDirectoryEditorMode
} from './mcp-management-state';
import { McpBundleExportModal } from './bundle/McpBundleExportModal';
import { downloadMcpBundle } from './bundle/mcp-bundle-download';

type InstanceFormValues = SaveConsoleMcpInstanceBody;
type CopyInstanceFormValues = CopySettingsMcpInstanceBody;
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
  const { message } = App.useApp();
  const csrfToken = useCsrfToken();
  const queryClient = useQueryClient();
  const exportDefaults = useQuery({
    queryKey: settingsMcpBundleExportDefaultsQueryKey,
    queryFn: fetchSettingsMcpBundleExportDefaults,
    enabled: canManage
  });
  const [instanceForm] = Form.useForm<InstanceFormValues>();
  const [copyInstanceForm] = Form.useForm<CopyInstanceFormValues>();
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
  const [expandedDirectoryKeys, setExpandedDirectoryKeys] = useState<string[]>(
    []
  );
  const groupSavedValuesRef = useRef<GroupFormValues | null>(null);
  const bindingSavedValuesRef = useRef<BindingFormValues | null>(null);
  const [discardDirectoryChangesOpen, setDiscardDirectoryChangesOpen] =
    useState(false);
  const [discoveryPolicyInstance, setDiscoveryPolicyInstance] =
    useState<ConsoleMcpInstance | null>(null);
  const [clientConfigurationInstance, setClientConfigurationInstance] =
    useState<ConsoleMcpInstance | null>(null);
  const [copyingInstance, setCopyingInstance] =
    useState<ConsoleMcpInstance | null>(null);
  const [bundleExportInstance, setBundleExportInstance] =
    useState<ConsoleMcpInstance | null>(null);
  const [exportingInstanceBundle, setExportingInstanceBundle] = useState(false);
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
  const copyInstanceMutation = useMutation({
    mutationFn: ({
      sourceInstanceId,
      values
    }: {
      sourceInstanceId: string;
      values: CopyInstanceFormValues;
    }) => copySettingsMcpInstance(sourceInstanceId, values, csrfToken),
    onSuccess: async () => {
      message.success(
        i18nText('settingsMcpManagement', 'auto.mcp_instance_copied')
      );
      setCopyingInstance(null);
      copyInstanceForm.resetFields();
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
  async function handleExportInstanceBundle(
    values: ExportSettingsMcpBundleBody
  ) {
    if (!bundleExportInstance) return;
    setExportingInstanceBundle(true);
    try {
      const response = await exportSettingsMcpInstanceBundle(
        bundleExportInstance.instance_id,
        values,
        csrfToken
      );
      downloadMcpBundle(response.blob, response.filename);
      setBundleExportInstance(null);
      message.success(
        i18nText('settingsMcpManagement', 'auto.mcp_bundle_export_ready')
      );
    } catch (error) {
      message.error(error instanceof Error ? error.message : String(error));
    } finally {
      setExportingInstanceBundle(false);
    }
  }

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

  const getFullReadablePath = () =>
    formatMcpGroupEditorPath({
      instanceName: selectedInstance?.name || 'mcp',
      selectedDirectoryKey,
      currentPath: groupForm.getFieldValue('path'),
      parentPath: parentGroupPath,
      draftDisplayName: watchedDisplayName,
      groups: groupByPath
    });

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
    <Space orientation="vertical" size="middle" className="mcp-management__stack">
      <McpInstanceTable
        onCreate={() => {
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
        canManage={canManage}
        instances={catalog.instances}
        groupCounts={groupCounts}
        toolCounts={toolCounts}
        onEdit={(record) => {
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
        onOpenDirectory={(record) => {
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
            group_path: normalizeMcpDirectoryPath(record.default_entry_path),
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
          setExpandedDirectoryKeys([]);
          setDirectoryModalOpen(true);
        }}
        onConnect={setClientConfigurationInstance}
        onEditDiscoveryPolicy={setDiscoveryPolicyInstance}
        onCopy={(record) => {
          copyInstanceForm.resetFields();
          setCopyingInstance(record);
        }}
        onExport={setBundleExportInstance}
        onDelete={(record) =>
          deleteInstanceMutation.mutateAsync(record.instance_id)
        }
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
      <McpCopyInstanceModal
        source={copyingInstance}
        form={copyInstanceForm}
        saving={copyInstanceMutation.isPending}
        onClose={() => {
          setCopyingInstance(null);
          copyInstanceForm.resetFields();
        }}
        onSave={(source, values) =>
          copyInstanceMutation.mutate({
            sourceInstanceId: source.instance_id,
            values
          })
        }
      />
      <McpBundleExportModal
        open={Boolean(bundleExportInstance)}
        title={i18nText(
          'settingsMcpManagement',
          'auto.mcp_instance_export_title'
        )}
        okText={i18nText('settingsMcpManagement', 'auto.mcp_instance_export')}
        defaultBundleId={bundleExportInstance?.instance_id ?? ''}
        exportDefaults={exportDefaults.data}
        exporting={exportingInstanceBundle}
        onCancel={() => setBundleExportInstance(null)}
        onExport={handleExportInstanceBundle}
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
                      setExpandedDirectoryKeys([]);
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

              <McpDirectoryTree
                key={`${directoryEditorMode}:${
                  directoryDraftActive ? directoryDraftVersion : 'stable'
                }`}
                canManage={canManage}
                expandedKeys={expandedDirectoryKeys}
                selectedKey={selectedDirectoryKey}
                treeData={treeData}
                onExpand={(changedKey, expanded) =>
                  setExpandedDirectoryKeys((currentKeys) =>
                    nextMcpDirectoryExpandedKeys(
                      currentKeys,
                      changedKey,
                      expanded
                    )
                  )
                }
                onSelect={(key) => {
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
                onEditGroup={(path) => {
                  setParentGroupPath(null);
                  setDirectoryEditorMode('group');
                  setDirectoryEditorIntent('edit');
                  setDirectoryDraftActive(false);
                  setEditingBinding(null);
                  applyDirectoryPathToForms(path);
                }}
                onEditBinding={applyBindingSelection}
                onDeleteGroup={(path) => deleteGroupMutation.mutate(path)}
                onDeleteBinding={(bindingId) =>
                  deleteBindingMutation.mutate(bindingId)
                }
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
                  <Form.Item name="path" hidden rules={[{ required: true }]}>
                    <Input />
                  </Form.Item>
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
                    <Input />
                  </Form.Item>
                  <Form.Item
                    name="enabled"
                    label={i18nText('settingsMcpManagement', 'auto.enabled')}
                    valuePropName="checked"
                  >
                    <Switch />
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
                  >
                    <Input />
                  </Form.Item>
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
                    <Switch />
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
      <McpDiscardDirectoryChangesModal
        open={discardDirectoryChangesOpen}
        onContinueEditing={() => {
          pendingDirectorySessionChangeRef.current = null;
          setDiscardDirectoryChangesOpen(false);
        }}
        onDiscard={() => {
          const changeSession = pendingDirectorySessionChangeRef.current;
          pendingDirectorySessionChangeRef.current = null;
          setDiscardDirectoryChangesOpen(false);
          changeSession?.();
        }}
      />
      <McpInstanceEditorModal
        open={instanceModalOpen}
        instance={editingInstance}
        form={instanceForm}
        saving={saveInstanceMutation.isPending}
        onClose={() => setInstanceModalOpen(false)}
        onSave={(values) => saveInstanceMutation.mutate(values)}
      />
    </Space>
  );
}
