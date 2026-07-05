import { useEffect, useMemo, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Checkbox, Select, Space, Table, Typography } from 'antd';
import type { ColumnsType } from 'antd/es/table';

import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import {
  fetchSettingsAllDataModels,
  settingsAllDataModelsQueryKey,
  type SettingsDataModel
} from '../../api/data-models';
import {
  fetchSettingsRoleDataPolicy,
  replaceSettingsRoleDataPolicy,
  settingsRoleDataPolicyQueryKey,
  type SettingsRoleDataPolicyScope,
  type SettingsRoleDataPolicyOverrideScope
} from '../../api/roles';

interface DefaultPolicyFormState {
  can_view: boolean;
  can_create: boolean;
  can_update: boolean;
  can_delete: boolean;
  default_view_scope: SettingsRoleDataPolicyScope;
  default_update_scope: SettingsRoleDataPolicyScope;
  default_delete_scope: SettingsRoleDataPolicyScope;
}

interface ModelPolicyFormState {
  data_model_id: string;
  can_create_override: boolean | null;
  view_scope_override: SettingsRoleDataPolicyOverrideScope;
  update_scope_override: SettingsRoleDataPolicyOverrideScope;
  delete_scope_override: SettingsRoleDataPolicyOverrideScope;
}

interface RoleDataPolicySectionProps {
  roleCode: string;
  canEdit: boolean;
  section: 'default-policy' | 'single-model-policy';
  formId: string;
}

const DEFAULT_POLICY_FORM_STATE: DefaultPolicyFormState = {
  can_view: false,
  can_create: false,
  can_update: false,
  can_delete: false,
  default_view_scope: 'own',
  default_update_scope: 'own',
  default_delete_scope: 'own'
};

type DefaultPolicyAction = 'create' | 'view' | 'update' | 'delete';
type OverrideScopeSelectValue = SettingsRoleDataPolicyScope | 'inherit';

const defaultPolicyActions: Array<{
  action: DefaultPolicyAction;
  permissionKey: keyof Pick<
    DefaultPolicyFormState,
    'can_create' | 'can_view' | 'can_update' | 'can_delete'
  >;
  label: string;
  scopeKey?: keyof Pick<
    DefaultPolicyFormState,
    'default_view_scope' | 'default_update_scope' | 'default_delete_scope'
  >;
}> = [
  {
    action: 'create',
    permissionKey: 'can_create',
    label: i18nText("settings", "auto.new")
  },
  {
    action: 'view',
    permissionKey: 'can_view',
    label: i18nText("settings", "auto.view"),
    scopeKey: 'default_view_scope'
  },
  {
    action: 'update',
    permissionKey: 'can_update',
    label: i18nText("settings", "auto.update"),
    scopeKey: 'default_update_scope'
  },
  {
    action: 'delete',
    permissionKey: 'can_delete',
    label: i18nText("settings", "auto.delete"),
    scopeKey: 'default_delete_scope'
  }
];

const defaultPolicyScopes: Array<{
  scope: SettingsRoleDataPolicyScope;
  label: string;
}> = [
  {
    scope: 'own',
    label: i18nText("settings", "auto.own_records")
  },
  {
    scope: 'scope_all',
    label: i18nText("settings", "auto.scope_all_records")
  }
];

const modelOverrideScopeOptions: Array<{
  value: OverrideScopeSelectValue;
  label: string;
}> = [
  {
    value: 'inherit',
    label: i18nText("settings", "auto.inherit")
  },
  {
    value: 'own',
    label: i18nText("settings", "auto.own_records")
  },
  {
    value: 'scope_all',
    label: i18nText("settings", "auto.scope_all_records")
  }
];

interface DefaultPolicyRow {
  action: DefaultPolicyAction;
  label: string;
  permissionKey: keyof Pick<
    DefaultPolicyFormState,
    'can_create' | 'can_view' | 'can_update' | 'can_delete'
  >;
  enabled: boolean;
  scopeKey?: keyof Pick<
    DefaultPolicyFormState,
    'default_view_scope' | 'default_update_scope' | 'default_delete_scope'
  >;
  scopeValue?: SettingsRoleDataPolicyScope;
}

