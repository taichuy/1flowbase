import { useEffect, useMemo, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Button,
  Checkbox,
  Form,
  Input,
  Modal,
  Popconfirm,
  Space,
  Tabs,
  Tag,
  Tree,
  Typography,
  message
} from 'antd';
import type { TreeDataNode } from 'antd';
import {
  SearchOutlined,
  PlusOutlined,
  EditOutlined,
  DeleteOutlined,
  SaveOutlined,
  TeamOutlined,
  SafetyCertificateOutlined
} from '@ant-design/icons';

import { useAuthStore } from '../../../state/auth-store';
import {
  fetchSettingsPermissions,
  settingsPermissionsQueryKey,
  type SettingsPermission
} from '../api/permissions';
import {
  createSettingsRole,
  deleteSettingsRole,
  fetchSettingsRolePermissions,
  fetchSettingsRoles,
  replaceSettingsRolePermissions,
  settingsRolePermissionsQueryKey,
  settingsRolesQueryKey,
  updateSettingsRole,
  type SettingsRole
} from '../api/roles';
import { SettingsSectionSurface } from './SettingsSectionSurface';
import { i18nText } from '../../../shared/i18n/text';
import { RoleDataPolicySection } from './role-permissions/RoleDataPolicySection';

// 分类映射，根据要求
const RESOURCE_MAP: Record<
  string,
  { tab: string; label: string; order: number }
> = {
  role_permission: {
    tab: i18nText("settings", "auto.basic_configuration"),
    label: i18nText("settings", "auto.permission_role_permission"),
    order: 1
  },
  user: { tab: i18nText("settings", "auto.basic_configuration"), label: i18nText("settings", "auto.user"), order: 2 },
  team: { tab: i18nText("settings", "auto.basic_configuration"), label: i18nText("settings", "auto.team"), order: 3 },
  external_data_source: {
    tab: i18nText("settings", "auto.basic_configuration"),
    label: i18nText("settings", "auto.data_source_external_data_source"),
    order: 4
  },

  application: { tab: i18nText("settings", "auto.system_management"), label: i18nText("settings", "auto.application"), order: 1 },
  embedded_app: { tab: i18nText("settings", "auto.system_management"), label: i18nText("settings", "auto.subsystem_embedded_app"), order: 2 },
  plugin_config: {
    tab: i18nText("settings", "auto.system_management"),
    label: i18nText("settings", "auto.plugin_configuration_plugin_config"),
    order: 3
  },
  state_model: { tab: i18nText("settings", "auto.system_management"), label: i18nText("settings", "auto.model_supplier_state_model"), order: 4 },

  route_page: { tab: i18nText("settings", "auto.routing_page"), label: i18nText("settings", "auto.route_permissions_route_page"), order: 1 },

  flow: { tab: i18nText("settings", "auto.agent_application"), label: i18nText("settings", "auto.workflow"), order: 1 },
  publish_endpoint: {
    tab: i18nText("settings", "auto.agent_application"),
    label: i18nText("settings", "auto.publish_endpoint"),
    order: 2
  }
};

const TAB_ORDER = [i18nText("settings", "auto.basic_configuration"), i18nText("settings", "auto.system_management"), i18nText("settings", "auto.routing_page"), i18nText("settings", "auto.agent_application"), i18nText("settings", "auto.others")];
const ROLE_PERMISSION_GENERAL_TAB = i18nText("settings", "auto.basic_general");
const ROLE_TABLE_GENERAL_TAB = i18nText("settings", "auto.table_general_configuration");
const ROLE_TABLE_SINGLE_TAB = i18nText("settings", "auto.table_single_configuration");

