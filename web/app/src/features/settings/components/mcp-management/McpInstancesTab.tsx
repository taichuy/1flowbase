import { useQueryClient } from '@tanstack/react-query';
import { useAuthStore } from '../../../../state/auth-store';
import FolderOutlined from '@ant-design/icons/es/icons/FolderOutlined';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import SaveOutlined from '@ant-design/icons/es/icons/SaveOutlined';

import {
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

import { McpClientConfigurationModal } from './McpClientConfigurationModal';
import { McpInstanceTable } from './McpInstancesTab/McpInstanceTable';
import { McpDirectoryTree } from './McpInstancesTab/McpDirectoryTree';
import {
  McpCopyInstanceModal,
  McpDiscardDirectoryChangesModal,
  McpInstanceEditorModal
} from './McpInstancesTab/McpInstanceModals';

import type { ConsoleMcpCatalog } from '@1flowbase/api-client';
import { settingsMcpCatalogQueryKey } from '../../api/mcp-management';

import { i18nText } from '../../../../shared/i18n/text';
import { FixedHeightModal } from '../../../../shared/ui/fixed-height-modal/FixedHeightModal';
import {
  nextMcpDirectoryExpandedKeys,
  normalizeMcpDirectoryPath
} from './mcp-management-view-model';
import { McpInstanceDiscoveryPolicyModal } from './McpInstanceDiscoveryPolicyModal';

import { McpBundleExportModal } from './bundle/McpBundleExportModal';
import { McpBundleImportFlow } from './bundle/McpBundleImportFlow';

import { useMcpDirectoryEditor } from './McpInstancesTab/use-mcp-directory-editor';
import { useMcpInstanceActions } from './McpInstancesTab/use-mcp-instance-actions';
import type { GroupFormValues } from './McpInstancesTab/editor-types';

export function McpInstancesTab({
  canManage,
  catalog
}: {
  canManage: boolean;
  catalog: ConsoleMcpCatalog;
}) {
  const csrfToken = useAuthStore((state) => state.csrfToken ?? '');
  const queryClient = useQueryClient();
  const directory = useMcpDirectoryEditor(catalog);
  const {
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
  } = directory;
  const {
    exportDefaults,
    instanceForm,
    copyInstanceForm,
    discoveryPolicyInstance,
    setDiscoveryPolicyInstance,
    clientConfigurationInstance,
    setClientConfigurationInstance,
    copyingInstance,
    setCopyingInstance,
    bundleExportInstance,
    setBundleExportInstance,
    exportingInstanceBundle,
    restoreSource,
    setRestoreSource,
    saveInstanceMutation,
    deleteInstanceMutation,
    copyInstanceMutation,
    handleExportInstanceBundle,
    selectedDiscoveryPolicy
  } = useMcpInstanceActions(canManage, catalog, directory);
  return (
    <Space
      orientation="vertical"
      size="middle"
      className="mcp-management__stack"
    >
      <McpInstanceTable
        onCreate={() => {
          setEditingInstance(null);
          instanceForm.setFieldsValue({
            instance_id: '',
            name: '',
            description_short: null,
            status: 'draft',
            default_entry_path: '/',
            webmcp_exposure: 'disabled'
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
            default_entry_path: record.default_entry_path,
            webmcp_exposure: record.webmcp_exposure
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
            path_segment: '',
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
        onRestoreDefault={(record) => {
          const source = record.managed_by;
          if (!source) return;
          setRestoreSource({
            kind: 'builtin_template',
            templateId: `mcp:${source.organization}/${source.bundle_id}`,
            instanceId: record.instance_id
          });
        }}
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
      <McpBundleImportFlow
        mode="restore"
        source={restoreSource}
        csrfToken={csrfToken}
        onClose={() => setRestoreSource(null)}
        onApplied={async () => {
          await queryClient.invalidateQueries({
            queryKey: settingsMcpCatalogQueryKey
          });
        }}
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
                          path_segment: '',
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
                      : directoryEditorMode === 'group'
                        ? normalizeMcpDirectoryPath(
                            groupForm.getFieldValue('path')
                          )
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
                      onSuccess: (savedGroup) => {
                        const savedValues: GroupFormValues = savedGroup
                          ? {
                              instance_id: values.instance_id,
                              path: savedGroup.path,
                              path_segment: '',
                              display_name: savedGroup.display_name,
                              description_short: savedGroup.description_short,
                              enabled: savedGroup.enabled,
                              sort_order: savedGroup.sort_order
                            }
                          : values;
                        const savedPath = normalizeMcpDirectoryPath(
                          savedValues.path
                        );
                        setParentGroupPath(null);
                        setDirectoryDraftActive(false);
                        setSelectedDirectoryKey(`group:${savedPath}`);
                        setDirectoryEditorIntent('edit');
                        groupSavedValuesRef.current = savedValues;
                        groupForm.setFieldsValue(savedValues);
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
                  <Form.Item name="path" hidden>
                    <Input />
                  </Form.Item>
                  {directoryEditorIntent === 'create' && (
                    <>
                      <Form.Item
                        label={i18nText(
                          'settingsMcpManagement',
                          'auto.directory_path'
                        )}
                      >
                        <Space.Compact block>
                          <Input
                            aria-label={i18nText(
                              'settingsMcpManagement',
                              'auto.parent_group_prefix'
                            )}
                            readOnly
                            title={groupCreationPathPrefix()}
                            value={groupCreationPathPrefix()}
                            style={{ width: '45%' }}
                          />
                          <Form.Item
                            name="path_segment"
                            noStyle
                            rules={[
                              { required: true, whitespace: true },
                              {
                                validator: (_, value) => {
                                  const pathSegment = String(
                                    value ?? ''
                                  ).trim();
                                  const path = groupCreationPath(pathSegment);
                                  return /^[A-Za-z0-9_-]+$/.test(pathSegment) &&
                                    path.length <= 255
                                    ? Promise.resolve()
                                    : Promise.reject(
                                        new Error(
                                          i18nText(
                                            'settingsMcpManagement',
                                            'auto.group_path_segment_invalid'
                                          )
                                        )
                                      );
                                }
                              }
                            ]}
                          >
                            <Input
                              aria-label={i18nText(
                                'settingsMcpManagement',
                                'auto.directory_path'
                              )}
                              onChange={(event) =>
                                groupForm.setFieldValue(
                                  'path',
                                  groupCreationPath(event.target.value)
                                )
                              }
                            />
                          </Form.Item>
                        </Space.Compact>
                      </Form.Item>
                    </>
                  )}
                  <Form.Item
                    name="display_name"
                    label={i18nText(
                      'settingsMcpManagement',
                      'auto.display_name'
                    )}
                    rules={[{ max: 255 }]}
                  >
                    <Input />
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
