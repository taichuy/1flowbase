import { ArrowLeftOutlined } from '@ant-design/icons';
import { Alert, Button, Input, Space, Typography } from 'antd';
import { useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import {
  signInWithPassword,
  type PasswordSignInResponse
} from '../api/session';

interface BuiltinPasswordSignInProps {
  authenticatorSelector?: { request: () => void } | null;
  onAuthenticated: (session: PasswordSignInResponse) => void | Promise<void>;
}

export function BuiltinPasswordSignIn({
  authenticatorSelector = null,
  onAuthenticated
}: BuiltinPasswordSignInProps) {
  const { t } = useTranslation('auth');
  const [identifier, setIdentifier] = useState('');
  const [password, setPassword] = useState('');
  const [pending, setPending] = useState(false);
  const [failed, setFailed] = useState(false);

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setPending(true);
    setFailed(false);
    try {
      const session = await signInWithPassword({
        identifier,
        password
      });
      await onAuthenticated(session);
    } catch {
      setFailed(true);
    } finally {
      setPending(false);
    }
  };

  return (
    <Space
      data-testid="builtin-password-sign-in"
      direction="vertical"
      size="middle"
      style={{ width: '100%' }}
    >
      {authenticatorSelector ? (
        <Button
          aria-label={t('sign_in.back_to_login_options')}
          icon={<ArrowLeftOutlined aria-hidden="true" />}
          onClick={authenticatorSelector.request}
          style={{ alignSelf: 'flex-start' }}
          type="text"
        />
      ) : null}
      <Typography.Title level={2} style={{ margin: 0, textAlign: 'center' }}>
        {t('sign_in.fallback_title')}
      </Typography.Title>
      {failed ? (
        <Alert
          type="error"
          showIcon
          message={t('sign_in.fallback_authentication_failed')}
        />
      ) : null}
      <form onSubmit={submit} style={{ display: 'grid', gap: 12 }}>
        <label style={{ display: 'grid', gap: 6 }}>
          {t('sign_in.fallback_identifier_label')}
          <Input
            required
            autoComplete="username"
            value={identifier}
            onChange={(event) => setIdentifier(event.target.value)}
          />
        </label>
        <label style={{ display: 'grid', gap: 6 }}>
          {t('sign_in.fallback_password_label')}
          <Input.Password
            required
            autoComplete="current-password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
          />
        </label>
        <Button htmlType="submit" type="primary" block loading={pending}>
          {t('sign_in.fallback_submit')}
        </Button>
      </form>
    </Space>
  );
}
