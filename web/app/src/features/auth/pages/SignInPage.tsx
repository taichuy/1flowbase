import { Alert, Button, Space, Typography, theme } from 'antd';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { useNavigate } from '@tanstack/react-router';

import { useAuthStore } from '../../../state/auth-store';
import {
  fetchCurrentMe,
  fetchLoginInstances,
  type PublicLoginInstance
} from '../api/session';
import { HeroAnimation } from '../components/HeroAnimation';
import { PublicAuthBlock } from '../components/PublicAuthBlock';

import './sign-in-page.css';

export function SignInPage() {
  const navigate = useNavigate();
  const { t } = useTranslation('auth');
  const { token } = theme.useToken();
  const setAuthenticated = useAuthStore((state) => state.setAuthenticated);
  const [loginInstances, setLoginInstances] = useState<PublicLoginInstance[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [selectedAuthenticatorId, setSelectedAuthenticatorId] = useState<string | null>(null);
  const selectedLoginInstance = useMemo(
    () => loginInstances.length === 1
      ? loginInstances[0]
      : loginInstances.find((item) => item.id === selectedAuthenticatorId) ?? null,
    [loginInstances, selectedAuthenticatorId]
  );

  useEffect(() => {
    let active = true;
    fetchLoginInstances()
      .then((payload) => {
        if (!active) return;
        setLoginInstances(payload.login_instances);
        setSelectedAuthenticatorId(null);
        setLoadError(
          payload.login_instances.length === 0 ? t('sign_in.no_login_instances') : null
        );
      })
      .catch(() => {
        if (!active) return;
        setLoginInstances([]);
        setSelectedAuthenticatorId(null);
        setLoadError(t('sign_in.login_instances_load_failed'));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => { active = false; };
  }, [t]);

  const handleAuthenticated = useCallback(async (session: {
    csrf_token: string;
    effective_display_role: string;
    current_workspace_id: string;
  }) => {
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
  }, [navigate, setAuthenticated]);

  return (
    <div style={{ display: 'flex', minHeight: '100vh', width: '100vw' }}>
      <HeroAnimation />
      <div
        className="auth-sign-in-panel"
        style={{
          display: 'flex', flexDirection: 'column', justifyContent: 'center',
          background: `linear-gradient(145deg, ${token.colorBgContainer} 60%, ${token.colorBgLayout} 100%)`,
          boxShadow: '-10px 0 32px rgba(0, 0, 0, 0.05)',
          borderLeft: `1px solid ${token.colorBorderSecondary}`,
          position: 'relative', zIndex: 10
        }}
      >
        <Space direction="vertical" size="large" style={{ width: '100%' }}>
          {loadError ? <Alert type="error" message={loadError} showIcon /> : null}
          {loginInstances.length > 1 && !selectedLoginInstance ? (
            <Space direction="vertical" size="small" style={{ width: '100%' }}>
              {loginInstances.map((instance) => (
                <Button
                  key={instance.id}
                  block
                  onClick={() => setSelectedAuthenticatorId(instance.id)}
                >
                  {instance.title}
                </Button>
              ))}
            </Space>
          ) : null}
          {!loading && selectedLoginInstance ? (
            <Space direction="vertical" size="middle" style={{ width: '100%' }}>
              {loginInstances.length > 1 ? (
                <Button
                  type="link"
                  style={{ alignSelf: 'flex-start', paddingInline: 0 }}
                  onClick={() => setSelectedAuthenticatorId(null)}
                >
                  {t('sign_in.back_to_login_options')}
                </Button>
              ) : null}
              <PublicAuthBlock
                key={selectedLoginInstance.id}
                instance={selectedLoginInstance}
                onAuthenticated={handleAuthenticated}
              />
            </Space>
          ) : null}
        </Space>

        <div style={{ textAlign: 'center', marginTop: 48 }}>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            <a href="https://1flowbase.taichuy.com/" target="_blank" rel="noreferrer"
              style={{ color: token.colorLink, textDecoration: 'none' }}>
              {t('sign_in.footer')}
            </a>
          </Typography.Text>
        </div>
      </div>
    </div>
  );
}