export function RoleDataPolicySection({
  roleCode,
  canEdit,
  section,
  formId
}: RoleDataPolicySectionProps) {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [defaultPolicy, setDefaultPolicy] = useState<DefaultPolicyFormState>(
    DEFAULT_POLICY_FORM_STATE
  );
  const [modelPolicyById, setModelPolicyById] = useState<
    Record<string, ModelPolicyFormState>
  >({});

  const dataPolicyQuery = useQuery({
    queryKey: settingsRoleDataPolicyQueryKey(roleCode),
    queryFn: () => fetchSettingsRoleDataPolicy(roleCode),
    enabled: Boolean(roleCode)
  });

  const dataModelsQuery = useQuery({
    queryKey: settingsAllDataModelsQueryKey,
    queryFn: fetchSettingsAllDataModels
  });

  useEffect(() => {
    const fetchedPolicy = dataPolicyQuery.data;
    if (!fetchedPolicy) {
      return;
    }

    setDefaultPolicy({
      can_view: fetchedPolicy.default_policy.can_view,
      can_create: fetchedPolicy.default_policy.can_create,
      can_update: fetchedPolicy.default_policy.can_update,
      can_delete: fetchedPolicy.default_policy.can_delete,
      default_view_scope: fetchedPolicy.default_policy.default_view_scope,
      default_update_scope: fetchedPolicy.default_policy.default_update_scope,
      default_delete_scope: fetchedPolicy.default_policy.default_delete_scope
    });
  }, [dataPolicyQuery.data]);

  useEffect(() => {
    const fetchedPolicy = dataPolicyQuery.data;
    const fetchedModels = dataModelsQuery.data;
    if (!fetchedPolicy || !fetchedModels) {
      return;
    }

    const fetchedModelPolicyById = new Map(
      fetchedPolicy.model_policies.map((policy) => [
        policy.data_model_id,
        policy
      ])
    );
    const nextModelPolicyById: Record<string, ModelPolicyFormState> = {};

    fetchedModels.forEach((model) => {
      const policy = fetchedModelPolicyById.get(model.id);
      nextModelPolicyById[model.id] = {
        data_model_id: model.id,
        can_create_override: policy?.can_create_override ?? null,
        view_scope_override: policy?.view_scope_override ?? null,
        update_scope_override: policy?.update_scope_override ?? null,
        delete_scope_override: policy?.delete_scope_override ?? null
      };
    });

    setModelPolicyById(nextModelPolicyById);
  }, [dataModelsQuery.data, dataPolicyQuery.data]);

  const replaceDataPolicyMutation = useMutation({
    mutationFn: async () => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return replaceSettingsRoleDataPolicy(
        roleCode,
        {
          default_policy: defaultPolicy,
          model_policies: (dataModelsQuery.data ?? []).map((model) => {
            const policy = modelPolicyById[model.id] ?? {
              data_model_id: model.id,
              can_create_override: null,
              view_scope_override: null,
              update_scope_override: null,
              delete_scope_override: null
            };

            return {
              data_model_id: model.id,
              can_create_override: policy.can_create_override,
              view_scope_override: policy.view_scope_override,
              update_scope_override: policy.update_scope_override,
              delete_scope_override: policy.delete_scope_override
            };
          })
        },
        csrfToken
      );
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: settingsRoleDataPolicyQueryKey(roleCode)
      });
    }
  });

  const setDefaultPermission = (
    key: 'can_view' | 'can_create' | 'can_update' | 'can_delete',
    checked: boolean
  ) => {
    setDefaultPolicy((current) => ({
      ...current,
      [key]: checked
    }));
  };

  const setDefaultScope = (
    key:
      | 'default_view_scope'
      | 'default_update_scope'
      | 'default_delete_scope',
    scope: SettingsRoleDataPolicyScope
  ) => {
    setDefaultPolicy((current) => ({
      ...current,
      [key]: scope
    }));
  };

  const setModelScope = (
    modelId: string,
    key:
      | 'view_scope_override'
      | 'update_scope_override'
      | 'delete_scope_override',
    scope: SettingsRoleDataPolicyOverrideScope
  ) => {
    setModelPolicyById((current) => {
      const currentPolicy = current[modelId] ?? {
        data_model_id: modelId,
        can_create_override: null,
        view_scope_override: null,
        update_scope_override: null,
        delete_scope_override: null
      };

      return {
        ...current,
        [modelId]: {
          ...currentPolicy,
          [key]: scope
        }
      };
    });
  };

  const defaultPolicyRows = useMemo<DefaultPolicyRow[]>(
    () =>
      defaultPolicyActions.map((actionConfig) => ({
        ...actionConfig,
        enabled: defaultPolicy[actionConfig.permissionKey],
        scopeValue: actionConfig.scopeKey
          ? defaultPolicy[actionConfig.scopeKey]
          : undefined
      })),
    [defaultPolicy]
  );

  const modelRows = useMemo(
    () =>
      (dataModelsQuery.data ?? []).map((model) => ({
        ...model,
        policy: modelPolicyById[model.id] ?? {
          data_model_id: model.id,
          can_create_override: null,
          view_scope_override: null,
          update_scope_override: null,
          delete_scope_override: null
        }
      })),
    [dataModelsQuery.data, modelPolicyById]
  );

  const defaultPolicyColumns: ColumnsType<DefaultPolicyRow> = [
    {
      title: i18nText("settings", "auto.operation"),
      dataIndex: 'label',
      key: 'label',
      render: (label: string) => <Typography.Text>{label}</Typography.Text>
    },
    {
      title: i18nText("settings", "auto.enabled"),
      key: 'enabled',
      render: (_value, row) => (
        <Checkbox
          aria-label={`${row.label} ${i18nText("settings", "auto.enabled")}`}
          checked={row.enabled}
          disabled={!canEdit}
          onChange={(event) => {
            setDefaultPermission(row.permissionKey, event.target.checked);
          }}
        />
      )
    },
    {
      title: i18nText("settings", "auto.scope"),
      key: 'scope',
      render: (_value, row) => {
        const scopeKey = row.scopeKey;
        return scopeKey ? (
          <Select
            aria-label={`${row.label} ${i18nText("settings", "auto.scope")}`}
            disabled={!canEdit || !row.enabled}
            onChange={(scope) => {
              setDefaultScope(scopeKey, scope);
            }}
            options={defaultPolicyScopes.map((scopeConfig) => ({
              label: scopeConfig.label,
              value: scopeConfig.scope
            }))}
            style={{ width: 128 }}
            value={row.scopeValue}
          />
        ) : (
          <Typography.Text type="secondary">-</Typography.Text>
        );
      }
    }
  ];

  const renderOverrideScope = (
    actionLabel: string,
    model: SettingsDataModel,
    policyKey:
      | 'view_scope_override'
      | 'update_scope_override'
      | 'delete_scope_override'
  ) => {
    const policy = modelPolicyById[model.id];
    return (
      <Select
        aria-label={`${actionLabel} ${model.title}`}
        disabled={!canEdit}
        onChange={(scope) => {
          const nextScope: SettingsRoleDataPolicyOverrideScope =
            scope === 'inherit' ? null : (scope as SettingsRoleDataPolicyScope);
          setModelScope(model.id, policyKey, nextScope);
        }}
        options={modelOverrideScopeOptions}
        style={{ width: 128 }}
        value={policy?.[policyKey] ?? 'inherit'}
      />
    );
  };

  const setModelCreateOverride = (modelId: string, checked: boolean) => {
    setModelPolicyById((current) => {
      const currentPolicy = current[modelId] ?? {
        data_model_id: modelId,
        can_create_override: null,
        view_scope_override: null,
        update_scope_override: null,
        delete_scope_override: null
      };

      return {
        ...current,
        [modelId]: {
          ...currentPolicy,
          can_create_override: checked
        }
      };
    });
  };

  const columns: ColumnsType<SettingsDataModel & { policy: ModelPolicyFormState }> =
    [
      {
        title: i18nText("settings", "auto.data_model"),
        dataIndex: 'title',
        key: 'title',
        render: (_value, model) => (
          <Space direction="vertical" size={0}>
            <Typography.Text>{model.title}</Typography.Text>
            <Typography.Text type="secondary">{model.code}</Typography.Text>
          </Space>
        )
      },
      {
        title: i18nText("settings", "auto.new"),
        key: 'can_create_override',
        render: (_value, model) => (
          <Checkbox
            aria-label={`${i18nText("settings", "auto.new")} ${model.title}`}
            checked={
              model.policy.can_create_override ?? defaultPolicy.can_create
            }
            disabled={!canEdit}
            onChange={(event) => {
              setModelCreateOverride(model.id, event.target.checked);
            }}
          />
        )
      },
      {
        title: i18nText("settings", "auto.view"),
        key: 'view_scope_override',
        render: (_value, model) =>
          renderOverrideScope(
            i18nText("settings", "auto.view"),
            model,
            'view_scope_override'
          )
      },
      {
        title: i18nText("settings", "auto.update"),
        key: 'update_scope_override',
        render: (_value, model) =>
          renderOverrideScope(
            i18nText("settings", "auto.update"),
            model,
            'update_scope_override'
          )
      },
      {
        title: i18nText("settings", "auto.delete"),
        key: 'delete_scope_override',
        render: (_value, model) =>
          renderOverrideScope(
            i18nText("settings", "auto.delete"),
            model,
            'delete_scope_override'
          )
      }
    ];

  return (
    <section aria-label={i18nText("settings", "auto.data_model_data_policy")}>
      <form
        id={formId}
        onSubmit={(event) => {
          event.preventDefault();
          replaceDataPolicyMutation.mutate();
        }}
      >
      <Space direction="vertical" size="middle" style={{ width: '100%' }}>
        {section === 'default-policy' ? (
          <section aria-label={i18nText("settings", "auto.default_policy")}>
            <Typography.Title level={5} style={{ marginTop: 0 }}>
              {i18nText("settings", "auto.default_policy")}
            </Typography.Title>
            <Table
              columns={defaultPolicyColumns}
              dataSource={defaultPolicyRows}
              loading={dataPolicyQuery.isLoading}
              pagination={false}
              rowKey="action"
              size="small"
            />
          </section>
        ) : (
          <Table
            columns={columns}
            dataSource={modelRows}
            loading={dataPolicyQuery.isLoading || dataModelsQuery.isLoading}
            pagination={false}
            rowKey="id"
            size="small"
          />
        )}
      </Space>
      </form>
    </section>
  );
}
