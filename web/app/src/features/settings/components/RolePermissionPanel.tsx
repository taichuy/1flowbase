import { type RolePermissionTab } from './role-permissions/role-permission-tabs';
import { useRolePermissionController } from './role-permissions/useRolePermissionController';

import {
  Alert,
  Button,
  Checkbox,
  Form,
  Input,
  Modal,
  Popconfirm,
  Select,
  Space,
  Switch,
  Table,
  Tabs,
  Tag,
  Typography
} from 'antd';

import SearchOutlined from '@ant-design/icons/es/icons/SearchOutlined';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import EditOutlined from '@ant-design/icons/es/icons/EditOutlined';
import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import SaveOutlined from '@ant-design/icons/es/icons/SaveOutlined';
import TeamOutlined from '@ant-design/icons/es/icons/TeamOutlined';
import SafetyCertificateOutlined from '@ant-design/icons/es/icons/SafetyCertificateOutlined';

import { SettingsSectionSurface } from './SettingsSectionSurface';
import { ResizableDrawer } from '../../../shared/ui/resizable-drawer/ResizableDrawer';
import { i18nText } from '../../../shared/i18n/text';

import {
  detailPolicyOperation,
  type ConsolePolicyCatalogGroup,
  type ConsolePolicyRowOperation
} from './role-permissions/console-policy-draft';

import './role-permissions/role-permission-panel.css';

