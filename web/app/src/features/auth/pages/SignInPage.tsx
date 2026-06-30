import {
  Alert,
  Button,
  Form,
  Input,
  Segmented,
  Space,
  Typography,
  theme
} from 'antd';
import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { useNavigate } from '@tanstack/react-router';

import { useAuthStore } from '../../../state/auth-store';
import {
  fetchCurrentMe,
  fetchLoginInstances,
  signInWithPassword,
  type PublicLoginInstance
} from '../api/session';
import { HeroAnimation } from '../components/HeroAnimation';

export function SignInPage() {
  const navigate = useNavigate();
  const { t } = useTranslation('auth');
  const { token } = theme.useToken();
  const setAuthenticated = useAuthStore((state) => state.setAuthenticated);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [loginInstances, setLoginInstances] = useState<PublicLoginInstance[]>(
    []
  );
  const [loginInstancesLoading, setLoginInstancesLoading] = useState(true);
  const [loginInstancesError, setLoginInstancesError] = useState<string | null>(
    null
  );
  const [selectedAuthenticatorId, setSelectedAuthenticatorId] = useState<
    string | null
  >(null);
  const [submitting, setSubmitting] = useState(false);
  const selectedLoginInstance = useMemo(
    () =>
      loginInstances.find(
        (instance) => instance.id === selectedAuthenticatorId
      ) ??
      loginInstances[0] ??
      null,
    [loginInstances, selectedAuthenticatorId]
  );
  const signInDisabled =
    loginInstancesLoading ||
    loginInstancesError != null ||
    loginInstances.length === 0;

  useEffect(() => {
    let active = true;

    setLoginInstancesLoading(true);
    setLoginInstancesError(null);
    fetchLoginInstances()
      .then((payload) => {
        if (!active) {
          return;
        }
        setLoginInstances(payload.login_instances);
        setSelectedAuthenticatorId(
          payload.default_authenticator_id ??
            payload.login_instances[0]?.id ??
            null
        );
        if (payload.login_instances.length === 0) {
          setLoginInstancesError(t('sign_in.no_login_instances'));
        }
      })
      .catch(() => {
        if (!active) {
          return;
        }
        setLoginInstances([]);
        setSelectedAuthenticatorId(null);
        setLoginInstancesError(t('sign_in.login_instances_load_failed'));
      })
      .finally(() => {
        if (active) {
          setLoginInstancesLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [t]);

  const handleFinish = async (values: {
    identifier: string;
    password: string;
  }) => {
    if (signInDisabled || !selectedLoginInstance) {
      setErrorMessage(
        loginInstancesError ?? t('sign_in.login_instances_load_failed')
      );
      return;
    }

    setSubmitting(true);
    setErrorMessage(null);

    try {
      const session = await signInWithPassword({
        ...values,
        authenticator_id: selectedLoginInstance.id
      });
      const me = await fetchCurrentMe();

      setAuthenticated({
        csrfToken: session.csrf_token,
        actor: {
          id: me.id,
          account: me.account,
          effective_display_role: session.effective_display_role,
          current_workspace_id: session.current_workspace_id
        },
        me
      });
      await navigate({ to: '/' });
    } catch (error) {
      setErrorMessage(
        error instanceof Error ? error.message : t('sign_in.error_fallback')
      );
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div style={{ display: 'flex', minHeight: '100vh', width: '100vw' }}>
      <HeroAnimation />
      <div
        style={{
          flex: '0 0 480px',
          display: 'flex',
          flexDirection: 'column',
          justifyContent: 'center',
          padding: '0 64px',
          background: `linear-gradient(145deg, ${token.colorBgContainer} 60%, ${token.colorBgLayout} 100%)`,
          boxShadow: '-10px 0 32px rgba(0, 0, 0, 0.05)',
          borderLeft: `1px solid ${token.colorBorderSecondary}`,
          position: 'relative',
          zIndex: 10
        }}
      >
        <Space direction="vertical" size="large" style={{ width: '100%' }}>
          <div style={{ textAlign: 'center', marginBottom: 16 }}>
            <Typography.Title level={2} style={{ margin: 0 }}>
              {t('sign_in.title')}
            </Typography.Title>
          </div>
          {errorMessage ? (
            <Alert type="error" message={errorMessage} showIcon />
          ) : null}
          {loginInstancesError ? (
            <Alert type="error" message={loginInstancesError} showIcon />
          ) : null}
          {loginInstances.length > 1 ? (
            <Segmented
              block
              value={selectedLoginInstance?.id}
              options={loginInstances.map((instance) => ({
                label: instance.title,
                value: instance.id
              }))}
              onChange={(value) => setSelectedAuthenticatorId(String(value))}
            />
          ) : null}
          <Form layout="vertical" onFinish={handleFinish} autoComplete="off">
            <Form.Item
              label={t('sign_in.identifier.label')}
              name="identifier"
              rules={[
                { required: true, message: t('sign_in.identifier.required') }
              ]}
            >
              <Input
                disabled={signInDisabled}
                placeholder={t('sign_in.identifier.placeholder')}
                size="large"
              />
            </Form.Item>
            <Form.Item
              label={t('sign_in.password.label')}
              name="password"
              rules={[
                { required: true, message: t('sign_in.password.required') }
              ]}
            >
              <Input.Password
                disabled={signInDisabled}
                placeholder={t('sign_in.password.placeholder')}
                size="large"
              />
            </Form.Item>
            <Button
              type="primary"
              htmlType="submit"
              loading={submitting || loginInstancesLoading}
              disabled={signInDisabled}
              block
              size="large"
            >
              {t('sign_in.submit')}
            </Button>
          </Form>
        </Space>

        <div style={{ textAlign: 'center', marginTop: 48 }}>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            <a
              href="https://www.taichuy.com"
              target="_blank"
              rel="noreferrer"
              style={{
                color: token.colorTextDescription,
                textDecoration: 'none'
              }}
            >
              {t('sign_in.footer')}
            </a>
          </Typography.Text>
        </div>
      </div>
    </div>
  );
}
