import { useCallback, useEffect, useMemo, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  Button,
  Checkbox,
  Drawer,
  Form,
  Input,
  Modal,
  Popconfirm,
  Select,
  Space,
  Table,
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
  fetchSettingsConsolePolicyCatalog,
  settingsConsolePolicyCatalogQueryKey,
  type SettingsConsolePolicyCatalog,
  type SettingsConsolePolicyCatalogLocale
} from '../api/permissions';
import {
  createSettingsRole,
  deleteSettingsRole,
  fetchSettingsRoleConsolePolicy,
  fetchSettingsRoleFrontstageRoutes,
  fetchSettingsRoles,
  replaceSettingsRoleConsolePolicy,
  replaceSettingsRoleFrontstageRoutes,
  settingsRoleFrontstageRoutesQueryKey,
  settingsRoleConsolePolicyQueryKey,
  settingsRolesQueryKey,
  updateSettingsRole,
  type SettingsRole,
  type SettingsRoleConsolePolicy,
  type SettingsRoleFrontstageRoutes
} from '../api/roles';
import { SettingsSectionSurface } from './SettingsSectionSurface';
import { i18nText } from '../../../shared/i18n/text';
import { FALLBACK_APP_LOCALE, toAppLocale } from '../../../shared/i18n/locales';
import { RoleDataPolicySection } from './role-permissions/RoleDataPolicySection';

const BACKEND_SYSTEM_SETTINGS_TAB = i18nText(
  'settings',
  'auto.backend_system_settings'
);
const CONSOLE_POLICY_TAB = 'console-policy';
const OTHER_POLICY_TAB = 'other-policy';
const DYNAMIC_ROUTE_TAB = 'dynamic-routes';
const ROLE_TABLE_GENERAL_TAB = 'table-general-policy';
const ROLE_TABLE_SINGLE_TAB = 'table-single-policy';

type ConsolePolicyCatalogGroup = SettingsConsolePolicyCatalog['groups'][number];
type ConsolePolicyCatalogOperation = ConsolePolicyCatalogGroup['operations'][number];
type ConsolePolicyGroup = SettingsRoleConsolePolicy['groups'][number];
type ConsolePolicyOperation = ConsolePolicyGroup['operations'][number];
type ConsolePolicyRowOperation = Extract<
  ConsolePolicyOperation,
  { kind: 'row' }
>;

function disabledConsolePolicyGroup(
  catalogGroup: ConsolePolicyCatalogGroup
): ConsolePolicyGroup {
  return {
    kind: catalogGroup.kind,
    group_id: catalogGroup.group_id,
    mode: 'disabled',
    operations: []
  };
}

function policyOperationFromFullProfile(
  operation: ConsolePolicyCatalogOperation
): ConsolePolicyOperation {
  if (operation.full_profile.kind === 'simple') {
    return {
      operation_id: operation.operation_id,
      kind: 'simple',
      enabled: operation.full_profile.enabled
    };
  }

  return {
    operation_id: operation.operation_id,
    kind: 'row',
    scope: operation.full_profile.scope
  };
}

function disabledConsolePolicyOperation(
  operation: ConsolePolicyCatalogOperation
): ConsolePolicyOperation {
  if (operation.full_profile.kind === 'simple') {
    return {
      operation_id: operation.operation_id,
      kind: 'simple',
      enabled: false
    };
  }

  return {
    operation_id: operation.operation_id,
    kind: 'row',
    scope: 'disabled'
  };
}

function matchesCatalogOperationKind(
  policyOperation: ConsolePolicyOperation,
  catalogOperation: ConsolePolicyCatalogOperation
) {
  return policyOperation.kind === catalogOperation.full_profile.kind;
}

function detailPolicyOperation(
  policyGroup: ConsolePolicyGroup,
  catalogOperation: ConsolePolicyCatalogOperation
): ConsolePolicyOperation {
  if (policyGroup.mode === 'full') {
    return policyOperationFromFullProfile(catalogOperation);
  }

  if (policyGroup.mode === 'disabled') {
    return disabledConsolePolicyOperation(catalogOperation);
  }

  const storedOperation = policyGroup.operations.find(
    (operation) => operation.operation_id === catalogOperation.operation_id
  );
  if (storedOperation && matchesCatalogOperationKind(storedOperation, catalogOperation)) {
    return storedOperation;
  }

  return disabledConsolePolicyOperation(catalogOperation);
}

