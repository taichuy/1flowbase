import { Form, Modal, Select, type FormInstance, type ModalProps } from 'antd';
import type { ReactNode } from 'react';
import { i18nText } from '../../../../shared/i18n/text';

// Hosts own values and persistence; both surfaces use the same settings form.
export function AssistantSettingsModal({
  form,
  applications,
  applicationFixed = false,
  mcpInstances,
  mcpLoading,
  onApplicationChange,
  children,
  ...modal
}: Pick<
  ModalProps,
  'open' | 'onCancel' | 'onOk' | 'zIndex' | 'confirmLoading' | 'okButtonProps'
> & {
  form: FormInstance;
  applications: Array<{ application_id: string; name: string }>;
  applicationFixed?: boolean;
  mcpInstances: Array<{ value: string; label: string }>;
  mcpLoading?: boolean;
  onApplicationChange?: (applicationId: string) => void;
  children?: ReactNode;
}) {
  return (
    <Modal {...modal} title={i18nText('appShell', 'auto.assistant_settings')}>
      <Form form={form} layout="vertical">
        <Form.Item
          label={i18nText('appShell', 'auto.assistant_flow')}
          name="application_id"
          rules={[{ required: true }]}
        >
          <Select
            allowClear={!applicationFixed}
            disabled={applicationFixed}
            options={applications.map((application) => ({
              value: application.application_id,
              label: application.name
            }))}
            onChange={onApplicationChange}
          />
        </Form.Item>
        <Form.Item
          label={i18nText('appShell', 'auto.assistant_mcp')}
          name="mcp_instance_ids"
        >
          <Select
            mode="multiple"
            loading={mcpLoading}
            disabled={mcpLoading}
            options={mcpInstances}
          />
        </Form.Item>
        {children}
      </Form>
    </Modal>
  );
}
