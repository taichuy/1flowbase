import { fireEvent, render, screen, within } from '@testing-library/react';
import { useState } from 'react';
import { App, ConfigProvider } from 'antd';
import zhCN from 'antd/locale/zh_CN';

import { AppProviders } from '../../../../app/AppProviders';
import { useAuthStore } from '../../../../state/auth-store';
import type { RolePermissionTab } from '../../components/RolePermissionPanel';

export function authenticate() {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'user-1',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'user-1',
      account: 'root',
      email: 'root@example.com',
      phone: null,
      nickname: 'Root',
      name: 'Root',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'root',
      permissions: ['role_permission.manage.all']
    }
  });
}

export function setPreferredLocale(locale: 'zh_Hans' | 'en_US') {
  useAuthStore.setState((state) => ({
    me: state.me ? { ...state.me, preferred_locale: locale } : state.me
  }));
}

export function renderRolePermissionPanel(
  RolePermissionPanel: typeof import('../../components/RolePermissionPanel').RolePermissionPanel,
  canManageRoles = true
) {
  function RolePermissionPanelHarness() {
    const [activePermissionTab, setActivePermissionTab] =
      useState<RolePermissionTab>('console-policy');

    return (
      <RolePermissionPanel
        canManageRoles={canManageRoles}
        activePermissionTab={activePermissionTab}
        onPermissionTabChange={(tab) => setActivePermissionTab(tab)}
      />
    );
  }

  return render(
    <AppProviders>
      <ConfigProvider locale={zhCN}>
        <App>
          <RolePermissionPanelHarness />
        </App>
      </ConfigProvider>
    </AppProviders>
  );
}

async function selectDefaultPolicyScope(
  section: HTMLElement,
  actionLabel: string,
  optionLabel: string
) {
  fireEvent.mouseDown(
    within(section).getByRole('combobox', { name: `${actionLabel} 作用域` })
  );
  const optionMatches = await screen.findAllByTitle(optionLabel);
  fireEvent.click(optionMatches[optionMatches.length - 1]);
}

async function selectPolicyCombobox(
  scope: HTMLElement,
  comboboxName: string,
  optionLabel: string
) {
  fireEvent.mouseDown(
    within(scope).getByRole('combobox', { name: comboboxName })
  );
  const optionMatches = await screen.findAllByTitle(optionLabel);
  fireEvent.click(optionMatches[optionMatches.length - 1]);
}

export const defaultDataPolicy = {
  role_code: 'member',
  default_policy: {
    can_view: true,
    can_create: true,
    can_update: true,
    can_delete: false,
    default_view_scope: 'own',
    default_update_scope: 'scope_all',
    default_delete_scope: 'own'
  },
  model_policies: [
    {
      data_model_id: 'model-orders',
      can_create_override: null,
      view_scope_override: null,
      update_scope_override: 'own',
      delete_scope_override: 'scope_all'
    },
    {
      data_model_id: 'model-customers',
      can_create_override: null,
      view_scope_override: null,
      update_scope_override: null,
      delete_scope_override: null
    }
  ]
};

export const policyModeOptions = [
  { value: 'full', label: '全部操作', description: '授予全部操作' },
  { value: 'custom', label: '按操作配置', description: '仅授予显式操作' }
];

export const allRowScopeOptions = [
  { value: 'disabled', label: '关闭', description: '不授予访问' },
  { value: 'own', label: '仅自己', description: '仅自己的记录' },
  { value: 'scope_all', label: '当前空间', description: '当前空间中的记录' }
];

export function consolePolicyCatalog(
  groups: unknown[],
  {
    locale = 'zh_Hans',
    groupStrategyOptions = policyModeOptions
  }: {
    locale?: 'zh_Hans' | 'en_US';
    groupStrategyOptions?: typeof policyModeOptions;
  } = {}
) {
  return {
    schema_version: '2026-07-15',
    locale,
    settings_order_revision: 0,
    group_strategy_options: groupStrategyOptions,
    groups,
    resources: []
  };
}

export function defaultDataModels() {
  return [
    {
      id: 'model-orders',
      scope_kind: 'workspace',
      scope_id: 'workspace-1',
      code: 'orders',
      title: 'Orders',
      status: 'published',
      runtime_availability: 'available',
      data_source_id: 'source-1',
      source_kind: 'main_source',
      external_resource_key: null,
      external_table_id: null,
      physical_table_name: 'orders',
      acl_namespace: 'data_model:orders',
      audit_namespace: 'data_model:orders',
      builtin_kind: null,
      capabilities: {
        can_delete: true,
        can_add_user_field: true,
        can_update_lifecycle_status: true,
        record: {
          can_list: true,
          can_get: true,
          can_create: true,
          can_update: true,
          can_delete: true
        }
      },
      fields: []
    },
    {
      id: 'model-customers',
      scope_kind: 'workspace',
      scope_id: 'workspace-1',
      code: 'customers',
      title: 'Customers',
      status: 'published',
      runtime_availability: 'available',
      data_source_id: 'source-1',
      source_kind: 'main_source',
      external_resource_key: null,
      external_table_id: null,
      physical_table_name: 'customers',
      acl_namespace: 'data_model:customers',
      audit_namespace: 'data_model:customers',
      builtin_kind: null,
      capabilities: {
        can_delete: true,
        can_add_user_field: true,
        can_update_lifecycle_status: true,
        record: {
          can_list: true,
          can_get: true,
          can_create: true,
          can_update: true,
          can_delete: true
        }
      },
      fields: []
    }
  ];
}