export function RolePermissionPanel({
  canManageRoles
}: {
  canManageRoles: boolean;
}) {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [messageApi, contextHolder] = message.useMessage();

  const [searchQuery, setSearchQuery] = useState('');
  const [selectedRoleCode, setSelectedRoleCode] = useState<string | null>(null);
  const [activePermissionTab, setActivePermissionTab] = useState(
    ROLE_PERMISSION_GENERAL_TAB
  );

  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [editingRole, setEditingRole] = useState<SettingsRole | null>(null);

  const [createForm] = Form.useForm();
  const [editForm] = Form.useForm();

  // Queries
  const rolesQuery = useQuery({
    queryKey: settingsRolesQueryKey,
    queryFn: fetchSettingsRoles
  });

  const permissionsQuery = useQuery({
    queryKey: settingsPermissionsQueryKey,
    queryFn: fetchSettingsPermissions
  });

  const rolePermissionsQuery = useQuery({
    queryKey: settingsRolePermissionsQueryKey(selectedRoleCode ?? 'none'),
    queryFn: () => fetchSettingsRolePermissions(selectedRoleCode ?? ''),
    enabled: Boolean(selectedRoleCode)
  });

  // Local state for fast UI updates
  const [localCheckedCodes, setLocalCheckedCodes] = useState<string[]>([]);

  useEffect(() => {
    setLocalCheckedCodes(rolePermissionsQuery.data?.permission_codes ?? []);
  }, [rolePermissionsQuery.data?.permission_codes]);

  useEffect(() => {
    if (!selectedRoleCode && rolesQuery.data?.length) {
      setSelectedRoleCode(rolesQuery.data[0].code);
    }
  }, [rolesQuery.data, selectedRoleCode]);

  const filteredRoles = useMemo(() => {
    if (!rolesQuery.data) return [];
    return rolesQuery.data.filter(
      (r) =>
        r.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        r.code.toLowerCase().includes(searchQuery.toLowerCase())
    );
  }, [rolesQuery.data, searchQuery]);

  const selectedRole = useMemo(() => {
    return rolesQuery.data?.find((r) => r.code === selectedRoleCode) || null;
  }, [rolesQuery.data, selectedRoleCode]);
  const dataPolicyFormId = selectedRole
    ? `role-data-policy-${selectedRole.code}`
    : undefined;
  const isDataPolicyTab =
    activePermissionTab === ROLE_TABLE_GENERAL_TAB ||
    activePermissionTab === ROLE_TABLE_SINGLE_TAB;

  const tabsData = useMemo(() => {
    const allPerms = permissionsQuery.data ?? [];
    const tabsMap = new Map<string, Map<string, SettingsPermission[]>>();

    allPerms.forEach((p) => {
      const resKey = p.resource || 'other';
      const mapInfo = RESOURCE_MAP[resKey];
      const tabName = mapInfo ? mapInfo.tab : i18nText("settings", "auto.others");

      if (!tabsMap.has(tabName)) {
        tabsMap.set(tabName, new Map());
      }
      const resMap = tabsMap.get(tabName)!;
      if (!resMap.has(resKey)) {
        resMap.set(resKey, []);
      }
      resMap.get(resKey)!.push(p);
    });

    return TAB_ORDER.filter((t) => tabsMap.has(t)).map((tabName) => {
      const resMap = tabsMap.get(tabName)!;
      const resources = Array.from(resMap.entries())
        .map(([resKey, perms]) => {
          const mapInfo = RESOURCE_MAP[resKey];
          return {
            key: resKey,
            label: mapInfo ? mapInfo.label : resKey,
            order: mapInfo ? mapInfo.order : 99,
            permissions: perms
          };
        })
        .sort((a, b) => a.order - b.order);

      const treeData: TreeDataNode[] = resources.map((res) => ({
        title: res.label,
        key: `resource:${res.key}`,
        children: res.permissions.map((p) => ({
          title: <span title={p.code}>{p.name}</span>,
          key: p.code
        }))
      }));

      const tabLeafKeys = resources.flatMap((res) =>
        res.permissions.map((p) => p.code)
      );

      return {
        key: tabName,
        label: tabName,
        treeData,
        tabLeafKeys
      };
    });
  }, [permissionsQuery.data]);

  const invalidateRoles = async () => {
    await queryClient.invalidateQueries({ queryKey: settingsRolesQueryKey });
    if (selectedRoleCode) {
      await queryClient.invalidateQueries({
        queryKey: settingsRolePermissionsQueryKey(selectedRoleCode)
      });
    }
  };

  const replacePermissionsMutation = useMutation({
    mutationFn: async (permissionCodes: string[]) => {
      if (!csrfToken || !selectedRoleCode) throw new Error('missing selection');
      return replaceSettingsRolePermissions(
        selectedRoleCode,
        { permission_codes: permissionCodes },
        csrfToken
      );
    },
    onSuccess: async () => {
      messageApi.success(i18nText("settings", "auto.permissions_updated_successfully"));
      await invalidateRoles();
    },
    onError: () => {
      messageApi.error(i18nText("settings", "auto.permission_update_failed"));
      // revert local state on error
      setLocalCheckedCodes(rolePermissionsQuery.data?.permission_codes ?? []);
    }
  });

  const createMutation = useMutation({
    mutationFn: async (values: Record<string, unknown>) => {
      if (!csrfToken) throw new Error('missing csrf token');
      return createSettingsRole(
        {
          code: String(values.code ?? ''),
          name: String(values.name ?? ''),
          introduction: String(values.introduction ?? ''),
          auto_grant_new_permissions: Boolean(
            values.auto_grant_new_permissions
          ),
          is_default_member_role: Boolean(values.is_default_member_role)
        },
        csrfToken
      );
    },
    onSuccess: async () => {
      messageApi.success(i18nText("settings", "auto.role_created_successfully"));
      createForm.resetFields();
      setIsCreateModalOpen(false);
      await invalidateRoles();
    },
    onError: () => messageApi.error(i18nText("settings", "auto.character_creation_failed"))
  });

  const updateMutation = useMutation({
    mutationFn: async (values: Record<string, unknown>) => {
      if (!csrfToken || !editingRole)
        throw new Error('missing csrf token or editing role');
      return updateSettingsRole(
        editingRole.code,
        {
          name: String(values.name ?? ''),
          introduction: String(values.introduction ?? ''),
          auto_grant_new_permissions: Boolean(
            values.auto_grant_new_permissions
          ),
          is_default_member_role: Boolean(values.is_default_member_role)
        },
        csrfToken
      );
    },
    onSuccess: async () => {
      messageApi.success(i18nText("settings", "auto.role_updated_successfully"));
      setEditingRole(null);
      await invalidateRoles();
    },
    onError: () => messageApi.error(i18nText("settings", "auto.character_update_failed"))
  });

  const deleteMutation = useMutation({
    mutationFn: async (roleCode: string) => {
      if (!csrfToken) throw new Error('missing csrf token');
      return deleteSettingsRole(roleCode, csrfToken);
    },
    onSuccess: async (_, variables) => {
      messageApi.success(i18nText("settings", "auto.role_deleted"));
      if (selectedRoleCode === variables) {
        setSelectedRoleCode(rolesQuery.data?.[0]?.code ?? null);
      }
      await invalidateRoles();
    },
    onError: () => messageApi.error(i18nText("settings", "auto.role_deletion_failed"))
  });

  const handleEditClick = (role: SettingsRole) => {
    setEditingRole(role);
    editForm.setFieldsValue({
      name: role.name,
      introduction: role.introduction ?? '',
      auto_grant_new_permissions: role.auto_grant_new_permissions,
      is_default_member_role: role.is_default_member_role
    });
  };

  const permissionTabItems = useMemo(() => {
    const renderPermissionTree = (tab: (typeof tabsData)[number]) => (
      <div style={{ paddingBottom: 32 }}>
        <Tree
          checkable
          disabled={!canManageRoles || !selectedRole?.is_editable}
          checkedKeys={localCheckedCodes.filter((code) =>
            tab.tabLeafKeys.includes(code)
          )}
          onCheck={(checkedKeysValue) => {
            const keys = Array.isArray(checkedKeysValue)
              ? checkedKeysValue
              : checkedKeysValue.checked;
            const newlyCheckedLeaves = keys
              .map(String)
              .filter((k) => !k.startsWith('resource:'));

            const otherCheckedCodes = localCheckedCodes.filter(
              (c) => !tab.tabLeafKeys.includes(c)
            );
            const newCodes = [...otherCheckedCodes, ...newlyCheckedLeaves];

            setLocalCheckedCodes(newCodes);
            replacePermissionsMutation.mutate(newCodes);
          }}
          treeData={tab.treeData}
          defaultExpandAll={false}
        />
      </div>
    );

    const regularTabs = tabsData.map((tab) => ({
      key:
        tab.key === i18nText("settings", "auto.basic_configuration")
          ? ROLE_PERMISSION_GENERAL_TAB
          : tab.key,
      label:
        tab.label === i18nText("settings", "auto.basic_configuration")
          ? ROLE_PERMISSION_GENERAL_TAB
          : tab.label,
      children: renderPermissionTree(tab)
    }));

    const defaultDataPolicyTab = selectedRole
      ? {
          key: ROLE_TABLE_GENERAL_TAB,
          label: ROLE_TABLE_GENERAL_TAB,
          children: (
            <RoleDataPolicySection
              canEdit={canManageRoles && selectedRole.is_editable}
              formId={dataPolicyFormId ?? ''}
              roleCode={selectedRole.code}
              section="default-policy"
            />
          )
        }
      : null;

    const singleModelPolicyTab = selectedRole
      ? {
          key: ROLE_TABLE_SINGLE_TAB,
          label: ROLE_TABLE_SINGLE_TAB,
          children: (
            <RoleDataPolicySection
              canEdit={canManageRoles && selectedRole.is_editable}
              formId={dataPolicyFormId ?? ''}
              roleCode={selectedRole.code}
              section="single-model-policy"
            />
          )
        }
      : null;

    const [firstTab, ...restTabs] = regularTabs;
    const fallbackGeneralTab = {
      key: ROLE_PERMISSION_GENERAL_TAB,
      label: ROLE_PERMISSION_GENERAL_TAB,
      children: <div style={{ paddingBottom: 32 }} />
    };

    return [
      firstTab ?? fallbackGeneralTab,
      ...(defaultDataPolicyTab ? [defaultDataPolicyTab] : []),
      ...(singleModelPolicyTab ? [singleModelPolicyTab] : []),
      ...restTabs
    ];
  }, [
    canManageRoles,
    dataPolicyFormId,
    localCheckedCodes,
    replacePermissionsMutation,
    selectedRole,
    tabsData
  ]);

  return (
    <SettingsSectionSurface title={i18nText("settings", "auto.permission_management")} hideHeader heightMode="fill">
      <div
        style={{
          display: 'flex',
          flexDirection: 'column',
          gap: '24px',
          width: '100%',
          minHeight: 'calc(100vh - 120px)'
        }}
      >
        {contextHolder}

        <div
          style={{
            flex: 1,
            minHeight: 0,
            display: 'flex',
            border: '1px solid #f0f0f0',
            borderRadius: '8px',
            background: '#fff',
            overflow: 'hidden'
          }}
        >
          {/* 左侧：角色列表 */}
          <div
            style={{
              width: 280,
              borderRight: '1px solid #f0f0f0',
              display: 'flex',
              flexDirection: 'column',
              background: '#fafafa',
              flexShrink: 0
            }}
          >
            <div
              style={{
                padding: 16,
                borderBottom: '1px solid #f0f0f0',
                background: '#fff'
              }}
            >
              <Space
                direction="vertical"
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
                    {i18nText("settings", "auto.create_new_role")}</Button>
                )}
                <Input
                  placeholder={i18nText("settings", "auto.search_for_roles")}
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
                  {i18nText("settings", "auto.loading")}</div>
              ) : filteredRoles.length === 0 ? (
                <div
                  style={{ padding: 32, textAlign: 'center', color: '#bfbfbf' }}
                >
                  {i18nText("settings", "auto.no_role_yet")}</div>
              ) : (
                <div style={{ padding: '8px 0' }}>
                  {filteredRoles.map((role) => {
                    const isActive = selectedRoleCode === role.code;
                    return (
                      <div
                        key={role.code}
                        onClick={() => setSelectedRoleCode(role.code)}
                        style={{
                          padding: '12px 16px',
                          cursor: 'pointer',
                          background: isActive ? '#e6f4ff' : 'transparent',
                          borderRight: isActive
                            ? '3px solid #1677ff'
                            : '3px solid transparent',
                          transition: 'all 0.2s'
                        }}
                      >
                        <div
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
                              {i18nText("settings", "auto.built_in")}</Tag>
                          )}
                        </div>
                        <div style={{ fontSize: '12px', color: '#8c8c8c' }}>
                          {role.code}
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          </div>

          {/* 右侧：权限配置详情 */}
          <div
            style={{
              flex: 1,
              display: 'flex',
              flexDirection: 'column',
              overflow: 'hidden'
            }}
          >
            {selectedRole ? (
              <>
                {/* 头部信息 */}
                <div
                  style={{
                    padding: '20px 24px',
                    borderBottom: '1px solid #f0f0f0',
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'flex-start',
                    flexShrink: 0
                  }}
                >
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
                    <Space size="large" style={{ color: '#595959' }}>
                      <span>{i18nText("settings", "auto.encoding")}{selectedRole.code}</span>
                      <span>{i18nText("settings", "auto.scope_alt")}{selectedRole.scope_kind}</span>
                      {selectedRole.introduction && (
                        <span>{i18nText("settings", "auto.description_alt")}{selectedRole.introduction}</span>
                      )}
                      {selectedRole.auto_grant_new_permissions ? (
                        <Tag color="blue">{i18nText("settings", "auto.automatically_receive_new_permissions")}</Tag>
                      ) : null}
                      {selectedRole.is_default_member_role ? (
                        <Tag color="green">{i18nText("settings", "auto.new_user_role")}</Tag>
                      ) : null}
                    </Space>
                  </div>
                  <Space>
                    {isDataPolicyTab && dataPolicyFormId ? (
                      <Button
                        aria-label={i18nText("settings", "auto.save_data_policy")}
                        disabled={!canManageRoles || !selectedRole.is_editable}
                        form={dataPolicyFormId}
                        htmlType="submit"
                        icon={<SaveOutlined />}
                        type="primary"
                      >
                        {i18nText("settings", "auto.save_data_policy")}</Button>
                    ) : null}
                    {canManageRoles && selectedRole.is_editable ? (
                      <>
                        <Button
                          icon={<EditOutlined />}
                          onClick={() => handleEditClick(selectedRole)}
                        >
                          {i18nText("settings", "auto.edit_basic_information")}</Button>
                        <Popconfirm
                          title={i18nText("settings", "auto.sure_want_delete_role")}
                          onConfirm={() =>
                            deleteMutation.mutate(selectedRole.code)
                          }
                          okText={i18nText("settings", "auto.delete")}
                          okButtonProps={{ danger: true }}
                        >
                          <Button danger icon={<DeleteOutlined />}>
                            {i18nText("settings", "auto.delete_role")}</Button>
                        </Popconfirm>
                      </>
                    ) : null}
                  </Space>
                </div>

                {/* 权限多 Tab 配置 */}
                <div
                  style={{ flex: 1, overflowY: 'auto', padding: '16px 24px' }}
                >
                  {permissionsQuery.isLoading ||
                  rolePermissionsQuery.isLoading ? (
                    <div style={{ padding: 32, textAlign: 'center' }}>
                      {i18nText("settings", "auto.loading_permission_data")}</div>
                  ) : (
                    <Tabs
                      activeKey={activePermissionTab}
                      items={permissionTabItems}
                      onChange={setActivePermissionTab}
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
                <Space direction="vertical" align="center">
                  <TeamOutlined style={{ fontSize: 48 }} />
                  <Typography.Text type="secondary">
                    {i18nText("settings", "auto.select_role_left_view_details")}</Typography.Text>
                </Space>
              </div>
            )}
          </div>
        </div>

        <Modal
          title={i18nText("settings", "auto.create_new_role")}
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
              label={i18nText("settings", "auto.character_name")}
              name="name"
              rules={[{ required: true, message: i18nText("settings", "auto.enter_role_name") }]}
            >
              <Input placeholder={i18nText("settings", "auto.example_operations_specialist")} />
            </Form.Item>
            <Form.Item
              label={i18nText("settings", "auto.role_coding")}
              name="code"
              rules={[{ required: true, message: i18nText("settings", "auto.enter_role_code") }]}
              extra={i18nText("settings", "auto.encoding_must_globally_unique_modified_creation")}
            >
              <Input placeholder={i18nText("settings", "auto.example_role_ops_specialist")} />
            </Form.Item>
            <Form.Item label={i18nText("settings", "auto.role_description")} name="introduction">
              <Input.TextArea
                placeholder={i18nText("settings", "auto.briefly_describe_responsibilities_scope_role")}
                rows={3}
              />
            </Form.Item>
            <Form.Item
              name="auto_grant_new_permissions"
              valuePropName="checked"
              extra={i18nText("settings", "auto.turned_new_permissions_added_future_automatically_granted_role")}
            >
              <Checkbox>{i18nText("settings", "auto.automatically_receive_subsequent_new_permissions")}</Checkbox>
            </Form.Item>
            <Form.Item
              name="is_default_member_role"
              valuePropName="checked"
              extra={i18nText("settings", "auto.one_new_user_role_same_workspace")}
            >
              <Checkbox>{i18nText("settings", "auto.new_user_role")}</Checkbox>
            </Form.Item>
          </Form>
        </Modal>

        <Modal
          title={i18nText("settings", "auto.edit_role_alt")}
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
              label={i18nText("settings", "auto.character_name")}
              name="name"
              rules={[{ required: true, message: i18nText("settings", "auto.enter_role_name") }]}
            >
              <Input />
            </Form.Item>
            <Form.Item label={i18nText("settings", "auto.role_description")} name="introduction">
              <Input.TextArea rows={3} />
            </Form.Item>
            <Form.Item
              name="auto_grant_new_permissions"
              valuePropName="checked"
              extra={i18nText("settings", "auto.turned_new_permissions_added_future_automatically_granted_role")}
            >
              <Checkbox>{i18nText("settings", "auto.automatically_receive_subsequent_new_permissions")}</Checkbox>
            </Form.Item>
            <Form.Item
              name="is_default_member_role"
              valuePropName="checked"
              extra={i18nText("settings", "auto.one_new_user_role_same_workspace")}
            >
              <Checkbox>{i18nText("settings", "auto.new_user_role")}</Checkbox>
            </Form.Item>
          </Form>
        </Modal>
      </div>
    </SettingsSectionSurface>
  );
}
