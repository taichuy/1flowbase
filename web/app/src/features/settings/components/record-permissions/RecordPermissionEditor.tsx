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
import type { RecordPermissionRole } from '../../api/identity-access/record-permissions';
import { useRecordPermissionSession } from './use-record-permission-session';
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
  const actor = useAuthStore((state) => state.actor);
  const scope = JSON.stringify([
    props.pluginCode,
    props.resourceCode,
    props.recordId,
    actor?.id,
    actor?.current_workspace_id,
    actor?.effective_display_role
  ]);
  return props.open ? <RecordPermissionSession key={scope} {...props} /> : null;
}

function RecordPermissionSession(props: RecordPermissionEditorProps) {
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
  const {
    data,
    mode,
    grants,
    loading,
    saving,
    error,
    reload,
    setMode,
    selectRoles,
    toggleAction,
    save
  } = useRecordPermissionSession({
    pluginCode,
    resourceCode,
    recordId,
    onSaved
  });

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
            disabled={!data || loading || error === 'conflict'}
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
            <Button size="small" disabled={saving} onClick={reload}>
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
