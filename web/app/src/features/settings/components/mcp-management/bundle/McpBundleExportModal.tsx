import { Form, Input, Modal, Select, Typography } from 'antd';
import { useEffect } from 'react';

import { i18nText } from '../../../../../shared/i18n/text';
import type { ExportSettingsMcpBundleBody } from '../../../api/mcp-management';

export function McpBundleExportModal({
  open,
  title,
  okText,
  defaultBundleId,
  exporting,
  onCancel,
  onExport
}: {
  open: boolean;
  title: string;
  okText: string;
  defaultBundleId: string;
  exporting: boolean;
  onCancel: () => void;
  onExport: (values: ExportSettingsMcpBundleBody) => void | Promise<void>;
}) {
  const [form] = Form.useForm<ExportSettingsMcpBundleBody>();

  useEffect(() => {
    if (!open) return;
    form.setFieldsValue({
      organization: 'taichuy',
      bundle_id: defaultBundleId,
      bundle_version: '1.0.0',
      locale: 'zh_Hans',
      minimum_host_version: '0.2.6'
    });
  }, [defaultBundleId, form, open]);

  return (
    <Modal
      open={open}
      title={title}
      okText={okText}
      confirmLoading={exporting}
      onCancel={onCancel}
      onOk={() => form.submit()}
    >
      <Form<ExportSettingsMcpBundleBody>
        form={form}
        layout="vertical"
        onFinish={(values) => void onExport(values)}
      >
        <Form.Item
          name="organization"
          label="organization"
          rules={[{ required: true }]}
        >
          <Input />
        </Form.Item>
        <Form.Item
          name="bundle_id"
          label="bundle_id"
          rules={[{ required: true }]}
        >
          <Input />
        </Form.Item>
        <Form.Item
          name="bundle_version"
          label="bundle_version"
          rules={[{ required: true }]}
        >
          <Input />
        </Form.Item>
        <Form.Item name="locale" label="locale" rules={[{ required: true }]}>
          <Select options={[{ value: 'zh_Hans' }, { value: 'en_US' }]} />
        </Form.Item>
        <Form.Item
          name="minimum_host_version"
          label="minimum_host_version"
          rules={[{ required: true }]}
        >
          <Input />
        </Form.Item>
        <Typography.Text type="secondary">
          {i18nText(
            'settingsMcpManagement',
            'auto.mcp_bundle_system_version_recorded'
          )}
        </Typography.Text>
      </Form>
    </Modal>
  );
}