export function RolePermissionPanel({
  canManageRoles,
  activePermissionTab,
  onPermissionTabChange
}: {
  canManageRoles: boolean;
  activePermissionTab: RolePermissionTab;
  onPermissionTabChange: (
    tab: RolePermissionTab,
    navigationMode: 'push' | 'replace'
  ) => void;
}) {
  const {
    searchQuery,
    setSearchQuery,
    selectedRoleCode,
    setSelectedRoleCode,
    isCreateModalOpen,
    setIsCreateModalOpen,
    editingRole,
    setEditingRole,
    consolePolicyDetail,
    setConsolePolicyDetail,
    selectedConsoleOperationIds,
    setSelectedConsoleOperationIds,
    consoleOperationPathQuery,
    setConsoleOperationPathQuery,
    createForm,
    editForm,
    rolesQuery,
    consolePolicyCatalogQuery,
    roleFrontstageRoutesQuery,
    roleConsolePolicyQuery,
    filteredRoles,
    selectedRole,
    dataPolicyFormId,
    isDataPolicyTab,
    replaceConsolePolicyMutation,
    createMutation,
    updateMutation,
    deleteMutation,
    handleEditClick,
    updateConsolePolicyDetailOperation,
    simpleDetailOperations,
    filteredDetailOperations,
    visibleSimpleDetailOperations,
    allSimpleDetailOperationsSelected,
    updateSelectedConsoleOperations,
    saveConsolePolicyDetail,
    permissionTabItems
  } = useRolePermissionController({
    canManageRoles,
    activePermissionTab,
    onPermissionTabChange
  });

  return (
    <SettingsSectionSurface heightMode="fill">
      <div
        style={{
          display: 'flex',
          flexDirection: 'column',
          gap: '24px',
          width: '100%',
          minHeight: 'calc(100vh - 120px)'
        }}
      >
        <div
          className="role-permission-layout"
          data-testid="role-permission-layout"
          style={{
            flex: 1,
            minHeight: 0,
            border: '1px solid #f0f0f0',
            borderRadius: '8px',
            background: '#fff'
          }}
        >
          {/* 左侧：角色列表 */}
          <div
            className="role-permission-layout__rail"
            data-testid="role-permission-rail"
          >
            <div
              style={{
                padding: 16,
                borderBottom: '1px solid #f0f0f0',
                background: '#fff'
              }}
            >
              <Space
                orientation="vertical"
                size="middle"
                style={{ width: '100%' }}
              >
                {canManageRoles && (
                  <Button
                    type="primary"
                    icon={<PlusOutlined />}
                    block
                    onClick={() => setIsCreateModalOpen(true)}
                  >
                    {i18nText('settings', 'auto.create_new_role')}
                  </Button>
                )}
                <Input
                  placeholder={i18nText('settings', 'auto.search_for_roles')}
                  prefix={<SearchOutlined style={{ color: '#bfbfbf' }} />}
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  allowClear
                />
              </Space>
            </div>

            <div style={{ flex: 1, overflowY: 'auto' }}>
              {rolesQuery.isLoading ? (
                <div
                  style={{ padding: 16, textAlign: 'center', color: '#bfbfbf' }}
                >
                  {i18nText('settings', 'auto.loading')}
                </div>
              ) : filteredRoles.length === 0 ? (
                <div
                  style={{ padding: 32, textAlign: 'center', color: '#bfbfbf' }}
                >
                  {i18nText('settings', 'auto.no_role_yet')}
                </div>
              ) : (
                <div style={{ padding: '8px 0' }}>
                  {filteredRoles.map((role) => {
                    const isActive = selectedRoleCode === role.code;
                    return (
                      <button
                        type="button"
                        className="role-permission-layout__role"
                        aria-pressed={isActive}
                        key={role.code}
                        onClick={() => setSelectedRoleCode(role.code)}
                      >
                        <span
                          style={{
                            display: 'flex',
                            justifyContent: 'space-between',
                            alignItems: 'center',
                            marginBottom: 4
                          }}
                        >
                          <Typography.Text
                            strong={isActive}
                            style={{ color: isActive ? '#1677ff' : 'inherit' }}
                          >
                            {role.name}
                          </Typography.Text>
                          {role.is_builtin && (
                            <Tag
                              color="gold"
                              style={{ margin: 0, border: 'none' }}
                            >
                              {i18nText('settings', 'auto.built_in')}
                            </Tag>
                          )}
                        </span>
                        <span
                          style={{
                            display: 'block',
                            fontSize: '12px',
                            color: '#8c8c8c'
                          }}
                        >
                          {role.code}
                        </span>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          </div>

          {/* 右侧：权限配置详情 */}
          <div
            className="role-permission-layout__content"
            data-testid="role-permission-content"
          >
            {selectedRole ? (
              <>
                {/* 头部信息 */}
                <div className="role-permission-layout__header">
                  <div>
                    <Typography.Title
                      level={4}
                      style={{ margin: 0, marginBottom: 8 }}
                    >
                      <SafetyCertificateOutlined
                        style={{ marginRight: 8, color: '#1677ff' }}
                      />
                      {selectedRole.name}
                    </Typography.Title>
                    <Space size="large" wrap style={{ color: '#595959' }}>
                      <span>
                        {i18nText('settings', 'auto.encoding')}
                        {selectedRole.code}
                      </span>
                      <span>
                        {i18nText('settings', 'auto.scope_alt')}
                        {selectedRole.scope_kind}
                      </span>
                      {selectedRole.introduction && (
                        <span>
                          {i18nText('settings', 'auto.description_alt')}
                          {selectedRole.introduction}
                        </span>
                      )}
                      {selectedRole.auto_grant_new_permissions ? (
                        <Tag color="blue">
                          {i18nText(
                            'settings',
                            'auto.automatically_receive_new_permissions'
                          )}
                        </Tag>
                      ) : null}
                      {selectedRole.is_default_member_role ? (
                        <Tag color="green">
                          {i18nText('settings', 'auto.new_user_role')}
                        </Tag>
                      ) : null}
                    </Space>
                  </div>
                  <Space wrap>
                    {isDataPolicyTab && dataPolicyFormId ? (
                      <Button
                        aria-label={i18nText(
                          'settings',
                          'auto.save_data_policy'
                        )}
                        disabled={!canManageRoles || !selectedRole.is_editable}
                        form={dataPolicyFormId}
                        htmlType="submit"
                        icon={<SaveOutlined />}
                        type="primary"
                      >
                        {i18nText('settings', 'auto.save_data_policy')}
                      </Button>
                    ) : null}
                    {canManageRoles && selectedRole.is_editable ? (
                      <>
                        <Button
                          icon={<EditOutlined />}
                          onClick={() => handleEditClick(selectedRole)}
                        >
                          {i18nText('settings', 'auto.edit_basic_information')}
                        </Button>
                        <Popconfirm
                          title={i18nText(
                            'settings',
                            'auto.sure_want_delete_role'
                          )}
                          onConfirm={() =>
                            deleteMutation.mutate(selectedRole.code)
                          }
                          okText={i18nText('settings', 'auto.delete')}
                          okButtonProps={{ danger: true }}
                        >
                          <Button danger icon={<DeleteOutlined />}>
                            {i18nText('settings', 'auto.delete_role')}
                          </Button>
                        </Popconfirm>
                      </>
                    ) : null}
                  </Space>
                </div>

                {/* 权限多 Tab 配置 */}
                <div className="role-permission-layout__policy">
                  {consolePolicyCatalogQuery.isLoading ||
                  roleConsolePolicyQuery.isLoading ? (
                    <div style={{ padding: 32, textAlign: 'center' }}>
                      {i18nText('settings', 'auto.loading_permission_data')}
                    </div>
                  ) : consolePolicyCatalogQuery.isError ||
                    roleConsolePolicyQuery.isError ||
                    roleFrontstageRoutesQuery.isError ? (
                    <Alert
                      type="error"
                      showIcon
                      title={i18nText(
                        'settings',
                        'auto.permission_data_load_failed'
                      )}
                      action={
                        <Button
                          onClick={() => {
                            void consolePolicyCatalogQuery.refetch();
                            void roleConsolePolicyQuery.refetch();
                            void roleFrontstageRoutesQuery.refetch();
                          }}
                        >
                          {i18nText('settings', 'auto.retry_permission_data')}
                        </Button>
                      }
                    />
                  ) : (
                    <Tabs
                      activeKey={activePermissionTab}
                      items={permissionTabItems}
                      onChange={(tab) =>
                        onPermissionTabChange(tab as RolePermissionTab, 'push')
                      }
                    />
                  )}
                </div>
              </>
            ) : (
              <div
                style={{
                  flex: 1,
                  display: 'flex',
                  justifyContent: 'center',
                  alignItems: 'center',
                  color: '#bfbfbf'
                }}
              >
                <Space orientation="vertical" align="center">
                  <TeamOutlined style={{ fontSize: 48 }} />
                  <Typography.Text type="secondary">
                    {i18nText('settings', 'auto.select_role_left_view_details')}
                  </Typography.Text>
                </Space>
              </div>
            )}
          </div>
        </div>

        <Modal
          title={i18nText('settings', 'auto.create_new_role')}
          open={isCreateModalOpen}
          onCancel={() => {
            setIsCreateModalOpen(false);
            createForm.resetFields();
          }}
          onOk={() => createForm.submit()}
          confirmLoading={createMutation.isPending}
          destroyOnHidden
        >
          <Form
            form={createForm}
            layout="vertical"
            onFinish={(values) => createMutation.mutate(values)}
            initialValues={{
              auto_grant_new_permissions: false,
              is_default_member_role: false
            }}
            style={{ marginTop: 24 }}
          >
            <Form.Item
              label={i18nText('settings', 'auto.character_name')}
              name="name"
              rules={[
                {
                  required: true,
                  message: i18nText('settings', 'auto.enter_role_name')
                }
              ]}
            >
              <Input
                placeholder={i18nText(
                  'settings',
                  'auto.example_operations_specialist'
                )}
              />
            </Form.Item>
            <Form.Item
              label={i18nText('settings', 'auto.role_coding')}
              name="code"
              rules={[
                {
                  required: true,
                  message: i18nText('settings', 'auto.enter_role_code')
                }
              ]}
              extra={i18nText(
                'settings',
                'auto.encoding_must_globally_unique_modified_creation'
              )}
            >
              <Input
                placeholder={i18nText(
                  'settings',
                  'auto.example_role_ops_specialist'
                )}
              />
            </Form.Item>
            <Form.Item
              label={i18nText('settings', 'auto.role_description')}
              name="introduction"
            >
              <Input.TextArea
                placeholder={i18nText(
                  'settings',
                  'auto.briefly_describe_responsibilities_scope_role'
                )}
                rows={3}
              />
            </Form.Item>
            <Form.Item
              name="auto_grant_new_permissions"
              valuePropName="checked"
              extra={i18nText(
                'settings',
                'auto.turned_new_permissions_added_future_automatically_granted_role'
              )}
            >
              <Checkbox>
                {i18nText(
                  'settings',
                  'auto.automatically_receive_subsequent_new_permissions'
                )}
              </Checkbox>
            </Form.Item>
            <Form.Item
              name="is_default_member_role"
              valuePropName="checked"
              extra={i18nText(
                'settings',
                'auto.one_new_user_role_same_workspace'
              )}
            >
              <Checkbox>{i18nText('settings', 'auto.new_user_role')}</Checkbox>
            </Form.Item>
          </Form>
        </Modal>

        <Modal
          title={i18nText('settings', 'auto.edit_role_alt')}
          open={!!editingRole}
          onCancel={() => setEditingRole(null)}
          onOk={() => editForm.submit()}
          confirmLoading={updateMutation.isPending}
          destroyOnHidden
        >
          <Form
            form={editForm}
            layout="vertical"
            onFinish={(values) => updateMutation.mutate(values)}
            style={{ marginTop: 24 }}
          >
            <Form.Item
              label={i18nText('settings', 'auto.character_name')}
              name="name"
              rules={[
                {
                  required: true,
                  message: i18nText('settings', 'auto.enter_role_name')
                }
              ]}
            >
              <Input />
            </Form.Item>
            <Form.Item
              label={i18nText('settings', 'auto.role_description')}
              name="introduction"
            >
              <Input.TextArea rows={3} />
            </Form.Item>
            <Form.Item
              name="auto_grant_new_permissions"
              valuePropName="checked"
              extra={i18nText(
                'settings',
                'auto.turned_new_permissions_added_future_automatically_granted_role'
              )}
            >
              <Checkbox>
                {i18nText(
                  'settings',
                  'auto.automatically_receive_subsequent_new_permissions'
                )}
              </Checkbox>
            </Form.Item>
            <Form.Item
              name="is_default_member_role"
              valuePropName="checked"
              extra={i18nText(
                'settings',
                'auto.one_new_user_role_same_workspace'
              )}
            >
              <Checkbox>{i18nText('settings', 'auto.new_user_role')}</Checkbox>
            </Form.Item>
          </Form>
        </Modal>

        <ResizableDrawer
          title={
            consolePolicyDetail
              ? i18nText('settings', 'auto.permission_policy_detail_title', {
                  value1: consolePolicyDetail.catalogGroup.label
                })
              : null
          }
          open={Boolean(consolePolicyDetail)}
          onClose={() => {
            setConsolePolicyDetail(null);
            setSelectedConsoleOperationIds([]);
          }}
          defaultWidth={640}
          minWidth={480}
          maxWidth={1200}
          destroyOnClose
          resizeLabel={i18nText(
            'settings',
            'auto.resize_permission_policy_drawer'
          )}
          footer={
            <Space style={{ display: 'flex', justifyContent: 'flex-end' }}>
              <Button
                onClick={() => {
                  setConsolePolicyDetail(null);
                  setSelectedConsoleOperationIds([]);
                }}
              >
                {i18nText('settings', 'auto.cancel')}
              </Button>
              <Button
                type="primary"
                loading={replaceConsolePolicyMutation.isPending}
                disabled={!canManageRoles || !selectedRole?.is_editable}
                onClick={saveConsolePolicyDetail}
              >
                {i18nText('settings', 'auto.permission_policy_save')}
              </Button>
            </Space>
          }
        >
          {consolePolicyDetail ? (
            <Space
              orientation="vertical"
              size="large"
              style={{ width: '100%' }}
            >
              {consolePolicyDetail.catalogGroup.description ? (
                <Typography.Paragraph type="secondary">
                  {consolePolicyDetail.catalogGroup.description}
                </Typography.Paragraph>
              ) : null}
              <div className="role-permission-detail-toolbar">
                <Input
                  className="role-permission-detail-toolbar__search"
                  aria-label={i18nText(
                    'settings',
                    'auto.search_operation_path'
                  )}
                  placeholder={i18nText(
                    'settings',
                    'auto.search_operation_path'
                  )}
                  prefix={<SearchOutlined />}
                  value={consoleOperationPathQuery}
                  onChange={(event) => {
                    setConsoleOperationPathQuery(event.target.value);
                    setSelectedConsoleOperationIds([]);
                  }}
                  allowClear
                />
                {simpleDetailOperations.length > 0 ? (
                  <div className="role-permission-detail-toolbar__actions">
                    <Button
                      disabled={
                        selectedConsoleOperationIds.length === 0 ||
                        !canManageRoles ||
                        !selectedRole?.is_editable ||
                        replaceConsolePolicyMutation.isPending
                      }
                      onClick={() => updateSelectedConsoleOperations(true)}
                    >
                      {i18nText('settings', 'auto.enable_selected_operations')}
                    </Button>
                    <Button
                      disabled={
                        selectedConsoleOperationIds.length === 0 ||
                        !canManageRoles ||
                        !selectedRole?.is_editable ||
                        replaceConsolePolicyMutation.isPending
                      }
                      onClick={() => updateSelectedConsoleOperations(false)}
                    >
                      {i18nText('settings', 'auto.disable_selected_operations')}
                    </Button>
                  </div>
                ) : null}
              </div>
              <Table
                rowKey="display_key"
                pagination={false}
                dataSource={[...filteredDetailOperations]
                  .sort((left, right) => left.order - right.order)
                  .map((operation, displayKey) => ({
                    ...operation,
                    display_key: displayKey
                  }))}
                columns={[
                  ...(simpleDetailOperations.length > 0
                    ? [
                        {
                          key: 'selection',
                          width: 48,
                          title: (
                            <Checkbox
                              aria-label={i18nText(
                                'settings',
                                'auto.select_all_switch_operations'
                              )}
                              checked={allSimpleDetailOperationsSelected}
                              indeterminate={
                                visibleSimpleDetailOperations.some(
                                  (operation) =>
                                    selectedConsoleOperationIds.includes(
                                      operation.operation_id
                                    )
                                ) && !allSimpleDetailOperationsSelected
                              }
                              disabled={
                                visibleSimpleDetailOperations.length === 0 ||
                                !canManageRoles ||
                                !selectedRole?.is_editable ||
                                replaceConsolePolicyMutation.isPending
                              }
                              onChange={(event) =>
                                setSelectedConsoleOperationIds(
                                  event.target.checked
                                    ? visibleSimpleDetailOperations.map(
                                        (operation) => operation.operation_id
                                      )
                                    : []
                                )
                              }
                            />
                          ),
                          render: (
                            _: unknown,
                            operation: ConsolePolicyCatalogGroup['operations'][number]
                          ) =>
                            operation.full_profile.kind === 'simple' ? (
                              <Checkbox
                                aria-label={i18nText(
                                  'settings',
                                  'auto.select_operation',
                                  { value1: operation.summary }
                                )}
                                checked={selectedConsoleOperationIds.includes(
                                  operation.operation_id
                                )}
                                disabled={
                                  !canManageRoles ||
                                  !selectedRole?.is_editable ||
                                  replaceConsolePolicyMutation.isPending
                                }
                                onChange={(event) =>
                                  setSelectedConsoleOperationIds((current) =>
                                    event.target.checked
                                      ? [...current, operation.operation_id]
                                      : current.filter(
                                          (id) => id !== operation.operation_id
                                        )
                                  )
                                }
                              />
                            ) : null
                        }
                      ]
                    : []),
                  {
                    title: i18nText('settings', 'auto.operation'),
                    key: 'operation',
                    render: (
                      _: unknown,
                      operation: ConsolePolicyCatalogGroup['operations'][number]
                    ) => (
                      <Space orientation="vertical" size={0}>
                        <Typography.Text>{operation.summary}</Typography.Text>
                        {operation.description ? (
                          <Typography.Text type="secondary">
                            {operation.description}
                          </Typography.Text>
                        ) : null}
                        <Typography.Text
                          code
                          type="secondary"
                          style={{ overflowWrap: 'anywhere' }}
                        >
                          {operation.route.method} {operation.route.path}
                        </Typography.Text>
                      </Space>
                    )
                  },
                  {
                    title: i18nText('settings', 'auto.scope'),
                    key: 'policy',
                    render: (
                      _: unknown,
                      operation: ConsolePolicyCatalogGroup['operations'][number]
                    ) => {
                      const policyOperation = detailPolicyOperation(
                        consolePolicyDetail.policyGroup,
                        operation
                      );

                      if (operation.full_profile.kind === 'simple') {
                        return (
                          <Switch
                            aria-label={operation.summary}
                            checked={
                              policyOperation.kind === 'simple' &&
                              policyOperation.enabled
                            }
                            disabled={
                              !canManageRoles ||
                              !selectedRole?.is_editable ||
                              replaceConsolePolicyMutation.isPending
                            }
                            onChange={(checked) =>
                              updateConsolePolicyDetailOperation(
                                operation.operation_id,
                                checked
                              )
                            }
                          />
                        );
                      }

                      const scope =
                        policyOperation.kind === 'row'
                          ? policyOperation.scope
                          : 'disabled';
                      return (
                        <Select
                          aria-label={`${operation.summary} ${i18nText(
                            'settings',
                            'auto.scope'
                          )}`}
                          value={scope}
                          disabled={
                            !canManageRoles ||
                            !selectedRole?.is_editable ||
                            replaceConsolePolicyMutation.isPending
                          }
                          options={operation.allowed_row_scopes.map(
                            (option) => ({
                              value: option.value,
                              label: option.label,
                              title: option.label
                            })
                          )}
                          onChange={(value) =>
                            updateConsolePolicyDetailOperation(
                              operation.operation_id,
                              value as ConsolePolicyRowOperation['scope']
                            )
                          }
                        />
                      );
                    }
                  }
                ]}
              />
            </Space>
          ) : null}
        </ResizableDrawer>
      </div>
    </SettingsSectionSurface>
  );
}

export { type RolePermissionTab } from './role-permissions/role-permission-tabs';
