import { Alert, Button, Form, Input, Space, Typography } from 'antd';

import type { ChangeMyPasswordInput } from '../api/me';
import { i18nText } from '../../../shared/i18n/text';

interface ChangePasswordValues {
  old_password: string;
  new_password: string;
  confirm_password: string;
}

export function ChangePasswordForm({
  className,
  submitting,
  errorMessage,
  onSubmit
}: {
  className?: string;
  submitting: boolean;
  errorMessage: string | null;
  onSubmit: (input: ChangeMyPasswordInput) => Promise<void> | void;
}) {
  const [form] = Form.useForm<ChangePasswordValues>();

  return (
    <Space className={className} orientation="vertical" size="large">
      <div>
        <Typography.Title level={3}>{i18nText("me", "auto.security_settings")}</Typography.Title>
        <Typography.Paragraph>
          {i18nText("me", "auto.password_update_notice")}</Typography.Paragraph>
      </div>

      {errorMessage ? <Alert type="error" title={errorMessage} showIcon /> : null}

      <Form<ChangePasswordValues>
        form={form}
        layout="vertical"
        onFinish={async (values) => {
          await onSubmit({
            old_password: values.old_password,
            new_password: values.new_password
          });
          form.resetFields();
        }}
      >
        <Form.Item
          label={i18nText("me", "auto.password")}
          name="old_password"
          rules={[{ required: true, message: i18nText("me", "auto.current_password_required") }]}
        >
          <Input.Password />
        </Form.Item>
        <Form.Item
          label={i18nText("me", "auto.new_password")}
          name="new_password"
          rules={[{ required: true, message: i18nText("me", "auto.new_password_required") }]}
        >
          <Input.Password />
        </Form.Item>
        <Form.Item
          label={i18nText("me", "auto.confirm_new_password")}
          name="confirm_password"
          dependencies={['new_password']}
          rules={[
            { required: true, message: i18nText("me", "auto.verify_new_password_required") },
            ({ getFieldValue }) => ({
              validator(_, value) {
                if (!value || value === getFieldValue('new_password')) {
                  return Promise.resolve();
                }

                return Promise.reject(new Error(i18nText("me", "auto.password_mismatch")));
              }
            })
          ]}
        >
          <Input.Password />
        </Form.Item>
        <Button type="primary" htmlType="submit" loading={submitting}>
          {i18nText("me", "auto.update_password")}</Button>
      </Form>
    </Space>
  );
}
