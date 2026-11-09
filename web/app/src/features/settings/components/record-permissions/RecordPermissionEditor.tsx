import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { i18nText } from '../../../../shared/i18n/text';
import {
  Alert,
  Button,
  Checkbox,
  Form,
  Modal,
  Select,
  Space,
  Typography
} from 'antd';
import {
  fetchRecordPermissions,
  replaceRecordPermissions,
  type RecordPermissionEditorData,
  type RecordPermissionGrant,
  type RecordPermissionMode,
  type RecordPermissionRole
} from '../../api/identity-access/record-permissions';
import { ApiClientError } from '@1flowbase/api-client';
import {
  DataTable,
  type DataTableColumn
} from '../../../../shared/ui/data-table/DataTable';
import { usePersistedDataTableConfiguration } from '../../../../shared/ui/data-table/data-table-state';
import { useAuthStore } from '../../../../state/auth-store';

export interface RecordPermissionEditorProps {
  pluginCode: string;
  resourceCode: string;
  recordId: string;
  open: boolean;
  locale?: string;
  onCancel(): void;
  onSaved(): void;
}

/** The host owns role identity and authorization; this component only edits a draft. */
export function RecordPermissionEditor(props: RecordPermissionEditorProps) {
  const {
    pluginCode,
    resourceCode,
    recordId,
    open,
    locale,
    onCancel,
    onSaved
  } = props;
  const { i18n } = useTranslation();
  const options = { lng: locale ?? i18n.resolvedLanguage ?? i18n.language };
  const t = {
    title: i18nText('settingsRecordPermissions', 'title', options),
    mode: i18nText('settingsRecordPermissions', 'mode', options),
    inherit: i18nText('settingsRecordPermissions', 'inherit', options),
    restricted: i18nText('settingsRecordPermissions', 'restricted', options),
    inherited: i18nText('settingsRecordPermissions', 'inherited', options),
    restriction: i18nText('settingsRecordPermissions', 'restriction', options),
    roles: i18nText('settingsRecordPermissions', 'roles', options),
    role: i18nText('settingsRecordPermissions', 'role', options),
    save: i18nText('settingsRecordPermissions', 'save', options),
    cancel: i18nText('settingsRecordPermissions', 'cancel', options),
    reload: i18nText('settingsRecordPermissions', 'reload', options),
    failed: i18nText('settingsRecordPermissions', 'failed', options),
    conflict: i18nText('settingsRecordPermissions', 'conflict', options),
    actions: {
      view: i18nText('settingsRecordPermissions', 'actions.view', options),
      update: i18nText('settingsRecordPermissions', 'actions.update', options),
      open: i18nText('settingsRecordPermissions', 'actions.open', options),
      delete: i18nText('settingsRecordPermissions', 'actions.delete', options)
    }
  };
  const actor = useAuthStore((state) => state.actor);
  const editorScope = JSON.stringify([
    pluginCode,
    resourceCode,
    recordId,
    actor?.id,
    actor?.current_workspace_id,
    actor?.effective_display_role
  ]);
  const [loadedScope, setLoadedScope] = useState<string | null>(null);
  const [data, setData] = useState<RecordPermissionEditorData | null>(null);
  const [mode, setMode] = useState<RecordPermissionMode>('inherit');
  const [grants, setGrants] = useState<RecordPermissionGrant[]>([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<'failed' | 'conflict' | null>(null);
  const [reload, setReload] = useState(0);

  useEffect(() => {
    if (!open) return;
    const controller = new AbortController();
    setData(null);
    setLoadedScope(null);
    setError(null);
    setLoading(true);
    fetchRecordPermissions(
      {
        plugin_code: pluginCode,
        resource_code: resourceCode,
        record_id: recordId
      },
      undefined,
      controller.signal
    )
      .then((result) => {
        if (controller.signal.aborted) return;
        setData(result);
        setLoadedScope(editorScope);
        setMode(result.mode);
        setGrants(result.grants);
      })
      .catch(() => {
        if (!controller.signal.aborted) setError('failed');
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [pluginCode, resourceCode, recordId, open, reload, editorScope]);

  function selectRoles(roleIds: string[]) {
    setGrants(
      roleIds.map(
        (role_id) =>
          grants.find((grant) => grant.role_id === role_id) ?? {
            role_id,
            actions: []
          }
      )
    );
  }

  function toggleAction(
    role_id: string,
    action_code: string,
    checked: boolean
  ) {
    setGrants((current) =>
      current.map((grant) =>
        grant.role_id === role_id
          ? {
              ...grant,
              actions: checked
                ? [...grant.actions, action_code]
                : grant.actions.filter((action) => action !== action_code)
            }
          : grant
      )
    );
  }

  const columns: DataTableColumn<RecordPermissionRole>[] = [
    {
      key: 'role',
      title: t.role,
      width: 180,
      render: (_value, role) => role.name
    },
    ...(data?.resource.actions ?? []).map((action) => ({
      key: action.action_code,
      title:
        t.actions[action.action_code as keyof typeof t.actions] ?? action.label,
      width: 100,
      render: (_value: unknown, role: RecordPermissionRole) => (
        <Checkbox
          aria-label={`${role.name} ${t.actions[action.action_code as keyof typeof t.actions] ?? action.label}`}
          disabled={saving}
          checked={grants.some(
            (grant) =>
              grant.role_id === role.role_id &&
              grant.actions.includes(action.action_code)
          )}
          onChange={(event) =>
            toggleAction(role.role_id, action.action_code, event.target.checked)
          }
        />
      )
    }))
  ];
  const configuration = usePersistedDataTableConfiguration({
    columns,
    storageKey: 'record-permissions.columns'
  });
  const selectedRoles =
    data?.roles.filter((role) =>
      grants.some((grant) => grant.role_id === role.role_id)
    ) ?? [];

  async function save() {
    if (
      !data ||
      loadedScope !== editorScope ||
      loading ||
      saving ||
      error === 'conflict'
    )
      return;
    setSaving(true);
    setError(null);
    try {
      await replaceRecordPermissions(
        {
          plugin_code: pluginCode,
          resource_code: resourceCode,
          record_id: recordId
        },
        {
          mode,
          grants: mode === 'inherit' ? [] : grants,
          expected_revision: data.revision
        },
        useAuthStore.getState().csrfToken
      );
      onSaved();
    } catch (cause) {
      setError(
        cause instanceof ApiClientError && cause.status === 409
          ? 'conflict'
          : 'failed'
      );
    } finally {
      setSaving(false);
    }
  }

  return (
    <Modal
      title={t.title}
      open={open}
      width={720}
      destroyOnHidden
      closable={!saving}
      maskClosable={!saving}
      onCancel={onCancel}
      footer={
        <Space>
          <Button disabled={saving} onClick={onCancel}>
            {t.cancel}
          </Button>
          <Button
            type="primary"
            loading={saving}
            disabled={
              !data ||
              loadedScope !== editorScope ||
              loading ||
              error === 'conflict'
            }
            onClick={() => void save()}
          >
            {t.save}
          </Button>
        </Space>
      }
    >
      {error && (
        <Alert
          type="error"
          title={t[error]}
          action={
            <Button
              size="small"
              disabled={saving}
              onClick={() => setReload((value) => value + 1)}
            >
              {t.reload}
            </Button>
          }
        />
      )}
      <Form layout="vertical" disabled={loading || saving || !data}>
        <Form.Item label={t.mode} htmlFor="record-permission-mode">
          <Select
            id="record-permission-mode"
            value={mode}
            onChange={setMode}
            options={[
              { value: 'inherit', label: t.inherit },
              { value: 'restricted', label: t.restricted }
            ]}
          />
        </Form.Item>
        <Typography.Paragraph type="secondary">
          {mode === 'inherit' ? t.inherited : t.restriction}
        </Typography.Paragraph>
        {mode === 'restricted' && (
          <>
            <Form.Item label={t.roles} htmlFor="record-permission-roles">
              <Select
                id="record-permission-roles"
                mode="multiple"
                value={grants.map((grant) => grant.role_id)}
                onChange={selectRoles}
                options={data?.roles.map((role) => ({
                  value: role.role_id,
                  label: role.name
                }))}
              />
            </Form.Item>
            <DataTable
              columns={columns}
              configuration={configuration}
              dataSource={selectedRoles}
              rowKey="role_id"
              loading={loading}
              page={1}
              pageSize={Math.max(1, selectedRoles.length)}
              total={selectedRoles.length}
              onPageChange={() => {}}
            />
          </>
        )}
      </Form>
    </Modal>
  );
}
