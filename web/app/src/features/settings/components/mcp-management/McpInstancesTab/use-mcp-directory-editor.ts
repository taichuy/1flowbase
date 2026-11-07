import { App, Form } from 'antd';
import {
  useCallback,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
  type SetStateAction
} from 'react';
import type {
  ConsoleMcpCatalog,
  ConsoleMcpInstance,
  ConsoleMcpToolBinding,
  ConsoleMcpGroup,
  ConsoleMcpInstanceDirectory
} from '@1flowbase/api-client';
import { useAuthStore } from '../../../../../state/auth-store';
import { useConsoleOperationAccess } from '../../../../../state/console-operation-access';
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
import {
  applyMcpDirectoryTreeDrop,
  buildMcpDirectoryEditorTreeData,
  type McpDirectoryTreeDropInfo
} from './directory-tree';
import { countMcpInstanceDirectoryItems } from './catalog-view';
import { useDirectoryDraft } from './use-directory-draft';
import type { GroupFormValues, BindingFormValues } from './editor-types';

/** Node editing buffers are independent of the confirmed directory draft and its persistence. */
export function useMcpDirectoryEditor(catalog: ConsoleMcpCatalog) {
  const can = useConsoleOperationAccess();
  const { message } = App.useApp();
  const csrfToken = useAuthStore((state) => state.csrfToken ?? '');
  const draft = useDirectoryDraft(csrfToken);
  const [groupForm] = Form.useForm<GroupFormValues>();
  const [bindingForm] = Form.useForm<BindingFormValues>();
  const watchedPath = Form.useWatch('path', {
    form: groupForm,
    preserve: true
  });
  Form.useWatch('path_segment', { form: groupForm, preserve: true });
  const watchedDisplayName = Form.useWatch('display_name', {
    form: groupForm,
    preserve: true
  });
  const watchedGroupDescriptionShort = Form.useWatch('description_short', {
    form: groupForm,
    preserve: true
  });
  Form.useWatch('enabled', { form: groupForm, preserve: true });
  const watchedGroupPath = Form.useWatch('group_path', {
    form: bindingForm,
    preserve: true
  });
  const watchedToolId = Form.useWatch('tool_id', {
    form: bindingForm,
    preserve: true
  });
  Form.useWatch('visible', { form: bindingForm, preserve: true });
  const [nodeEditing, setNodeEditing] = useState(false);
  const [parentGroupPath, setParentGroupPath] = useState<string | null>(null);
  const [directoryEditorIntent, setDirectoryEditorIntent] = useState<
    'create' | 'edit'
  >('edit');
  const [directoryDraftActive, setDirectoryDraftActive] = useState(false);
  const [directoryDraftVersion, setDirectoryDraftVersion] = useState(0);
  const [expandedDirectoryKeys, setExpandedDirectoryKeys] = useState<string[]>(
    []
  );
  const groupSavedValuesRef = useRef<GroupFormValues | null>(null);
  const editingGroupIdRef = useRef<string | null>(null);
  const bindingSavedValuesRef = useRef<BindingFormValues | null>(null);
  const [discardDirectoryChangesOpen, setDiscardDirectoryChangesOpen] =
    useState(false);
  const pendingDirectorySessionChangeRef = useRef<(() => void) | null>(null);
  const [instancesState, dispatch] = useReducer(
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
      dispatch({ type: 'setEditingInstance', value }),
    []
  );
  const setEditingBinding = useCallback(
    (value: SetStateAction<ConsoleMcpToolBinding | null>) =>
      dispatch({ type: 'setEditingBinding', value }),
    []
  );
  const setInstanceModalOpen = useCallback(
    (value: SetStateAction<boolean>) =>
      dispatch({ type: 'setInstanceModalOpen', value }),
    []
  );
  const setDirectoryModalOpen = useCallback(
    (value: SetStateAction<boolean>) =>
      dispatch({ type: 'setDirectoryModalOpen', value }),
    []
  );
  const setDirectoryEditorMode = useCallback(
    (value: SetStateAction<McpDirectoryEditorMode>) =>
      dispatch({ type: 'setDirectoryEditorMode', value }),
    []
  );
  const setRequestedInstanceId = useCallback(
    (value: SetStateAction<string>) =>
      dispatch({ type: 'setRequestedInstanceId', value }),
    []
  );
  const setSelectedDirectoryKey = useCallback(
    (value: SetStateAction<string>) =>
      dispatch({ type: 'setSelectedDirectoryKey', value }),
    []
  );
  const selectedInstance =
    catalog.instances.find((row) => row.instance_id === requestedInstanceId) ??
    catalog.instances[0];
  const selectedInstanceGroups = useMemo(
    () => draft.snapshot?.groups ?? [],
    [draft.snapshot]
  );
  const selectedInstanceBindings = useMemo(
    () => draft.snapshot?.bindings ?? [],
    [draft.snapshot]
  );
  const groupByPath = useMemo(
    () => new Map(selectedInstanceGroups.map((row) => [row.path, row])),
    [selectedInstanceGroups]
  );
  const bindingById = useMemo(
    () => new Map(selectedInstanceBindings.map((row) => [row.id, row])),
    [selectedInstanceBindings]
  );
  const bindingOptions = selectedInstanceBindings.map((row) => ({
    label: row.tool_id,
    value: row.id
  }));
  const { groupCounts, toolCounts } = useMemo(
    () => countMcpInstanceDirectoryItems(catalog),
    [catalog]
  );

  const directorySnapshot = draft.snapshot;
  const resolveDirectoryId = draft.resolveId;
  // Server IDs are mapped by the submitted snapshot, independent of later moves.
  useEffect(() => {
    if (!editingBinding || !directorySnapshot) return;
    const saved = directorySnapshot.bindings.find(
      (row) => row.id === resolveDirectoryId(editingBinding.id)
    );
    if (saved && saved.id !== editingBinding.id) {
      setEditingBinding(saved);
      setSelectedDirectoryKey((key) =>
        key === `binding:${editingBinding.id}` ? `binding:${saved.id}` : key
      );
    }
  }, [
    directorySnapshot,
    resolveDirectoryId,
    editingBinding,
    setEditingBinding,
    setSelectedDirectoryKey
  ]);
  useEffect(() => {
    const group =
      directorySnapshot?.groups.find(
        (row) => row.id === resolveDirectoryId(editingGroupIdRef.current ?? '')
      ) ?? directorySnapshot?.groups.find((row) => row.path === watchedPath);
    if (group) editingGroupIdRef.current = group.id;
  }, [directorySnapshot, resolveDirectoryId, watchedPath]);

  function applyDirectoryPathToForms(
    path: string,
    snapshot = draft.getSnapshot()
  ) {
    const normalized = normalizeMcpDirectoryPath(path);
    const group = snapshot?.groups.find((row) => row.path === normalized);
    editingGroupIdRef.current = group?.id ?? null;
    const values: GroupFormValues = {
      instance_id: selectedInstance?.instance_id ?? '',
      path: normalized,
      path_segment: '',
      display_name: group?.display_name ?? '',
      description_short: group?.description_short ?? null,
      enabled: group?.enabled ?? true,
      sort_order: group?.sort_order ?? 0
    };
    groupForm.resetFields();
    groupForm.setFieldsValue(values);
    groupSavedValuesRef.current = values;
    bindingForm.setFieldsValue({
      instance_id: selectedInstance?.instance_id ?? '',
      group_path: normalized
    });
  }
  function applyBindingSelection(bindingId?: string) {
    const binding = draft
      .getSnapshot()
      ?.bindings.find((row) => row.id === bindingId);
    if (!binding) return;
    setDirectoryEditorMode('binding');
    setDirectoryEditorIntent('edit');
    setDirectoryDraftActive(false);
    setParentGroupPath(null);
    setNodeEditing(false);
    setEditingBinding(binding);
    setSelectedDirectoryKey(`binding:${binding.id}`);
    const values = {
      instance_id: selectedInstance?.instance_id ?? '',
      group_path: binding.group_path,
      tool_id: binding.tool_id,
      visible: binding.visible,
      sort_order: binding.sort_order
    };
    bindingForm.resetFields();
    bindingForm.setFieldsValue(values);
    bindingSavedValuesRef.current = values;
  }
  function selectDirectoryNode(key: string) {
    setSelectedDirectoryKey(key);
    setNodeEditing(false);
    setDirectoryDraftActive(false);
    setParentGroupPath(null);
    setDirectoryEditorIntent('edit');
    if (key.startsWith('binding:')) {
      applyBindingSelection(key.slice('binding:'.length));
      return;
    }
    setEditingBinding(null);
    setDirectoryEditorMode('group');
    applyDirectoryPathToForms(
      key.startsWith('group:')
        ? key.slice('group:'.length)
        : (selectedInstance?.default_entry_path ?? '/')
    );
  }
  async function openDirectory(instance: ConsoleMcpInstance) {
    setRequestedInstanceId(instance.instance_id);
    setDirectoryModalOpen(true);
    setEditingBinding(null);
    setDirectoryEditorMode('group');
    setDirectoryEditorIntent('edit');
    setDirectoryDraftActive(false);
    setNodeEditing(false);
    setParentGroupPath(null);
    setExpandedDirectoryKeys([]);
    setSelectedDirectoryKey(
      `instance:${instance.instance_id}:${normalizeMcpDirectoryPath(instance.default_entry_path)}`
    );
    groupForm.resetFields();
    bindingForm.resetFields();
    const snapshot = await draft.load(instance.instance_id);
    if (snapshot) {
      const path = normalizeMcpDirectoryPath(instance.default_entry_path);
      const group = snapshot.groups.find((row) => row.path === path);
      editingGroupIdRef.current = group?.id ?? null;
      const values = {
        instance_id: instance.instance_id,
        path,
        path_segment: '',
        display_name: group?.display_name ?? '',
        description_short: group?.description_short ?? null,
        enabled: group?.enabled ?? true,
        sort_order: group?.sort_order ?? 0
      };
      groupForm.setFieldsValue(values);
      groupSavedValuesRef.current = values;
      bindingForm.setFieldsValue({
        instance_id: instance.instance_id,
        group_path: path,
        visible: true,
        sort_order: 0
      });
      bindingSavedValuesRef.current = bindingForm.getFieldsValue(true);
    }
  }
  function editCurrentNode() {
    if (can('mcp.directory.replace') && draft.snapshot) setNodeEditing(true);
  }
  function cancelNodeEdit() {
    if (directoryEditorIntent === 'create') {
      setDirectoryDraftActive(false);
      selectDirectoryNode(
        `group:${parentGroupPath ?? selectedDirectoryPath()}`
      );
      return;
    }
    selectDirectoryNode(selectedDirectoryKey);
  }
  const directorySessionHasChanges = () => {
    if (!nodeEditing) return false;
    if (directoryEditorMode === 'group') {
      const project = (v: Partial<GroupFormValues> | null) => [
        v?.path ?? '/',
        v?.path_segment ?? '',
        v?.display_name ?? '',
        v?.description_short ?? null,
        v?.enabled ?? true,
        v?.sort_order ?? 0
      ];
      return (
        JSON.stringify(project(groupForm.getFieldsValue(true))) !==
        JSON.stringify(project(groupSavedValuesRef.current))
      );
    }
    const project = (v: Partial<BindingFormValues> | null) => [
      v?.group_path ?? '/',
      v?.tool_id ?? '',
      v?.visible ?? true,
      v?.sort_order ?? 0
    ];
    return (
      JSON.stringify(project(bindingForm.getFieldsValue(true))) !==
      JSON.stringify(project(bindingSavedValuesRef.current))
    );
  };
  function requestDirectorySessionChange(
    action: () => void,
    wholeTree = false
  ) {
    if (!directorySessionHasChanges() && !(wholeTree && draft.hasChanges)) {
      action();
      return;
    }
    pendingDirectorySessionChangeRef.current = action;
    setDiscardDirectoryChangesOpen(true);
  }
  function closeDirectoryModal() {
    if (draft.saveDirectoryMutation.isPending) return;
    requestDirectorySessionChange(() => {
      draft.clear();
      setDirectoryModalOpen(false);
      setNodeEditing(false);
      setDirectoryDraftActive(false);
      setEditingBinding(null);
      groupForm.resetFields();
      bindingForm.resetFields();
    }, true);
  }
  function changeDirectoryInstance(instanceId: string) {
    const instance = catalog.instances.find(
      (row) => row.instance_id === instanceId
    );
    if (instance && !draft.saveDirectoryMutation.isPending)
      requestDirectorySessionChange(() => {
        void openDirectory(instance);
      }, true);
  }
  function selectedDirectoryPath() {
    if (selectedDirectoryKey.startsWith('group:'))
      return normalizeMcpDirectoryPath(
        selectedDirectoryKey.slice('group:'.length)
      );
    if (selectedDirectoryKey.startsWith('binding:'))
      return normalizeMcpDirectoryPath(
        draft
          .getSnapshot()
          ?.bindings.find(
            (row) => row.id === selectedDirectoryKey.slice('binding:'.length)
          )?.group_path ?? editingBinding?.group_path
      );
    return normalizeMcpDirectoryPath(selectedInstance?.default_entry_path);
  }
  const groupCreationPathPrefix = () =>
    buildMcpChildGroupPath(parentGroupPath ?? selectedDirectoryPath(), '');
  const groupCreationPath = (segment: string) =>
    buildMcpChildGroupPath(parentGroupPath ?? selectedDirectoryPath(), segment);
  function expandDirectoryPath(path: string) {
    if (!selectedInstance) return;
    const keys = [
      `instance:${selectedInstance.instance_id}:${normalizeMcpDirectoryPath(selectedInstance.default_entry_path)}`
    ];
    let current = '';
    for (const segment of path.split('/').filter(Boolean)) {
      current += `/${segment}`;
      keys.push(`group:${current}`);
    }
    setExpandedDirectoryKeys((old) => [...new Set([...old, ...keys])]);
  }
  function startChildGroupCreation(path = selectedDirectoryPath()) {
    expandDirectoryPath(path);
    setDirectoryEditorMode('group');
    setDirectoryEditorIntent('create');
    setDirectoryDraftActive(true);
    setDirectoryDraftVersion((value) => value + 1);
    setNodeEditing(true);
    setEditingBinding(null);
    setParentGroupPath(path);
    const values = {
      instance_id: selectedInstance?.instance_id ?? '',
      path,
      path_segment: '',
      display_name: '',
      description_short: null,
      enabled: true,
      sort_order: 0
    };
    groupForm.resetFields();
    groupForm.setFieldsValue(values);
    groupSavedValuesRef.current = values;
  }
  function startToolMount(path = selectedDirectoryPath()) {
    expandDirectoryPath(path);
    setDirectoryEditorMode('binding');
    setDirectoryEditorIntent('create');
    setDirectoryDraftActive(true);
    setDirectoryDraftVersion((value) => value + 1);
    setNodeEditing(true);
    setEditingBinding(null);
    setParentGroupPath(null);
    const values = {
      instance_id: selectedInstance?.instance_id ?? '',
      group_path: path,
      tool_id: '',
      visible: true,
      sort_order: 0
    };
    bindingForm.resetFields();
    bindingForm.setFieldsValue(values);
    bindingSavedValuesRef.current = values;
  }
  function validateSnapshot(snapshot: ConsoleMcpInstanceDirectory) {
    if (
      new Set(snapshot.groups.map((row) => row.path)).size !==
        snapshot.groups.length ||
      new Set(
        snapshot.bindings.map((row) => `${row.group_path}/${row.tool_id}`)
      ).size !== snapshot.bindings.length
    ) {
      throw new Error(
        i18nText('settingsMcpManagement', 'auto.directory_duplicate_path')
      );
    }
  }
  function stage(
    transform: (
      snapshot: ConsoleMcpInstanceDirectory
    ) => ConsoleMcpInstanceDirectory
  ) {
    if (!can('mcp.directory.replace')) return false;
    try {
      draft.update((snapshot) => {
        const next = transform(snapshot);
        validateSnapshot(next);
        return next;
      });
      return true;
    } catch (error) {
      message.error(error instanceof Error ? error.message : String(error));
      return false;
    }
  }
  function saveGroup(values: GroupFormValues) {
    if (!nodeEditing || !draft.snapshot) return;
    const old =
      directoryEditorIntent === 'edit'
        ? (draft.snapshot.groups.find(
            (row) => row.id === editingGroupIdRef.current
          ) ?? groupByPath.get(values.path))
        : groupByPath.get(values.path);
    if (directoryEditorIntent === 'create' && old) {
      message.error(
        i18nText('settingsMcpManagement', 'auto.directory_duplicate_path')
      );
      return;
    }
    const saved: ConsoleMcpGroup = {
      id: old?.id ?? `draft:${crypto.randomUUID()}`,
      instance_record_id: draft.snapshot.instance_record_id,
      path:
        directoryEditorIntent === 'edit' && old
          ? old.path
          : normalizeMcpDirectoryPath(values.path),
      display_name:
        values.display_name?.trim() ||
        values.path.split('/').filter(Boolean).at(-1) ||
        '/',
      description_short: values.description_short,
      enabled: values.enabled,
      sort_order: old?.sort_order ?? values.sort_order
    };
    if (
      !stage((snapshot) => ({
        ...snapshot,
        groups: [...snapshot.groups.filter((row) => row.id !== saved.id), saved]
      }))
    )
      return;
    setSelectedDirectoryKey(`group:${saved.path}`);
    setParentGroupPath(null);
    setDirectoryEditorIntent('edit');
    setDirectoryDraftActive(false);
    setNodeEditing(false);
    applyDirectoryPathToForms(saved.path);
  }
  function saveBinding(values: BindingFormValues) {
    if (!nodeEditing || !draft.snapshot) return;
    const old =
      editingBinding &&
      (draft.snapshot.bindings.find(
        (row) => row.id === draft.resolveId(editingBinding.id)
      ) ??
        draft.snapshot.bindings.find(
          (row) =>
            row.tool_id === editingBinding.tool_id &&
            row.group_path === editingBinding.group_path
        ));
    const tool = catalog.tools.find((row) => row.tool_id === values.tool_id);
    if (!tool) return;
    const saved: ConsoleMcpToolBinding = {
      id: old?.id ?? `draft:${crypto.randomUUID()}`,
      instance_record_id: draft.snapshot.instance_record_id,
      tool_record_id: tool.id,
      tool_id: values.tool_id,
      group_path:
        old?.group_path ?? normalizeMcpDirectoryPath(values.group_path),
      display_alias: old?.display_alias ?? null,
      visible: values.visible,
      sort_order: old?.sort_order ?? values.sort_order
    };
    if (
      !stage((snapshot) => ({
        ...snapshot,
        bindings: [
          ...snapshot.bindings.filter((row) => row.id !== saved.id),
          saved
        ]
      }))
    )
      return;
    applyBindingSelection(saved.id);
  }
  function deleteGroup(path: string) {
    const under = (value: string) =>
      value === path || value.startsWith(`${path}/`);
    if (
      stage((snapshot) => ({
        ...snapshot,
        groups: snapshot.groups.filter((row) => !under(row.path)),
        bindings: snapshot.bindings.filter((row) => !under(row.group_path))
      }))
    )
      selectDirectoryNode(`instance:${selectedInstance?.instance_id}:/`);
  }
  function deleteBinding(id: string) {
    if (
      stage((snapshot) => ({
        ...snapshot,
        bindings: snapshot.bindings.filter((row) => row.id !== id)
      }))
    )
      selectDirectoryNode(`group:${selectedDirectoryPath()}`);
  }
  function handleTreeDrop(info: McpDirectoryTreeDropInfo) {
    if (!selectedInstance || !can('mcp.directory.replace')) return;
    applyMcpDirectoryTreeDrop({
      info,
      selectedInstance,
      selectedInstanceBindings,
      selectedInstanceGroups,
      bindingById,
      groupByPath,
      onMoveBinding: (id, values, options) => {
        if (
          stage((snapshot) => ({
            ...snapshot,
            bindings: snapshot.bindings.map((row) =>
              row.id === id ? { ...row, ...values } : row
            )
          }))
        )
          options.onSuccess();
      },
      onMoveGroup: (values, options) => {
        const old = groupByPath.get(values.sourcePath);
        if (!old) return;
        const target = buildMcpChildGroupPath(
          values.targetParentPath,
          values.sourcePath.split('/').filter(Boolean).at(-1)!
        );
        const move = (path: string) =>
          path === values.sourcePath || path.startsWith(`${values.sourcePath}/`)
            ? target + path.slice(values.sourcePath.length)
            : path;
        const moved = { ...old, path: target, sort_order: values.sortOrder };
        if (
          stage((snapshot) => ({
            ...snapshot,
            groups: snapshot.groups.map((row) =>
              row.id === old.id ? moved : { ...row, path: move(row.path) }
            ),
            bindings: snapshot.bindings.map((row) => ({
              ...row,
              group_path: move(row.group_path)
            }))
          }))
        )
          options.onSuccess(moved);
      },
      onSelectKey: (key) => {
        if (!nodeEditing) selectDirectoryNode(key);
      }
    });
    if (nodeEditing) {
      const snapshot = draft.getSnapshot();
      if (editingBinding) {
        const row = snapshot?.bindings.find(
          (value) => value.id === draft.resolveId(editingBinding.id)
        );
        if (row) {
          bindingForm.setFieldValue('group_path', row.group_path);
          if (bindingSavedValuesRef.current)
            bindingSavedValuesRef.current = {
              ...bindingSavedValuesRef.current,
              group_path: row.group_path
            };
        }
      } else if (editingGroupIdRef.current) {
        const row = snapshot?.groups.find(
          (value) => value.id === editingGroupIdRef.current
        );
        if (row) {
          groupForm.setFieldValue('path', row.path);
          if (groupSavedValuesRef.current)
            groupSavedValuesRef.current = {
              ...groupSavedValuesRef.current,
              path: row.path
            };
          setSelectedDirectoryKey(`group:${row.path}`);
        }
      }
    }
  }
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
  return {
    groupForm,
    bindingForm,
    watchedGroupPath,
    watchedToolId,
    watchedDisplayName,
    watchedGroupDescriptionShort,
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
    groupCounts,
    toolCounts,
    selectedInstance,
    bindingOptions,
    treeData,
    handleTreeDrop,
    applyDirectoryPathToForms,
    applyBindingSelection,
    directorySessionHasChanges,
    requestDirectorySessionChange,
    closeDirectoryModal,
    selectedDirectoryPath,
    groupCreationPathPrefix,
    groupCreationPath,
    startChildGroupCreation,
    startToolMount,
    openDirectory,
    changeDirectoryInstance,
    selectDirectoryNode,
    editCurrentNode,
    cancelNodeEdit,
    saveGroup,
    saveBinding,
    deleteGroup,
    deleteBinding,
    nodeEditing,
    directorySnapshot: draft.snapshot,
    directoryHasChanges: draft.hasChanges,
    directoryLoading: draft.loading,
    directoryError: draft.error,
    saveDirectoryMutation: draft.saveDirectoryMutation,
    canEditNode: Boolean(
      draft.snapshot &&
      (editingBinding ||
        groupByPath.has(normalizeMcpDirectoryPath(watchedPath)))
    )
  };
}
