import { useEffect, useState } from 'react';
import { Button, Drawer, Form, Input, App, Select } from 'antd';
import {
  createSettingsFileTable,
  type SettingsFileStorage,
  type SettingsFileTable
} from '../api/file-management';
import { useAuthStore } from '../../../state/auth-store';
import { i18nText } from '../../../shared/i18n/text';

type DrawerMode = 'create' | 'view' | 'edit';

interface FileTableDrawerProps {
  open: boolean;
  mode: DrawerMode;
  record: SettingsFileTable | null;
  storages: SettingsFileStorage[];
  onClose: () => void;
  onSuccess: () => void;
  onUpdateBinding: (tableId: string, storageId: string) => Promise<void>;
}

interface TableFormValues {
  code: string;
  title: string;
  bound_storage_id: string;
}

export function FileTableDrawer({
  open,
  mode,
  record,
  storages,
  onClose,
  onSuccess,
  onUpdateBinding
}: FileTableDrawerProps) {
  const { message } = App.useApp();
  const [form] = Form.useForm<TableFormValues>();
  const [submitting, setSubmitting] = useState(false);
  const [bindingSubmitting, setBindingSubmitting] = useState(false);
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const isView = mode === 'view';

  useEffect(() => {
    if (open) {
      if (record && mode !== 'create') {
        form.setFieldsValue({
          code: record.code,
          title: record.title,
          bound_storage_id: record.bound_storage_id || ''
        });
      } else {
        form.resetFields();
      }
    }
  }, [open, record, mode, form]);

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      setSubmitting(true);

      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      await createSettingsFileTable(
        { code: values.code, title: values.title },
        csrfToken
      );

      message.success(i18nText('settings', 'auto.file_table_created'));
      onSuccess();
      onClose();
    } catch (err: unknown) {
      if (err && typeof err === 'object' && 'errorFields' in err) return;
      const msg =
        err instanceof Error
          ? err.message
          : i18nText('settings', 'auto.create_failed_retry');
      message.error(msg);
    } finally {
      setSubmitting(false);
    }
  };

  const handleBindingSave = async () => {
    if (!record) return;
    try {
      const values = form.getFieldsValue();
      setBindingSubmitting(true);
      await onUpdateBinding(record.id, values.bound_storage_id || '');
      message.success(i18nText('settings', 'auto.binding_updated'));
      onSuccess();
      onClose();
    } catch (err: unknown) {
      const msg =
        err instanceof Error
          ? err.message
          : i18nText('settings', 'auto.binding_update_failed_retry');
      message.error(msg);
    } finally {
      setBindingSubmitting(false);
    }
  };

  const storageOptions = storages.map((s) => ({
    label: `${s.title} (${s.code})`,
    value: s.id
  }));

  return (
    <Drawer
      title={
        mode === 'create'
          ? i18nText('settings', 'auto.add_file_table')
          : mode === 'edit'
            ? i18nText('settings', 'auto.edit_file_table')
            : i18nText('settings', 'auto.view_file_table')
      }
      open={open}
      onClose={onClose}
      size={480}
      extra={
        !isView ? (
          mode === 'edit' ? (
            <Button
              type="primary"
              loading={bindingSubmitting}
              onClick={handleBindingSave}
            >
              {i18nText('settings', 'auto.save_binding')}
            </Button>
          ) : (
            <Button type="primary" loading={submitting} onClick={handleSubmit}>
              {i18nText('settings', 'auto.create')}
            </Button>
          )
        ) : undefined
      }
    >
      <Form
        form={form}
        layout="vertical"
        disabled={isView}
        initialValues={{ bound_storage_id: '' }}
      >
        <Form.Item
          name="code"
          label={i18nText('settings', 'auto.table_code')}
          rules={[
            {
              required: true,
              message: i18nText('settings', 'auto.table_code_required')
            }
          ]}
        >
          <Input
            placeholder={i18nText('settings', 'auto.table_code_placeholder')}
            disabled={mode !== 'create'}
          />
        </Form.Item>

        <Form.Item
          name="title"
          label={i18nText('settings', 'auto.name')}
          rules={[
            {
              required: true,
              message: i18nText('settings', 'auto.name_required')
            }
          ]}
        >
          <Input
            placeholder={i18nText('settings', 'auto.table_name_placeholder')}
            disabled={isView}
          />
        </Form.Item>

        {mode !== 'create' && (
          <>
            <Form.Item
              name="bound_storage_id"
              label={i18nText('settings', 'auto.bound_storage')}
              rules={[
                {
                  required: true,
                  message: i18nText('settings', 'auto.storage_required')
                }
              ]}
            >
              <Select
                options={storageOptions}
                placeholder={i18nText('settings', 'auto.select_storage')}
                allowClear
                disabled={isView}
              />
            </Form.Item>

            <Form.Item label={i18nText('settings', 'auto.scope')}>
              <Input
                value={
                  record ? `${record.scope_kind} / ${record.scope_id}` : '-'
                }
                disabled
              />
            </Form.Item>

            {record && (
              <>
                <Form.Item label={i18nText('settings', 'auto.built_in_table')}>
                  <Input
                    value={
                      record.is_builtin
                        ? i18nText('settings', 'auto.yes')
                        : i18nText('settings', 'auto.no')
                    }
                    disabled
                  />
                </Form.Item>
                <Form.Item label={i18nText('settings', 'auto.status')}>
                  <Input value={record.status} disabled />
                </Form.Item>
              </>
            )}
          </>
        )}
      </Form>

      {mode === 'edit' && record && !isView && (
        <div style={{ marginTop: 16, color: '#888', fontSize: 13 }}>
          {i18nText('settings', 'auto.file_table_edit_binding_notice')}
        </div>
      )}
    </Drawer>
  );
}
