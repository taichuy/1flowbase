import { Alert, Button, Form, Input, Space, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import type {
  ChangeMyPasswordInput,
  ConsoleAuthenticationMethods
} from '../api/me';
import { ChangePasswordForm } from './ChangePasswordForm';

export function PasswordAuthenticationMethod({
  methods,
  disabled,
  changing,
  setting,
  authorizingEntryId,
  changeError,
  onChangePassword,
  onAuthorize,
  onSetPassword
}: {
  methods: ConsoleAuthenticationMethods;
  disabled: boolean;
  changing: boolean;
  setting: boolean;
  authorizingEntryId: string | undefined;
  changeError: string | null;
  onChangePassword: (input: ChangeMyPasswordInput) => Promise<void>;
  onAuthorize: (login_entry_id: string) => void;
  onSetPassword: (input: { new_password: string }) => void;
}) {
  const { t } = useTranslation('me');
  if (methods.password_configured) {
    return (
      <Space orientation="vertical" style={{ width: '100%' }}>
        <Typography.Paragraph>
          {t('authentication.password_configured')}
        </Typography.Paragraph>
        {!methods.password_usable ? (
          <Alert
            showIcon
            type="warning"
            title={t('authentication.password_unavailable')}
          />
        ) : null}
        <ChangePasswordForm
          className="me-security-panel"
          submitting={changing}
          errorMessage={changeError}
          onSubmit={onChangePassword}
        />
      </Space>
    );
  }
  return (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      <Typography.Paragraph>
        {t('authentication.password_not_configured')}
      </Typography.Paragraph>
      {methods.password_setup_verified ? (
        <>
          <Alert
            type="success"
            showIcon
            title={t('authentication.password_verified')}
          />
          <Typography.Paragraph>
            {t('auto.password_update_notice')}
          </Typography.Paragraph>
          <Form<{ new_password: string; confirm_password: string }>
            layout="vertical"
            onFinish={({ new_password }) => onSetPassword({ new_password })}
          >
            <Form.Item
              name="new_password"
              label={t('auto.new_password')}
              rules={[
                { required: true, message: t('auto.new_password_required') }
              ]}
            >
              <Input.Password autoComplete="new-password" />
            </Form.Item>
            <Form.Item
              name="confirm_password"
              label={t('auto.confirm_new_password')}
              dependencies={['new_password']}
              rules={[
                {
                  required: true,
                  message: t('auto.verify_new_password_required')
                },
                ({ getFieldValue }) => ({
                  validator: (_, value) =>
                    !value || value === getFieldValue('new_password')
                      ? Promise.resolve()
                      : Promise.reject(new Error(t('auto.password_mismatch')))
                })
              ]}
            >
              <Input.Password autoComplete="new-password" />
            </Form.Item>
            <Button
              type="primary"
              htmlType="submit"
              loading={setting}
              disabled={disabled}
            >
              {t('authentication.set_password')}
            </Button>
          </Form>
        </>
      ) : (
        <>
          <Alert
            showIcon
            type="info"
            title={t('authentication.verify_password_setup')}
          />
          {methods.password_verification_entries.length === 0 ? (
            <Typography.Paragraph type="secondary">
              {t('authentication.no_verification_entry')}
            </Typography.Paragraph>
          ) : (
            <Space wrap>
              {methods.password_verification_entries.map((entry) => (
                <Button
                  key={entry.id}
                  disabled={disabled}
                  loading={authorizingEntryId === entry.id}
                  onClick={() => onAuthorize(entry.id)}
                >
                  {t('authentication.verify_with', { entry: entry.title })}
                </Button>
              ))}
            </Space>
          )}
        </>
      )}
    </Space>
  );
}
