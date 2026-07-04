import { Button, Checkbox, Form, Select, Space, Table, Tag } from 'antd';
import type { ColumnsType } from 'antd/es/table';
import { useEffect, useState } from 'react';

import type {
  SettingsDataModelScopeGrant,
  UpdateSettingsDataModelScopeGrantInput
} from '../../api/data-models';
import { i18nText } from '../../../../shared/i18n/text';

const workspaceProfileOptions = ['owner', 'scope_all'].map((value) => ({
  label: value,
  value
}));

const systemProfileOptions = ['owner', 'scope_all', 'system_all'].map((value) => ({
  label: value,
  value
}));

type DraftGrant = SettingsDataModelScopeGrant & {
  confirm_unsafe_external_source_system_all: boolean;
};

export function DataModelPermissionsTab({
  grants,
  loading,
  saving,
  onSave
}: {
  grants: SettingsDataModelScopeGrant[];
  loading: boolean;
  saving: boolean;
  onSave: (
    grant: SettingsDataModelScopeGrant,
    input: UpdateSettingsDataModelScopeGrantInput
  ) => void;
}) {
  const [drafts, setDrafts] = useState<DraftGrant[]>([]);

  useEffect(() => {
    setDrafts(
      grants.map((grant) => ({
        ...grant,
        confirm_unsafe_external_source_system_all: false
      }))
    );
  }, [grants]);

  const updateDraft = (grantId: string, patch: Partial<DraftGrant>) => {
    setDrafts((current) =>
      current.map((grant) =>
        grant.id === grantId ? { ...grant, ...patch } : grant
      )
    );
  };

  const columns: ColumnsType<DraftGrant> = [
    {
      title: 'Scope',
      key: 'scope',
      render: (_, grant) => (
        <Space direction="vertical" size={2}>
          <Tag>{grant.scope_kind}</Tag>
          <span>{grant.scope_id}</span>
        </Space>
      )
    },
    {
      title: 'Permission',
      dataIndex: 'permission_profile',
      key: 'permission_profile',
      render: (_, grant) => (
        <Select
          aria-label={i18nText("settings", "auto.permissions", { value1: grant.id })}
          value={grant.permission_profile}
          options={
            grant.scope_kind === 'system'
              ? systemProfileOptions
              : workspaceProfileOptions
          }
          onChange={(value) => updateDraft(grant.id, { permission_profile: value })}
        />
      )
    },
    {
      title: 'Enabled',
      dataIndex: 'enabled',
      key: 'enabled',
      render: (_, grant) => (
        <Checkbox
          checked={grant.enabled}
          onChange={(event) =>
            updateDraft(grant.id, { enabled: event.target.checked })
          }
        />
      )
    },
    {
      title: 'Unsafe confirmation',
      key: 'confirm',
      render: (_, grant) => (
        <Checkbox
          checked={grant.confirm_unsafe_external_source_system_all}
          disabled={grant.permission_profile !== 'system_all'}
          onChange={(event) =>
            updateDraft(grant.id, {
              confirm_unsafe_external_source_system_all: event.target.checked
            })
          }
        >
          {i18nText("settings", "auto.confirm_system_all")}</Checkbox>
      )
    }
  ];

  return (
    <Form layout="vertical">
      <Table
        rowKey="id"
        size="small"
        loading={loading}
        dataSource={drafts}
        columns={columns}
        pagination={false}
      />
      <div className="data-model-panel__actions">
        <Button
          type="primary"
          loading={saving}
          disabled={drafts.length === 0}
          onClick={() => {
            drafts.forEach((draft) => {
              const original = grants.find((grant) => grant.id === draft.id);
              if (!original) return;
              onSave(original, {
                enabled: draft.enabled,
                permission_profile: draft.permission_profile,
                confirm_unsafe_external_source_system_all:
                  draft.confirm_unsafe_external_source_system_all
              });
            });
          }}
        >
          {i18nText("settings", "auto.save_permissions")}</Button>
      </div>
    </Form>
  );
}
