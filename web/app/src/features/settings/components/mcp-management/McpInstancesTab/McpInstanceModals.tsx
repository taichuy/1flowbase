import type {
  ConsoleMcpInstance,
  SaveConsoleMcpInstanceBody
} from '@1flowbase/api-client';
import { Button, Form, Input, Modal, Select, Tooltip, Typography } from 'antd';
import type { FormInstance } from 'antd';
import { ReloadOutlined } from '@ant-design/icons';

import type { CopySettingsMcpInstanceBody } from '../../../api/mcp-management';
import { i18nText } from '../../../../../shared/i18n/text';
import { buildRandomToolIdSeed } from '../mcp-management-view-model';

export function McpDiscardDirectoryChangesModal({
  open,
  onContinueEditing,
  onDiscard
}: {
  open: boolean;
  onContinueEditing: () => void;
  onDiscard: () => void;
}) {
  return (
    <Modal
      open={open}
      title={i18nText(
        'settingsMcpManagement',
        'auto.discard_unsaved_changes_title'
      )}
      okText={i18nText('settingsMcpManagement', 'auto.discard_unsaved_changes')}
      cancelText={i18nText('settingsMcpManagement', 'auto.continue_editing')}
      okButtonProps={{ danger: true }}
      onCancel={onContinueEditing}
      onOk={onDiscard}
    >
      <Typography.Text>
        {i18nText(
          'settingsMcpManagement',
          'auto.discard_unsaved_changes_description'
        )}
      </Typography.Text>
    </Modal>
  );
}

export function McpCopyInstanceModal({
  source,
  form,
  saving,
  onClose,
  onSave
}: {
  source: ConsoleMcpInstance | null;
  form: FormInstance<CopySettingsMcpInstanceBody>;
  saving: boolean;
  onClose: () => void;
  onSave: (
    source: ConsoleMcpInstance,
    values: CopySettingsMcpInstanceBody
  ) => void;
}) {
  return (
    <Modal
      open={Boolean(source)}
      title={i18nText('settingsMcpManagement', 'auto.copy_instance_title')}
      okText={i18nText('settingsMcpManagement', 'auto.copy_instance_action')}
      confirmLoading={saving}
      onCancel={onClose}
      onOk={() => form.submit()}
    >
      <Form
        form={form}
        layout="vertical"
        onFinish={(values) => source && onSave(source, values)}
      >
        <Form.Item
          name="instance_id"
          label="instance_id"
          rules={[{ required: true }]}
        >
          <Input />
        </Form.Item>
        <Form.Item
          name="name"
          label={i18nText('settings', 'auto.instance_name')}
          rules={[{ required: true }]}
        >
          <Input />
        </Form.Item>
      </Form>
    </Modal>
  );
}

export function McpInstanceEditorModal({
  open,
  instance,
  form,
  saving,
  onClose,
  onSave
}: {
  open: boolean;
  instance: ConsoleMcpInstance | null;
  form: FormInstance<SaveConsoleMcpInstanceBody>;
  saving: boolean;
  onClose: () => void;
  onSave: (values: SaveConsoleMcpInstanceBody) => void;
}) {
  return (
    <Modal
      open={open}
      title={
        instance
          ? i18nText('settings', 'auto.edit')
          : i18nText('settings', 'auto.new')
      }
      onCancel={onClose}
      onOk={() => form.submit()}
      confirmLoading={saving}
    >
      <Form form={form} layout="vertical" onFinish={onSave}>
        <Form.Item
          name="instance_id"
          label="instance_id"
          rules={[{ required: true }]}
        >
          <Input
            disabled={Boolean(instance)}
            addonAfter={
              instance ? undefined : (
                <Tooltip title="随机生成 instance_id">
                  <Button
                    type="text"
                    htmlType="button"
                    size="small"
                    icon={<ReloadOutlined />}
                    aria-label="随机生成 instance_id"
                    onClick={() =>
                      form.setFieldValue('instance_id', buildRandomToolIdSeed())
                    }
                  />
                </Tooltip>
              )
            }
          />
        </Form.Item>
        <Form.Item name="name" label="name" rules={[{ required: true }]}>
          <Input />
        </Form.Item>
        <Form.Item name="description_short" label="description_short">
          <Input />
        </Form.Item>
        <Form.Item name="status" label="status" rules={[{ required: true }]}>
          <Select
            options={['draft', 'enabled', 'disabled', 'archived'].map(
              (value) => ({ label: value, value })
            )}
          />
        </Form.Item>
        <Form.Item
          name="default_entry_path"
          label="default_entry_path"
          rules={[{ required: true }]}
        >
          <Input />
        </Form.Item>
      </Form>
    </Modal>
  );
}
