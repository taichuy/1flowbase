import { useCallback, useEffect, useMemo, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { Form, Tree, App } from 'antd';
import type { TreeDataNode } from 'antd';

import { useAuthStore } from '../../../../state/auth-store';
import {
  fetchSettingsConsolePolicyCatalog,
  settingsConsolePolicyCatalogQueryKey,
  type SettingsConsolePolicyCatalogLocale
} from '../../api/identity-access/permissions';
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
} from '../../api/identity-access/roles';

import { i18nText } from '../../../../shared/i18n/text';
import {
  FALLBACK_APP_LOCALE,
  toAppLocale
} from '../../../../shared/i18n/locales';
import { RoleDataPolicySection } from './RoleDataPolicySection';
import { renderConsolePolicyCatalogTable } from './console-policy-catalog-table';
import {
  disabledConsolePolicyGroup,
  materializeDetailPolicyOperations,
  sameConsolePolicyGroup,
  replaceConsolePolicyGroup,
  type ConsolePolicyCatalogGroup,
  type ConsolePolicyGroup,
  type ConsolePolicyRowOperation
} from './console-policy-draft';
import { useConsoleSettingsOrder } from './useConsoleSettingsOrder';
export function useRolePermissionController({
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
  const { i18n } = useTranslation();
  const consolePolicyCatalogLocale: SettingsConsolePolicyCatalogLocale =
    toAppLocale(i18n.resolvedLanguage) ??
    toAppLocale(i18n.language) ??
    FALLBACK_APP_LOCALE;
  const backendSettingsTabLabel = i18nText('settings', 'auto.backend_setting');
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const workspaceId = useAuthStore(
    (state) => state.actor?.current_workspace_id
  );
  const queryClient = useQueryClient();
  const { message: messageApi } = App.useApp();

  const [searchQuery, setSearchQuery] = useState('');
  const [selectedRoleCode, setSelectedRoleCode] = useState<string | null>(null);

  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [editingRole, setEditingRole] = useState<SettingsRole | null>(null);
  const [consolePolicyGroups, setConsolePolicyGroups] = useState<
    SettingsRoleConsolePolicy['groups']
  >([]);
  const [consolePolicyDetail, setConsolePolicyDetail] = useState<{
    catalogGroup: ConsolePolicyCatalogGroup;
    policyGroup: ConsolePolicyGroup;
  } | null>(null);
  const [selectedConsoleOperationIds, setSelectedConsoleOperationIds] =
    useState<string[]>([]);
  const [consoleOperationPathQuery, setConsoleOperationPathQuery] =
    useState('');

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

  const [localCheckedRouteIds, setLocalCheckedRouteIds] = useState<string[]>(
    []
  );

  const routeKindById = useMemo(() => {
    const kinds = new Map<string, 'group' | 'page' | 'tab'>();
    const collectKinds = (nodes: SettingsRoleFrontstageRoutes['tree']) => {
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
    const deriveGroupChecks = (nodes: SettingsRoleFrontstageRoutes['tree']) => {
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
  }, [roleFrontstageRoutesQuery.data, roleFrontstageRoutesQuery.dataUpdatedAt]);

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

  const consoleSettingsOrder = useConsoleSettingsOrder({
    canManageRoles,
    csrfToken,
    locale: consolePolicyCatalogLocale,
    catalog: consolePolicyCatalogQuery.data,
    queryClient,
    messageApi
  });

  const replaceFrontstageRoutesMutation = useMutation({
    scope: {
      id: `settings-role-frontstage-routes:${selectedRoleCode ?? 'none'}`
    },
    mutationFn: async (input: {
      roleCode: string;
      page_ids: string[];
      tab_ids: string[];
    }) => {
      if (!csrfToken) throw new Error('missing selection');
      return replaceSettingsRoleFrontstageRoutes(
        input.roleCode,
        { page_ids: input.page_ids, tab_ids: input.tab_ids },
        csrfToken
      );
    },
    onSuccess: () => {
      messageApi.success(
        i18nText('settings', 'auto.dynamic_route_permissions_updated')
      );
    },
    onError: () => {
      messageApi.error(
        i18nText('settings', 'auto.permission_policy_update_failed')
      );
    },
    onSettled: async (_, __, input) => {
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: settingsRoleFrontstageRoutesQueryKey(input.roleCode)
        }),
        queryClient.invalidateQueries({ queryKey: ['frontstage', workspaceId] })
      ]);
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
      messageApi.success(
        i18nText('settings', 'auto.role_created_successfully')
      );
      createForm.resetFields();
      setIsCreateModalOpen(false);
      await invalidateRoles();
    },
    onError: () =>
      messageApi.error(i18nText('settings', 'auto.character_creation_failed'))
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
      messageApi.success(
        i18nText('settings', 'auto.role_updated_successfully')
      );
      setEditingRole(null);
      await invalidateRoles();
    },
    onError: () =>
      messageApi.error(i18nText('settings', 'auto.character_update_failed'))
  });

  const deleteMutation = useMutation({
    mutationFn: async (roleCode: string) => {
      if (!csrfToken) throw new Error('missing csrf token');
      return deleteSettingsRole(roleCode, csrfToken);
    },
    onSuccess: async (_, variables) => {
      messageApi.success(i18nText('settings', 'auto.role_deleted'));
      if (selectedRoleCode === variables) {
        setSelectedRoleCode(rolesQuery.data?.[0]?.code ?? null);
      }
      await invalidateRoles();
    },
    onError: () =>
      messageApi.error(i18nText('settings', 'auto.role_deletion_failed'))
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
    (catalogGroup: ConsolePolicyCatalogGroup, forceCustom = false) => {
      const policyGroup = policyGroupForCatalogGroup(catalogGroup);

      setSelectedConsoleOperationIds([]);
      setConsoleOperationPathQuery('');
      setConsolePolicyDetail({
        catalogGroup,
        policyGroup: {
          ...policyGroup,
          strategy: forceCustom ? 'custom' : policyGroup.strategy,
          operations: policyGroup.operations.map((operation) => ({
            ...operation
          }))
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
          strategy: 'custom',
          operations
        }
      };
    });
  };

  const simpleDetailOperations =
    consolePolicyDetail?.catalogGroup.operations.filter(
      (operation) => operation.full_profile.kind === 'simple'
    ) ?? [];
  const filteredDetailOperations =
    consolePolicyDetail?.catalogGroup.operations.filter((operation) =>
      operation.route.path
        .toLowerCase()
        .includes(consoleOperationPathQuery.trim().toLowerCase())
    ) ?? [];
  const visibleSimpleDetailOperations = filteredDetailOperations.filter(
    (operation) => operation.full_profile.kind === 'simple'
  );
  const allSimpleDetailOperationsSelected =
    visibleSimpleDetailOperations.length > 0 &&
    visibleSimpleDetailOperations.every((operation) =>
      selectedConsoleOperationIds.includes(operation.operation_id)
    );

  const updateSelectedConsoleOperations = (enabled: boolean) => {
    if (selectedConsoleOperationIds.length === 0) return;
    const selectedIds = new Set(selectedConsoleOperationIds);
    setConsolePolicyDetail((current) => {
      if (!current) return current;
      const simpleIds = new Set(
        current.catalogGroup.operations
          .filter((operation) => operation.full_profile.kind === 'simple')
          .map((operation) => operation.operation_id)
      );
      const operations = materializeDetailPolicyOperations(
        current.policyGroup,
        current.catalogGroup
      ).map((operation) =>
        operation.kind === 'simple' &&
        simpleIds.has(operation.operation_id) &&
        selectedIds.has(operation.operation_id)
          ? { ...operation, enabled }
          : operation
      );
      return {
        ...current,
        policyGroup: { ...current.policyGroup, strategy: 'custom', operations }
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
    setSelectedConsoleOperationIds([]);
    saveConsolePolicyGroups(nextGroups);
  };

  const permissionTabItems = useMemo(() => {
    const catalogGroups = consolePolicyCatalogQuery.data?.groups ?? [];
    const groupStrategyOptions =
      consolePolicyCatalogQuery.data?.group_strategy_options ?? [];
    const catalogTableContext = {
      catalogGroups,
      groupStrategyOptions,
      policyGroupForCatalogGroup,
      canManageRoles,
      selectedRole,
      replaceConsolePolicyMutation,
      consolePolicyGroups,
      saveConsolePolicyGroups,
      openConsolePolicyDetail,
      consoleSettingsOrder
    };

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
          disabled={
            !canManageRoles ||
            !selectedRole?.is_editable ||
            replaceFrontstageRoutesMutation.isPending
          }
          checkedKeys={displayedCheckedRouteIds}
          treeData={(roleFrontstageRoutesQuery.data?.tree ?? []).map(
            function toNode(node): TreeDataNode {
              return {
                key: node.id,
                title: node.title ?? '未命名',
                children: node.children.map(toNode)
              };
            }
          )}
          onCheck={(_, info) => {
            const descendants: string[] = [];
            const collectDescendants = (node: TreeDataNode) => {
              descendants.push(String(node.key));
              node.children?.forEach(collectDescendants);
            };
            collectDescendants(info.node as TreeDataNode);
            const nextKeys = new Set(
              localCheckedRouteIds.filter(
                (id) => routeKindById.get(id) !== 'group'
              )
            );
            descendants.forEach((key) => {
              if (routeKindById.get(key) === 'group') return;
              if (info.checked) nextKeys.add(key);
              else nextKeys.delete(key);
            });
            const keys = Array.from(nextKeys);
            setLocalCheckedRouteIds(keys);
            if (selectedRole) {
              replaceFrontstageRoutesMutation.mutate({
                roleCode: selectedRole.code,
                page_ids: keys.filter((id) => routeKindById.get(id) === 'page'),
                tab_ids: keys.filter((id) => routeKindById.get(id) === 'tab')
              });
            }
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
          label: backendSettingsTabLabel,
          children: renderConsolePolicyCatalogTable(
            'settings_feature',
            catalogTableContext
          )
        }
      : null;
    const otherPolicyTab = otherGroups.length
      ? {
          key: OTHER_POLICY_TAB,
          label: i18nText('settings', 'auto.others'),
          children: renderConsolePolicyCatalogTable(
            'other',
            catalogTableContext
          )
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
    backendSettingsTabLabel,
    consolePolicyGroups,
    consoleSettingsOrder,
    dataPolicyFormId,
    displayedCheckedRouteIds,
    localCheckedRouteIds,
    openConsolePolicyDetail,
    policyGroupForCatalogGroup,
    replaceConsolePolicyMutation,
    replaceFrontstageRoutesMutation,
    roleFrontstageRoutesQuery.data,
    routeKindById,
    saveConsolePolicyGroups,
    selectedRole
  ]);

  useEffect(() => {
    if (
      consolePolicyCatalogQuery.isLoading ||
      rolesQuery.isLoading ||
      (rolesQuery.data?.length && !selectedRole) ||
      permissionTabItems.length === 0
    ) {
      return;
    }

    const activeTabIsAvailable = permissionTabItems.some(
      (item) => item.key === activePermissionTab
    );
    if (!activeTabIsAvailable) {
      const availableTabKeys = new Set(
        permissionTabItems.map((item) => item.key)
      );
      const policyPeerFallback =
        activePermissionTab === CONSOLE_POLICY_TAB &&
        availableTabKeys.has(OTHER_POLICY_TAB)
          ? OTHER_POLICY_TAB
          : activePermissionTab === OTHER_POLICY_TAB &&
              availableTabKeys.has(CONSOLE_POLICY_TAB)
            ? CONSOLE_POLICY_TAB
            : null;
      onPermissionTabChange(
        policyPeerFallback ?? (permissionTabItems[0].key as RolePermissionTab),
        'replace'
      );
    }
  }, [
    activePermissionTab,
    consolePolicyCatalogQuery.isLoading,
    onPermissionTabChange,
    permissionTabItems,
    rolesQuery.data?.length,
    rolesQuery.isLoading,
    selectedRole
  ]);
  return {
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
  };
}

import {
  CONSOLE_POLICY_TAB,
  OTHER_POLICY_TAB,
  DYNAMIC_ROUTE_TAB,
  ROLE_TABLE_GENERAL_TAB,
  ROLE_TABLE_SINGLE_TAB,
  type RolePermissionTab
} from './role-permission-tabs';
