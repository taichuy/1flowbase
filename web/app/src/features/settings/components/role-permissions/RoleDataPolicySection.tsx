import { useEffect, useMemo, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Button, Divider, Radio, Space, Switch, Table, Typography } from 'antd';
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
  view_scope_override: SettingsRoleDataPolicyOverrideScope;
  update_scope_override: SettingsRoleDataPolicyOverrideScope;
  delete_scope_override: SettingsRoleDataPolicyOverrideScope;
}

interface RoleDataPolicySectionProps {
  roleCode: string;
  canEdit: boolean;
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

export function RoleDataPolicySection({
  roleCode,
  canEdit
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
              view_scope_override: null,
              update_scope_override: null,
              delete_scope_override: null
            };

            return {
              data_model_id: model.id,
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

  const modelRows = useMemo(
    () =>
      (dataModelsQuery.data ?? []).map((model) => ({
        ...model,
        policy: modelPolicyById[model.id] ?? {
          data_model_id: model.id,
          view_scope_override: null,
          update_scope_override: null,
          delete_scope_override: null
        }
      })),
    [dataModelsQuery.data, modelPolicyById]
  );

  const renderDefaultScope = (
    actionLabel: string,
    policyKey:
      | 'default_view_scope'
      | 'default_update_scope'
      | 'default_delete_scope',
    enabled: boolean
  ) => (
    <Radio.Group
      aria-label={`${actionLabel} ${i18nText("settings", "auto.scope")}`}
      disabled={!canEdit || !enabled}
      onChange={(event) => {
        setDefaultScope(policyKey, event.target.value);
      }}
      value={defaultPolicy[policyKey]}
    >
      <Radio
        aria-label={`${actionLabel} ${i18nText("settings", "auto.own_records")}`}
        value="own"
      >
        {i18nText("settings", "auto.own_records")}
      </Radio>
      <Radio
        aria-label={`${actionLabel} ${i18nText("settings", "auto.scope_all_records")}`}
        value="scope_all"
      >
        {i18nText("settings", "auto.scope_all_records")}
      </Radio>
    </Radio.Group>
  );

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
      <Radio.Group
        aria-label={`${actionLabel} ${model.title}`}
        disabled={!canEdit}
        onChange={(event) => {
          setModelScope(model.id, policyKey, event.target.value);
        }}
        value={policy?.[policyKey] ?? null}
      >
        <Radio
          aria-label={`${actionLabel} ${i18nText("settings", "auto.inherit")}`}
          value={null}
        >
          {i18nText("settings", "auto.inherit")}
        </Radio>
        <Radio
          aria-label={`${actionLabel} ${i18nText("settings", "auto.own_records")}`}
          value="own"
        >
          {i18nText("settings", "auto.own_records")}
        </Radio>
        <Radio
          aria-label={`${actionLabel} ${i18nText("settings", "auto.scope_all_records")}`}
          value="scope_all"
        >
          {i18nText("settings", "auto.scope_all_records")}
        </Radio>
      </Radio.Group>
    );
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
      <Divider orientation="left">
        {i18nText("settings", "auto.data_model_data_policy")}
      </Divider>

      <Space direction="vertical" size="middle" style={{ width: '100%' }}>
        <section aria-label={i18nText("settings", "auto.default_policy")}>
          <Typography.Title level={5} style={{ marginTop: 0 }}>
            {i18nText("settings", "auto.default_policy")}
          </Typography.Title>
          <Space direction="vertical" size="small">
            <Space wrap>
              <Switch
                aria-label={i18nText("settings", "auto.view")}
                checked={defaultPolicy.can_view}
                disabled={!canEdit}
                onChange={(checked) => {
                  setDefaultPermission('can_view', checked);
                }}
                onClick={(checked) => {
                  setDefaultPermission('can_view', checked);
                }}
              />
              <Typography.Text>{i18nText("settings", "auto.view")}</Typography.Text>
              {renderDefaultScope(
                i18nText("settings", "auto.view"),
                'default_view_scope',
                defaultPolicy.can_view
              )}
            </Space>
            <Space wrap>
              <Switch
                aria-label={i18nText("settings", "auto.new")}
                checked={defaultPolicy.can_create}
                disabled={!canEdit}
                onChange={(checked) => {
                  setDefaultPermission('can_create', checked);
                }}
                onClick={(checked) => {
                  setDefaultPermission('can_create', checked);
                }}
              />
              <Typography.Text>{i18nText("settings", "auto.new")}</Typography.Text>
            </Space>
            <Space wrap>
              <Switch
                aria-label={i18nText("settings", "auto.update")}
                checked={defaultPolicy.can_update}
                disabled={!canEdit}
                onChange={(checked) => {
                  setDefaultPermission('can_update', checked);
                }}
                onClick={(checked) => {
                  setDefaultPermission('can_update', checked);
                }}
              />
              <Typography.Text>{i18nText("settings", "auto.update")}</Typography.Text>
              {renderDefaultScope(
                i18nText("settings", "auto.update"),
                'default_update_scope',
                defaultPolicy.can_update
              )}
            </Space>
            <Space wrap>
              <Switch
                aria-label={i18nText("settings", "auto.delete")}
                checked={defaultPolicy.can_delete}
                disabled={!canEdit}
                onChange={(checked) => {
                  setDefaultPermission('can_delete', checked);
                }}
                onClick={(checked) => {
                  setDefaultPermission('can_delete', checked);
                }}
              />
              <Typography.Text>{i18nText("settings", "auto.delete")}</Typography.Text>
              {renderDefaultScope(
                i18nText("settings", "auto.delete"),
                'default_delete_scope',
                defaultPolicy.can_delete
              )}
            </Space>
          </Space>
        </section>

        <Table
          columns={columns}
          dataSource={modelRows}
          loading={dataPolicyQuery.isLoading || dataModelsQuery.isLoading}
          pagination={false}
          rowKey="id"
          size="small"
        />

        <div style={{ textAlign: 'right' }}>
          <Button
            disabled={!canEdit}
            loading={replaceDataPolicyMutation.isPending}
            onClick={() => replaceDataPolicyMutation.mutate()}
            type="primary"
          >
            {i18nText("settings", "auto.save_data_policy")}
          </Button>
        </div>
      </Space>
    </section>
  );
}