function materializeDetailPolicyOperations(
  policyGroup: ConsolePolicyGroup,
  catalogGroup: ConsolePolicyCatalogGroup
) {
  return catalogGroup.operations.map((catalogOperation) =>
    detailPolicyOperation(policyGroup, catalogOperation)
  );
}

function sameConsolePolicyGroup(
  left: Pick<ConsolePolicyGroup, 'kind' | 'group_id'>,
  right: Pick<ConsolePolicyGroup, 'kind' | 'group_id'>
) {
  return left.kind === right.kind && left.group_id === right.group_id;
}

function replaceConsolePolicyGroup(
  groups: ConsolePolicyGroup[],
  nextGroup: ConsolePolicyGroup
) {
  const existingIndex = groups.findIndex((group) =>
    sameConsolePolicyGroup(group, nextGroup)
  );
  if (existingIndex === -1) {
    return [...groups, nextGroup];
  }

  return groups.map((group, index) =>
    index === existingIndex ? nextGroup : group
  );
}

export function RolePermissionPanel({
  canManageRoles
}: {
  canManageRoles: boolean;
}) {
  const { i18n } = useTranslation();
  const consolePolicyCatalogLocale: SettingsConsolePolicyCatalogLocale =
    toAppLocale(i18n.resolvedLanguage) ??
    toAppLocale(i18n.language) ??
    FALLBACK_APP_LOCALE;
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [messageApi, contextHolder] = message.useMessage();

  const [searchQuery, setSearchQuery] = useState('');
  const [selectedRoleCode, setSelectedRoleCode] = useState<string | null>(null);
  const [activePermissionTab, setActivePermissionTab] =
    useState(CONSOLE_POLICY_TAB);

  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [editingRole, setEditingRole] = useState<SettingsRole | null>(null);
  const [consolePolicyGroups, setConsolePolicyGroups] = useState<
    SettingsRoleConsolePolicy['groups']
  >([]);
  const [consolePolicyDetail, setConsolePolicyDetail] = useState<{
    catalogGroup: ConsolePolicyCatalogGroup;
    policyGroup: ConsolePolicyGroup;
  } | null>(null);

  const [createForm] = Form.useForm();
  const [editForm] = Form.useForm();

  // Queries
  const rolesQuery = useQuery({
    queryKey: settingsRolesQueryKey,
    queryFn: fetchSettingsRoles
  });

  const consolePolicyCatalogQuery = useQuery({
    queryKey: settingsConsolePolicyCatalogQueryKey(consolePolicyCatalogLocale),
    queryFn: () => fetchSettingsConsolePolicyCatalog(consolePolicyCatalogLocale)
  });

  const roleFrontstageRoutesQuery = useQuery({
    queryKey: settingsRoleFrontstageRoutesQueryKey(selectedRoleCode ?? 'none'),
    queryFn: () => fetchSettingsRoleFrontstageRoutes(selectedRoleCode ?? ''),
    enabled: Boolean(selectedRoleCode)
  });

  const roleConsolePolicyQuery = useQuery({
    queryKey: settingsRoleConsolePolicyQueryKey(selectedRoleCode ?? 'none'),
    queryFn: () => fetchSettingsRoleConsolePolicy(selectedRoleCode ?? ''),
    enabled: Boolean(selectedRoleCode)
  });

  const [localCheckedRouteIds, setLocalCheckedRouteIds] = useState<string[]>([]);

  const routeKindById = useMemo(() => {
    const kinds = new Map<string, 'group' | 'page' | 'tab'>();
    const collectKinds = (
      nodes: SettingsRoleFrontstageRoutes['tree']
    ) => {
      nodes.forEach((node) => {
        kinds.set(node.id, node.kind);
        collectKinds(node.children);
      });
    };
    collectKinds(roleFrontstageRoutesQuery.data?.tree ?? []);
    return kinds;
  }, [roleFrontstageRoutesQuery.data?.tree]);

  const displayedCheckedRouteIds = useMemo(() => {
    const checkedIds = new Set(localCheckedRouteIds);
    const deriveGroupChecks = (
      nodes: SettingsRoleFrontstageRoutes['tree']
    ) => {
      nodes.forEach((node) => {
        deriveGroupChecks(node.children);
        if (
          node.kind === 'group' &&
          node.children.length > 0 &&
          node.children.every((child) => checkedIds.has(child.id))
        ) {
          checkedIds.add(node.id);
        }
      });
    };
    deriveGroupChecks(roleFrontstageRoutesQuery.data?.tree ?? []);
    return Array.from(checkedIds);
  }, [localCheckedRouteIds, roleFrontstageRoutesQuery.data?.tree]);

  useEffect(() => {
    setLocalCheckedRouteIds([
      ...(roleFrontstageRoutesQuery.data?.checked_page_ids ?? []),
      ...(roleFrontstageRoutesQuery.data?.checked_tab_ids ?? [])
    ]);
  }, [roleFrontstageRoutesQuery.data]);

  useEffect(() => {
    setConsolePolicyGroups(roleConsolePolicyQuery.data?.groups ?? []);
  }, [roleConsolePolicyQuery.data?.groups]);

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

  const invalidateRoles = async () => {
    await queryClient.invalidateQueries({ queryKey: settingsRolesQueryKey });
  };

  const replaceConsolePolicyMutation = useMutation({
    mutationFn: async (groups: SettingsRoleConsolePolicy['groups']) => {
      if (!csrfToken || !selectedRoleCode) throw new Error('missing selection');
      return replaceSettingsRoleConsolePolicy(
        selectedRoleCode,
        { groups },
        csrfToken
      );
    },
    onSuccess: async () => {
      messageApi.success(
        i18nText('settings', 'auto.permission_policy_updated_successfully')
      );
      if (selectedRoleCode) {
        await queryClient.invalidateQueries({
          queryKey: settingsRoleConsolePolicyQueryKey(selectedRoleCode)
        });
      }
    },
    onError: () => {
      messageApi.error(
        i18nText('settings', 'auto.permission_policy_update_failed')
      );
      setConsolePolicyGroups(roleConsolePolicyQuery.data?.groups ?? []);
    }
  });

  const replaceFrontstageRoutesMutation = useMutation({
    scope: {
      id: `settings-role-frontstage-routes:${selectedRoleCode ?? 'none'}`
    },
    mutationFn: async (routeIds: string[]) => {
      if (!csrfToken || !selectedRoleCode || !roleFrontstageRoutesQuery.data)
        throw new Error('missing selection');
      const tabIds = new Set<string>();
      const collectTabs = (nodes: typeof roleFrontstageRoutesQuery.data.tree) => {
        for (const node of nodes) { if (node.kind === 'tab') tabIds.add(node.id); collectTabs(node.children); }
      };
      collectTabs(roleFrontstageRoutesQuery.data.tree);
      return replaceSettingsRoleFrontstageRoutes(selectedRoleCode, {
        page_ids: routeIds.filter((id) => routeKindById.get(id) === 'page'),
        tab_ids: routeIds.filter((id) => tabIds.has(id))
      }, csrfToken);
    },
    onSuccess: () => messageApi.success('动态路由权限已更新'),
    onError: () => setLocalCheckedRouteIds([
      ...(roleFrontstageRoutesQuery.data?.checked_page_ids ?? []),
      ...(roleFrontstageRoutesQuery.data?.checked_tab_ids ?? [])
    ])
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

  const saveConsolePolicyGroups = useCallback(
    (groups: SettingsRoleConsolePolicy['groups']) => {
      setConsolePolicyGroups(groups);
      replaceConsolePolicyMutation.mutate(groups);
    },
    [replaceConsolePolicyMutation]
  );

  const policyGroupForCatalogGroup = useCallback(
    (catalogGroup: ConsolePolicyCatalogGroup) =>
      consolePolicyGroups.find((group) =>
        sameConsolePolicyGroup(group, catalogGroup)
      ) ?? disabledConsolePolicyGroup(catalogGroup),
    [consolePolicyGroups]
  );

  const openConsolePolicyDetail = useCallback(
    (catalogGroup: ConsolePolicyCatalogGroup) => {
      const policyGroup = policyGroupForCatalogGroup(catalogGroup);

      setConsolePolicyDetail({
        catalogGroup,
        policyGroup: {
          ...policyGroup,
          operations: policyGroup.operations.map((operation) => ({ ...operation }))
        }
      });
    },
    [policyGroupForCatalogGroup]
  );

  const updateConsolePolicyDetailOperation = (
    operationId: string,
    value: boolean | ConsolePolicyRowOperation['scope']
  ) => {
    setConsolePolicyDetail((current) => {
      if (!current) return current;
      const catalogOperation = current.catalogGroup.operations.find(
        (operation) => operation.operation_id === operationId
      );
      if (!catalogOperation) return current;

      const nextPolicyOperation =
        catalogOperation.full_profile.kind === 'simple'
          ? {
              operation_id: operationId,
              kind: 'simple' as const,
              enabled: Boolean(value)
            }
          : {
              operation_id: operationId,
              kind: 'row' as const,
              scope: value as ConsolePolicyRowOperation['scope']
            };
      const operations = materializeDetailPolicyOperations(
        current.policyGroup,
        current.catalogGroup
      );
      const existingOperationIndex = operations.findIndex(
        (operation) => operation.operation_id === operationId
      );
      if (existingOperationIndex === -1) {
        operations.push(nextPolicyOperation);
      } else {
        operations[existingOperationIndex] = nextPolicyOperation;
      }

      return {
        ...current,
        policyGroup: {
          ...current.policyGroup,
          mode: 'custom',
          operations
        }
      };
    });
  };

  const saveConsolePolicyDetail = () => {
    if (!consolePolicyDetail) return;
    const nextGroups = replaceConsolePolicyGroup(
      consolePolicyGroups,
      consolePolicyDetail.policyGroup
    );
    setConsolePolicyDetail(null);
    saveConsolePolicyGroups(nextGroups);
  };

  const permissionTabItems = useMemo(() => {
    const catalogGroups = consolePolicyCatalogQuery.data?.groups ?? [];
    const groupModeOptions =
      consolePolicyCatalogQuery.data?.group_mode_options ?? [];
    const policyTableRows = (kind: ConsolePolicyCatalogGroup['kind']) =>
      catalogGroups
        .filter((catalogGroup) => catalogGroup.kind === kind)
        .map((catalogGroup, displayKey) => ({
          display_key: displayKey,
          catalogGroup,
          policyGroup: policyGroupForCatalogGroup(catalogGroup)
        }));

    const renderConsolePolicyTable = (
      kind: ConsolePolicyCatalogGroup['kind']
    ) => (
      <Table
        rowKey="display_key"
        pagination={false}
        dataSource={policyTableRows(kind)}
        columns={[
          {
            title: i18nText('settings', 'auto.backend_setting'),
            key: 'backend-setting',
            render: (
              _: unknown,
              row: ReturnType<typeof policyTableRows>[number]
            ) => (
              <Space direction="vertical" size={0}>
                <Typography.Text strong>{row.catalogGroup.label}</Typography.Text>
                {row.catalogGroup.description ? (
                  <Typography.Text type="secondary">
                    {row.catalogGroup.description}
                  </Typography.Text>
                ) : null}
              </Space>
            )
          },
          {
            title: i18nText('settings', 'auto.grant_access'),
            key: 'grant-access',
            width: 160,
            align: 'center' as const,
            render: (
              _: unknown,
              row: ReturnType<typeof policyTableRows>[number]
            ) => (
              <Checkbox
                aria-label={i18nText(
                  'settings',
                  'auto.grant_backend_setting_access',
                  { value1: row.catalogGroup.label }
                )}
                disabled={!canManageRoles || !selectedRole?.is_editable}
                checked={row.policyGroup.mode !== 'disabled'}
                onChange={(event) => {
                  const nextGroup: ConsolePolicyGroup = {
                    ...row.policyGroup,
                    mode: event.target.checked ? 'full' : 'disabled',
                    operations: []
                  };
                  const nextGroups = replaceConsolePolicyGroup(
                    consolePolicyGroups,
                    nextGroup
                  );
                  saveConsolePolicyGroups(nextGroups);
                }}
              />
            )
          },
          {
            title: i18nText('settings', 'auto.permission_policy_summary'),
            key: 'policy-summary',
            render: (
              _: unknown,
              row: ReturnType<typeof policyTableRows>[number]
            ) => {
              const modeOption = groupModeOptions.find(
                (option) => option.value === row.policyGroup.mode
              );
              return modeOption ? <Tag>{modeOption.label}</Tag> : null;
            }
          },
          {
            title: i18nText('settings', 'auto.operation'),
            key: 'policy-detail',
            render: (
              _: unknown,
              row: ReturnType<typeof policyTableRows>[number]
            ) => {
              const fullModeOption = groupModeOptions.find(
                (option) => option.value === 'full'
              );
              const restoreFullLabel = fullModeOption
                ? `${i18nText('settings', 'auto.restore')} ${fullModeOption.label}`
                : null;
              const canEdit = canManageRoles && selectedRole?.is_editable;
              return (
                <Space size={0}>
                  {row.policyGroup.mode === 'custom' && restoreFullLabel ? (
                    <Button
                      type="link"
                      aria-label={restoreFullLabel}
                      disabled={!canEdit}
                      onClick={() =>
                        saveConsolePolicyGroups(
                          replaceConsolePolicyGroup(consolePolicyGroups, {
                            ...row.policyGroup,
                            mode: 'full',
                            operations: []
                          })
                        )
                      }
                    >
                      {restoreFullLabel}
                    </Button>
                  ) : null}
                  <Button
                    type="link"
                    aria-label={i18nText(
                      'settings',
                      'auto.permission_policy_details',
                      { value1: row.catalogGroup.label }
                    )}
                    disabled={!canEdit}
                    onClick={() => openConsolePolicyDetail(row.catalogGroup)}
                  >
                    {i18nText('settings', 'auto.permission_policy_details_text')}
                  </Button>
                </Space>
              );
            }
          }
        ]}
      />
    );

    const defaultDataPolicyTab = selectedRole
      ? {
          key: ROLE_TABLE_GENERAL_TAB,
          label: i18nText('settings', 'auto.table_general_configuration'),
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
          label: i18nText('settings', 'auto.table_single_configuration'),
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

    const dynamicRouteTab = {
      key: DYNAMIC_ROUTE_TAB,
      label: i18nText('settings', 'auto.dynamic_routes'),
      children: (
        <Tree
          checkable
          checkStrictly
          disabled={!canManageRoles || !selectedRole?.is_editable}
          checkedKeys={displayedCheckedRouteIds}
          treeData={(roleFrontstageRoutesQuery.data?.tree ?? []).map(function toNode(node): TreeDataNode {
            return {
              key: node.id,
              title: node.title ?? '未命名',
              children: node.children.map(toNode)
            };
          })}
          onCheck={(_, info) => {
            const descendants: string[] = [];
            const collectDescendants = (node: TreeDataNode) => {
              descendants.push(String(node.key));
              node.children?.forEach(collectDescendants);
            };
            collectDescendants(info.node as TreeDataNode);
            const nextKeys = new Set(
              localCheckedRouteIds.filter((id) => routeKindById.get(id) !== 'group')
            );
            descendants.forEach((key) => {
              if (routeKindById.get(key) === 'group') return;
              if (info.checked) nextKeys.add(key);
              else nextKeys.delete(key);
            });
            const keys = Array.from(nextKeys);
            setLocalCheckedRouteIds(keys);
            replaceFrontstageRoutesMutation.mutate(keys);
          }}
        />
      )
    };

    const settingsFeatureGroups = catalogGroups.filter(
      (group) => group.kind === 'settings_feature'
    );
    const otherGroups = catalogGroups.filter((group) => group.kind === 'other');
    const backendSettingsTab = settingsFeatureGroups.length
      ? {
          key: CONSOLE_POLICY_TAB,
          label: BACKEND_SYSTEM_SETTINGS_TAB,
          children: renderConsolePolicyTable('settings_feature')
        }
      : null;
    const otherPolicyTab = otherGroups.length
      ? {
          key: OTHER_POLICY_TAB,
          label: i18nText('settings', 'auto.others'),
          children: renderConsolePolicyTable('other')
        }
      : null;

    return [
      dynamicRouteTab,
      ...(defaultDataPolicyTab ? [defaultDataPolicyTab] : []),
      ...(singleModelPolicyTab ? [singleModelPolicyTab] : []),
      ...(backendSettingsTab ? [backendSettingsTab] : []),
      ...(otherPolicyTab ? [otherPolicyTab] : [])
    ];
  }, [
    canManageRoles,
    consolePolicyCatalogQuery.data,
    consolePolicyGroups,
    dataPolicyFormId,
    displayedCheckedRouteIds,
    localCheckedRouteIds,
    openConsolePolicyDetail,
    policyGroupForCatalogGroup,
    replaceFrontstageRoutesMutation,
    roleFrontstageRoutesQuery.data,
    routeKindById,
    saveConsolePolicyGroups,
    selectedRole
  ]);

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
                  {consolePolicyCatalogQuery.isLoading ||
                  roleConsolePolicyQuery.isLoading ? (
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

        <Drawer
          title={
            consolePolicyDetail
              ? i18nText('settings', 'auto.permission_policy_detail_title', {
                  value1: consolePolicyDetail.catalogGroup.label
                })
              : undefined
          }
          open={Boolean(consolePolicyDetail)}
          onClose={() => setConsolePolicyDetail(null)}
          width={640}
          destroyOnHidden
          footer={
            <Space style={{ display: 'flex', justifyContent: 'flex-end' }}>
              <Button onClick={() => setConsolePolicyDetail(null)}>
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
            <Space direction="vertical" size="large" style={{ width: '100%' }}>
              {consolePolicyDetail.catalogGroup.description ? (
                <Typography.Paragraph type="secondary">
                  {consolePolicyDetail.catalogGroup.description}
                </Typography.Paragraph>
              ) : null}
              <Table
                rowKey="display_key"
                pagination={false}
                dataSource={[...consolePolicyDetail.catalogGroup.operations]
                  .sort((left, right) => left.order - right.order)
                  .map((operation, displayKey) => ({
                    ...operation,
                    display_key: displayKey
                  }))}
                columns={[
                  {
                    title: i18nText('settings', 'auto.permission_policy_operation'),
                    key: 'operation',
                    render: (
                      _: unknown,
                      operation: ConsolePolicyCatalogGroup['operations'][number]
                    ) => (
                      <Space direction="vertical" size={0}>
                        <Typography.Text>{operation.label}</Typography.Text>
                        {operation.description ? (
                          <Typography.Text type="secondary">
                            {operation.description}
                          </Typography.Text>
                        ) : null}
                      </Space>
                    )
                  },
                  {
                    title: i18nText('settings', 'auto.permission_policy_scope'),
                    key: 'policy',
                    render: (
                      _: unknown,
                      operation: ConsolePolicyCatalogGroup['operations'][number]
                    ) => {
                      const policyOperation =
                        detailPolicyOperation(
                          consolePolicyDetail.policyGroup,
                          operation
                        );

                      if (operation.full_profile.kind === 'simple') {
                        return (
                          <Checkbox
                            aria-label={operation.label}
                            checked={
                              policyOperation.kind === 'simple' &&
                              policyOperation.enabled
                            }
                            disabled={
                              !canManageRoles ||
                              !selectedRole?.is_editable ||
                              replaceConsolePolicyMutation.isPending
                            }
                            onChange={(event) =>
                              updateConsolePolicyDetailOperation(
                                operation.operation_id,
                                event.target.checked
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
                          aria-label={`${operation.label} ${i18nText(
                            'settings',
                            'auto.permission_policy_scope'
                          )}`}
                          value={scope}
                          disabled={
                            !canManageRoles ||
                            !selectedRole?.is_editable ||
                            replaceConsolePolicyMutation.isPending
                          }
                          options={operation.allowed_row_scopes.map((option) => ({
                            value: option.value,
                            label: option.label
                          }))}
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
        </Drawer>
      </div>
    </SettingsSectionSurface>
  );
}
