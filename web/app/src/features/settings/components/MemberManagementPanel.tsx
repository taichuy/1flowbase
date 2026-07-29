import { useCallback, useMemo, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import {
  Avatar,
  Button,
  Form,
  Input,
  Modal,
  Popconfirm,
  Select,
  Space,
  Switch,
  Table,
  Tag,
  Typography
} from 'antd';
import {
  CheckCircleOutlined,
  DeleteOutlined,
  EditOutlined,
  KeyOutlined,
  StopOutlined,
  UserAddOutlined
} from '@ant-design/icons';

import { useAuthStore } from '../../../state/auth-store';
import {
  changeCurrentUserPassword,
  createSettingsMember,
  deleteSettingsMember,
  disableSettingsMember,
  enableSettingsMember,
  fetchSettingsMembers,
  replaceSettingsMemberRoles,
  resetSettingsMemberPassword,
  settingsMembersQueryKey,
  updateSettingsMember,
  type SettingsMember
} from '../api/members';
import { fetchSettingsRoles, settingsRolesQueryKey } from '../api/roles';
import { SettingsSectionSurface } from './SettingsSectionSurface';
import { i18nText } from '../../../shared/i18n/text';

const TEMP_PASSWORD = 'Temp@123456';

export function MemberManagementPanel({
  canManageMembers,
  canManageRoleBindings
}: {
  canManageMembers: boolean;
  canManageRoleBindings: boolean;
}) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const actor = useAuthStore((state) => state.actor);
  const setAnonymous = useAuthStore((state) => state.setAnonymous);

  const [createForm] = Form.useForm();
  const [profileForm] = Form.useForm();
  const [passwordForm] = Form.useForm();
  const [createModalOpen, setCreateModalOpen] = useState(false);
  const [profileEditMember, setProfileEditMember] =
    useState<SettingsMember | null>(null);
  const [passwordEditMember, setPasswordEditMember] =
    useState<SettingsMember | null>(null);

  const membersQuery = useQuery({
    queryKey: settingsMembersQueryKey,
    queryFn: fetchSettingsMembers
  });
  const rolesQuery = useQuery({
    queryKey: settingsRolesQueryKey,
    queryFn: fetchSettingsRoles,
    enabled: canManageRoleBindings
  });

  const createMutation = useMutation({
    mutationFn: async (values: Record<string, unknown>) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return createSettingsMember(
        {
          account: String(values.account ?? ''),
          email: String(values.email ?? ''),
          phone: values.phone ? String(values.phone) : null,
          password: String(values.password ?? ''),
          name: String(values.name ?? ''),
          nickname: String(values.nickname ?? ''),
          introduction: String(values.introduction ?? ''),
          email_login_enabled: Boolean(values.email_login_enabled),
          phone_login_enabled: Boolean(values.phone_login_enabled)
        },
        csrfToken
      );
    },
    onSuccess: async () => {
      createForm.resetFields();
      setCreateModalOpen(false);
      await queryClient.invalidateQueries({ queryKey: settingsMembersQueryKey });
    }
  });

  const disableMutation = useMutation({
    mutationFn: async (memberId: string) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return disableSettingsMember(memberId, csrfToken);
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: settingsMembersQueryKey });
    }
  });

  const enableMutation = useMutation({
    mutationFn: async (memberId: string) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return enableSettingsMember(memberId, csrfToken);
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: settingsMembersQueryKey });
    }
  });

  const deleteMutation = useMutation({
    mutationFn: async (memberId: string) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return deleteSettingsMember(memberId, csrfToken);
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: settingsMembersQueryKey });
    }
  });

  const resetPasswordMutation = useMutation({
    mutationFn: async (memberId: string) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return resetSettingsMemberPassword(
        memberId,
        { new_password: TEMP_PASSWORD },
        csrfToken
      );
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: settingsMembersQueryKey });
    }
  });

  const updateMemberMutation = useMutation({
    mutationFn: async ({
      memberId,
      values,
      roleCodes
    }: {
      memberId: string;
      values: Record<string, unknown>;
      roleCodes: string[] | null;
    }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      const member = await updateSettingsMember(
        memberId,
        {
          name: String(values.name ?? ''),
          nickname: String(values.nickname ?? ''),
          email: String(values.email ?? ''),
          phone: values.phone ? String(values.phone) : null,
          introduction: String(values.introduction ?? '')
        },
        csrfToken
      );

      if (roleCodes) {
        await replaceSettingsMemberRoles(
          memberId,
          { role_codes: roleCodes },
          csrfToken
        );
      }

      return member;
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: settingsMembersQueryKey });
      setProfileEditMember(null);
      profileForm.resetFields();
    }
  });

  const changePasswordMutation = useMutation({
    mutationFn: async (values: Record<string, unknown>) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return changeCurrentUserPassword(
        {
          old_password: String(values.old_password ?? ''),
          new_password: String(values.new_password ?? '')
        },
        csrfToken
      );
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: settingsMembersQueryKey });
      passwordForm.resetFields();
      setPasswordEditMember(null);
      setAnonymous();
      await navigate({
        to: '/sign-in',
        search: { authenticator_id: undefined }
      });
    }
  });

  const handleCreateSubmit = useCallback(
    (values: Record<string, unknown>) => {
      createMutation.mutate(values);
    },
    [createMutation]
  );

  const handleOpenProfileEdit = useCallback(
    (member: SettingsMember) => {
      setProfileEditMember(member);
      profileForm.setFieldsValue({
        name: member.name,
        nickname: member.nickname,
        email: member.email,
        phone: member.phone,
        introduction: member.introduction,
        role_codes: member.role_codes
      });
    },
    [profileForm]
  );

  const handleProfileEditSubmit = useCallback(
    (values: Record<string, unknown>) => {
      if (profileEditMember) {
        const submittedRoleCodes = Array.isArray(values.role_codes)
          ? values.role_codes.map(String)
          : profileEditMember.role_codes;
        const nextRoleCodes = profileEditMember.role_codes.includes('root')
          ? Array.from(new Set(['root', ...submittedRoleCodes]))
          : submittedRoleCodes;

        updateMemberMutation.mutate({
          memberId: profileEditMember.id,
          values,
          roleCodes: canManageRoleBindings ? nextRoleCodes : null
        });
      }
    },
    [canManageRoleBindings, profileEditMember, updateMemberMutation]
  );

  const handleOpenPasswordEdit = useCallback(
    (member: SettingsMember) => {
      setPasswordEditMember(member);
      passwordForm.resetFields();
    },
    [passwordForm]
  );

  const roleOptions = useMemo(
    () => {
      const options = (rolesQuery.data ?? []).map((role) => ({
        label: role.name,
        value: role.code
      }));

      if (
        profileEditMember?.role_codes.includes('root') &&
        !options.some((option) => option.value === 'root')
      ) {
        return [
          { label: 'root', value: 'root', disabled: true },
          ...options
        ];
      }

      return options;
    },
    [profileEditMember, rolesQuery.data]
  );

  const handleRoleSelectionChange = useCallback(
    (roleCodes: string[]) => {
      if (profileEditMember?.role_codes.includes('root')) {
        profileForm.setFieldValue(
          'role_codes',
          Array.from(new Set(['root', ...roleCodes]))
        );
        return;
      }

      profileForm.setFieldValue('role_codes', roleCodes);
    },
    [profileEditMember, profileForm]
  );

  const columns = useMemo(
    () => [
      {
        title: i18nText("settings", "auto.avatar"),
        key: 'avatar',
        width: 72,
        align: 'center' as const,
        render: (_: unknown, member: SettingsMember) => (
          <Avatar
            size="small"
            style={{ backgroundColor: '#00d084', flexShrink: 0 }}
          >
            {member.account.charAt(0).toUpperCase()}
          </Avatar>
        )
      },
      {
        title: i18nText("settings", "auto.account_number"),
        dataIndex: 'account',
        key: 'account',
        width: 160,
        render: (account: SettingsMember['account']) => (
          <Typography.Text strong style={{ fontSize: 14 }}>
            {account}
          </Typography.Text>
        )
      },
      {
        title: i18nText("settings", "auto.name_alt"),
        dataIndex: 'name',
        key: 'name',
        width: 160,
        render: (name: SettingsMember['name']) => (
          <Typography.Text style={{ fontSize: 14 }}>
            {name}
          </Typography.Text>
        )
      },
      {
        title: i18nText("settings", "auto.nickname"),
        dataIndex: 'nickname',
        key: 'nickname',
        width: 160,
        render: (nickname: SettingsMember['nickname']) => (
          <Typography.Text style={{ fontSize: 14 }}>
            {nickname}
          </Typography.Text>
        )
      },
      {
        title: i18nText("settings", "auto.contact_information"),
        key: 'contact',
        render: (_: unknown, member: SettingsMember) => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
            <Typography.Text style={{ fontSize: 13 }}>
              {member.email}
            </Typography.Text>
            {member.phone ? (
              <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                {member.phone}
              </Typography.Text>
            ) : null}
          </div>
        )
      },
      {
        title: i18nText("settings", "auto.status"),
        key: 'status',
        width: 80,
        render: (_: unknown, member: SettingsMember) => (
          <Tag color={member.status === 'active' ? 'green' : 'default'}>
            {member.status === 'active' ? i18nText("settings", "auto.enabled") : i18nText("settings", "auto.deactivate")}
          </Tag>
        )
      },
      ...(canManageMembers
        ? [
            {
              title: i18nText("settings", "auto.operation"),
              key: 'action',
              width: 240,
              render: (_: unknown, member: SettingsMember) => {
                const isRootMember = member.role_codes.includes('root');
                const isCurrentUser = member.id === actor?.id;

                return (
                  <Space size={4}>
                    <Button
                      size="small"
                      icon={<EditOutlined />}
                      onClick={() => handleOpenProfileEdit(member)}
                    >
                      {i18nText("settings", "auto.edit")}</Button>
                    {isRootMember ? (
                      <Button
                        size="small"
                        icon={<KeyOutlined />}
                        disabled={!isCurrentUser}
                        loading={changePasswordMutation.isPending}
                        onClick={
                          isCurrentUser
                            ? () => handleOpenPasswordEdit(member)
                            : undefined
                        }
                      >
                        {i18nText("settings", "auto.reset_password")}</Button>
                    ) : (
                      <Popconfirm
                        title={i18nText("settings", "auto.reset_password")}
                        description={i18nText("settings", "auto.reset_password_temporary_password_needs_changed_immediately_user_logs", { value1: member.name })}
                        onConfirm={() =>
                          resetPasswordMutation.mutate(member.id)
                        }
                        okText={i18nText("settings", "auto.confirm_reset")}
                        cancelText={i18nText("settings", "auto.cancel")}
                      >
                        <Button
                          size="small"
                          icon={<KeyOutlined />}
                          loading={resetPasswordMutation.isPending}
                        >
                          {i18nText("settings", "auto.reset_password")}</Button>
                      </Popconfirm>
                    )}
                    {member.status === 'active' ? (
                      isRootMember ? (
                        <Button
                          size="small"
                          color="orange"
                          variant="outlined"
                          icon={<StopOutlined />}
                          disabled
                        >
                          {i18nText("settings", "auto.deactivate")}</Button>
                      ) : (
                        <Popconfirm
                          title={i18nText("settings", "auto.deactivate_account")}
                          description={i18nText("settings", "auto.sure_want_deactivate_s_account_deactivation_user_able_log", { value1: member.name })}
                          onConfirm={() => disableMutation.mutate(member.id)}
                          okText={i18nText("settings", "auto.confirm_deactivation")}
                          cancelText={i18nText("settings", "auto.cancel")}
                          okButtonProps={{
                            color: 'orange',
                            variant: 'solid'
                          }}
                        >
                          <Button
                            size="small"
                            color="orange"
                            variant="outlined"
                            icon={<StopOutlined />}
                            loading={disableMutation.isPending}
                          >
                            {i18nText("settings", "auto.deactivate")}</Button>
                        </Popconfirm>
                      )
                    ) : (
                      <Popconfirm
                        title={i18nText("settings", "auto.restore_account")}
                        description={i18nText("settings", "auto.sure_want_restore_s_account_user_able_log", { value1: member.name })}
                        onConfirm={() => enableMutation.mutate(member.id)}
                        okText={i18nText("settings", "auto.confirm_restore")}
                        cancelText={i18nText("settings", "auto.cancel")}
                        okButtonProps={{
                          color: 'green',
                          variant: 'solid'
                        }}
                      >
                        <Button
                          size="small"
                          color="green"
                          variant="outlined"
                          icon={<CheckCircleOutlined />}
                          loading={enableMutation.isPending}
                        >
                          {i18nText("settings", "auto.restore")}</Button>
                      </Popconfirm>
                    )}
                    {isRootMember || isCurrentUser ? (
                      <Button
                        size="small"
                        danger
                        icon={<DeleteOutlined />}
                        disabled
                      >
                        {i18nText("settings", "auto.delete")}</Button>
                    ) : (
                      <Popconfirm
                        title={i18nText("settings", "auto.delete_member")}
                        description={i18nText("settings", "auto.sure_want_delete_member_account_physical_delete", { value1: member.name })}
                        onConfirm={() => deleteMutation.mutate(member.id)}
                        okText={i18nText("settings", "auto.confirm_delete")}
                        cancelText={i18nText("settings", "auto.cancel")}
                        okButtonProps={{ danger: true }}
                      >
                        <Button
                          size="small"
                          danger
                          icon={<DeleteOutlined />}
                          loading={deleteMutation.isPending}
                        >
                          {i18nText("settings", "auto.delete")}</Button>
                      </Popconfirm>
                    )}
                  </Space>
                );
              }
            }
          ]
        : [])
    ],
    [
      canManageMembers,
      deleteMutation,
      disableMutation,
      enableMutation,
      resetPasswordMutation,
      changePasswordMutation,
      actor?.id,
      handleOpenProfileEdit,
      handleOpenPasswordEdit
    ]
  );

  return (
    <SettingsSectionSurface heightMode="fill">
      <div style={{ display: 'flex', flexDirection: 'column', gap: 20 }}>
        {canManageMembers ? (
          <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
            <Button
              type="primary"
              icon={<UserAddOutlined />}
              onClick={() => setCreateModalOpen(true)}
            >
              {i18nText("settings", "auto.create_new_user")}</Button>
          </div>
        ) : null}

        <Table<SettingsMember>
          rowKey="id"
          loading={membersQuery.isLoading}
          dataSource={membersQuery.data ?? []}
          pagination={false}
          columns={columns}
          size="middle"
        />

        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          {i18nText("settings", "auto.resetting_password_reset_target_account_password_temporary_password_require_user")}</Typography.Text>

        {/* Create Member Modal */}
        <Modal
          title={i18nText("settings", "auto.create_new_user")}
          open={createModalOpen}
          onCancel={() => {
            setCreateModalOpen(false);
            createForm.resetFields();
          }}
          onOk={() => createForm.submit()}
          confirmLoading={createMutation.isPending}
          okText={i18nText("settings", "auto.create")}
          cancelText={i18nText("settings", "auto.cancel")}
          width={600}
          destroyOnHidden
        >
          <Form
            form={createForm}
            layout="vertical"
            onFinish={handleCreateSubmit}
            style={{ marginTop: 16 }}
          >
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: '1fr 1fr',
                gap: '0 16px'
              }}
            >
              <Form.Item
                label={i18nText("settings", "auto.account_number")}
                name="account"
                rules={[{ required: true, message: i18nText("settings", "auto.enter_account_number") }]}
              >
                <Input />
              </Form.Item>
              <Form.Item
                label={i18nText("settings", "auto.name_alt")}
                name="name"
                rules={[{ required: true, message: i18nText("settings", "auto.enter_full_name") }]}
              >
                <Input />
              </Form.Item>
              <Form.Item
                label={i18nText("settings", "auto.email")}
                name="email"
                rules={[
                  { required: true, message: i18nText("settings", "auto.enter_email") },
                  { type: 'email', message: i18nText("settings", "auto.enter_valid_email_address") }
                ]}
              >
                <Input />
              </Form.Item>
              <Form.Item label={i18nText("settings", "auto.mobile_phone_number")} name="phone">
                <Input />
              </Form.Item>
              <Form.Item
                label={i18nText("settings", "auto.nickname")}
                name="nickname"
                rules={[{ required: true, message: i18nText("settings", "auto.enter_nickname") }]}
              >
                <Input />
              </Form.Item>
              <Form.Item
                label={i18nText("settings", "auto.initial_password")}
                name="password"
                initialValue={TEMP_PASSWORD}
                rules={[{ required: true, message: i18nText("settings", "auto.enter_initial_password") }]}
              >
                <Input.Password />
              </Form.Item>
            </div>
            <Form.Item label={i18nText("settings", "auto.personal_introduction")} name="introduction">
              <Input.TextArea rows={2} />
            </Form.Item>
            <div style={{ display: 'flex', gap: 24 }}>
              <Form.Item
                label={i18nText("settings", "auto.email_login")}
                name="email_login_enabled"
                valuePropName="checked"
                initialValue
              >
                <Switch />
              </Form.Item>
              <Form.Item
                label={i18nText("settings", "auto.mobile_login")}
                name="phone_login_enabled"
                valuePropName="checked"
                initialValue={false}
              >
                <Switch />
              </Form.Item>
            </div>
          </Form>
        </Modal>

        {/* Profile Edit Modal */}
        <Modal
          title={
            profileEditMember
              ? i18nText("settings", "auto.edit_user_profile", { value1: profileEditMember.name })
              : i18nText("settings", "auto.edit_profile")
          }
          open={Boolean(profileEditMember)}
          onCancel={() => {
            setProfileEditMember(null);
            profileForm.resetFields();
          }}
          onOk={() => profileForm.submit()}
          confirmLoading={updateMemberMutation.isPending}
          okText={i18nText("settings", "auto.save")}
          cancelText={i18nText("settings", "auto.cancel")}
          width={560}
          destroyOnHidden
        >
          <Form
            form={profileForm}
            layout="vertical"
            onFinish={handleProfileEditSubmit}
            style={{ marginTop: 16 }}
          >
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: '1fr 1fr',
                gap: '0 16px'
              }}
            >
              <Form.Item
                label={i18nText("settings", "auto.name_alt")}
                name="name"
                rules={[{ required: true, message: i18nText("settings", "auto.enter_full_name") }]}
              >
                <Input />
              </Form.Item>
              <Form.Item
                label={i18nText("settings", "auto.nickname")}
                name="nickname"
                rules={[{ required: true, message: i18nText("settings", "auto.enter_nickname") }]}
              >
                <Input />
              </Form.Item>
              <Form.Item
                label={i18nText("settings", "auto.email")}
                name="email"
                rules={[
                  { required: true, message: i18nText("settings", "auto.enter_email") },
                  { type: 'email', message: i18nText("settings", "auto.enter_valid_email_address") }
                ]}
              >
                <Input />
              </Form.Item>
              <Form.Item label={i18nText("settings", "auto.mobile_phone_number")} name="phone">
                <Input />
              </Form.Item>
            </div>
            {canManageRoleBindings ? (
              <Form.Item label={i18nText("settings", "auto.role")} name="role_codes">
                <Select
                  mode="multiple"
                  options={roleOptions}
                  loading={rolesQuery.isLoading}
                  onChange={handleRoleSelectionChange}
                  placeholder={i18nText("settings", "auto.select_role")}
                />
              </Form.Item>
            ) : null}
            <Form.Item label={i18nText("settings", "auto.personal_introduction")} name="introduction">
              <Input.TextArea rows={3} />
            </Form.Item>
          </Form>
        </Modal>

        {/* Change Password Modal */}
        <Modal
          title={i18nText("settings", "auto.reset_password")}
          open={Boolean(passwordEditMember)}
          onCancel={() => {
            setPasswordEditMember(null);
            passwordForm.resetFields();
          }}
          onOk={() => passwordForm.submit()}
          confirmLoading={changePasswordMutation.isPending}
          okText={i18nText("settings", "auto.confirm_reset")}
          cancelText={i18nText("settings", "auto.cancel")}
          width={480}
          destroyOnHidden
        >
          <Form
            form={passwordForm}
            layout="vertical"
            onFinish={(values) => changePasswordMutation.mutate(values)}
            style={{ marginTop: 16 }}
          >
            <Form.Item
              label={i18nText("settings", "auto.current_password")}
              name="old_password"
              rules={[{ required: true, message: i18nText("settings", "auto.enter_current_password") }]}
            >
              <Input.Password />
            </Form.Item>
            <Form.Item
              label={i18nText("settings", "auto.new_password")}
              name="new_password"
              rules={[{ required: true, message: i18nText("settings", "auto.enter_new_password") }]}
            >
              <Input.Password />
            </Form.Item>
            <Form.Item
              label={i18nText("settings", "auto.confirm_new_password")}
              name="confirm_password"
              dependencies={['new_password']}
              rules={[
                { required: true, message: i18nText("settings", "auto.enter_new_password_again") },
                ({ getFieldValue }) => ({
                  validator(_, value) {
                    if (!value || value === getFieldValue('new_password')) {
                      return Promise.resolve();
                    }

                    return Promise.reject(new Error(i18nText("settings", "auto.passwords_do_not_match")));
                  }
                })
              ]}
            >
              <Input.Password />
            </Form.Item>
          </Form>
        </Modal>

      </div>
    </SettingsSectionSurface>
  );
}
