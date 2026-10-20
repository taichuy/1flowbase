import { Alert, Button, Form, Input, Space, Typography } from 'antd';

import type { ChangeMyPasswordInput } from '../api/me';
import { useTranslation } from 'react-i18next';

interface ChangePasswordValues {
  old_password: string;
  new_password: string;
  confirm_password: string;
}

export function ChangePasswordForm({
  className,
  submitting,
  disabled = false,
  errorMessage,
  onSubmit
}: {
  className?: string;
  submitting: boolean;
  disabled?: boolean;
  errorMessage: string | null;
  onSubmit: (input: ChangeMyPasswordInput) => Promise<void> | void;
}) {
  const { t } = useTranslation('me');
  const [form] = Form.useForm<ChangePasswordValues>();

  return (
    <Space className={className} orientation="vertical" size="large">
      <div>
        <Typography.Paragraph>
          {t('auto.password_update_notice')}
        </Typography.Paragraph>
      </div>

      {errorMessage ? (
        <Alert type="error" title={errorMessage} showIcon />
      ) : null}

      <Form<ChangePasswordValues>
        form={form}
        disabled={disabled}
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
          label={t('auto.old_password')}
          name="old_password"
          rules={[
            { required: true, message: t('auto.current_password_required') }
          ]}
        >
          <Input.Password autoComplete="current-password" />
        </Form.Item>
        <Form.Item
          label={t('auto.new_password')}
          name="new_password"
          rules={[{ required: true, message: t('auto.new_password_required') }]}
        >
          <Input.Password autoComplete="new-password" />
        </Form.Item>
        <Form.Item
          label={t('auto.confirm_new_password')}
          name="confirm_password"
          dependencies={['new_password']}
          rules={[
            { required: true, message: t('auto.verify_new_password_required') },
            ({ getFieldValue }) => ({
              validator(_, value) {
                if (!value || value === getFieldValue('new_password')) {
                  return Promise.resolve();
                }

                return Promise.reject(new Error(t('auto.password_mismatch')));
              }
            })
          ]}
        >
          <Input.Password autoComplete="new-password" />
        </Form.Item>
        <Button type="primary" htmlType="submit" loading={submitting}>
          {t('auto.update_password')}
        </Button>
      </Form>
    </Space>
  );
}
