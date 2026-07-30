import { Button, Space, Typography, theme } from 'antd';
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
import { BuiltinPasswordSignIn } from '../components/BuiltinPasswordSignIn';
import { PublicAuthBlock } from '../components/PublicAuthBlock';

import './sign-in-page.css';

interface SignInPageProps {
  authenticatorId?: string;
}

export function SignInPage({ authenticatorId }: SignInPageProps) {
  const navigate = useNavigate();
  const { t } = useTranslation('auth');
  const { token } = theme.useToken();
  const setAuthenticated = useAuthStore((state) => state.setAuthenticated);
  const [loginInstances, setLoginInstances] = useState<PublicLoginInstance[]>(
    []
  );
  const [loading, setLoading] = useState(true);
  const [discoveryFailed, setDiscoveryFailed] = useState(false);
  const selectedLoginInstance = useMemo(
    () =>
      loginInstances.length === 1
        ? loginInstances[0]
        : (loginInstances.find((item) => item.id === authenticatorId) ?? null),
    [authenticatorId, loginInstances]
  );

  useEffect(() => {
    let active = true;
    fetchLoginInstances()
      .then((payload) => {
        if (!active) return;
        setLoginInstances(payload.login_instances);
        setDiscoveryFailed(false);
      })
      .catch(() => {
        if (!active) return;
        setLoginInstances([]);
        setDiscoveryFailed(true);
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, []);

  const requestAuthenticatorSelector = useCallback(() => {
    void navigate({
      to: '/sign-in',
      search: { authenticator_id: undefined },
      replace: true
    });
  }, [navigate]);

  const authenticatorSelector = useMemo(
    () =>
      loginInstances.length > 1
        ? { request: requestAuthenticatorSelector }
        : null,
    [loginInstances.length, requestAuthenticatorSelector]
  );

  useEffect(() => {
    if (loading || discoveryFailed || !authenticatorId) return;
    const pointsToEnabledAuthenticator =
      loginInstances.length > 1 &&
      loginInstances.some((instance) => instance.id === authenticatorId);
    if (!pointsToEnabledAuthenticator) requestAuthenticatorSelector();
  }, [
    authenticatorId,
    discoveryFailed,
    loading,
    loginInstances,
    requestAuthenticatorSelector
  ]);

  const handleAuthenticated = useCallback(
    async (session: {
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
    },
    [navigate, setAuthenticated]
  );

  return (
    <div style={{ display: 'flex', minHeight: '100vh', width: '100vw' }}>
      <HeroAnimation />
      <div
        className="auth-sign-in-panel"
        style={{
          display: 'flex',
          flexDirection: 'column',
          justifyContent: 'center',
          background: `linear-gradient(145deg, ${token.colorBgContainer} 60%, ${token.colorBgLayout} 100%)`,
          boxShadow: '-10px 0 32px rgba(0, 0, 0, 0.05)',
          borderLeft: `1px solid ${token.colorBorderSecondary}`,
          position: 'relative',
          zIndex: 10
        }}
      >
        <Space direction="vertical" size="large" style={{ width: '100%' }}>
          {loginInstances.length > 1 && !selectedLoginInstance ? (
            <Space direction="vertical" size={16} style={{ width: '100%' }}>
              {loginInstances.map((instance) => (
                <Button
                  key={instance.id}
                  block
                  className="auth-sign-in-selector-button"
                  shape="round"
                  size="large"
                  type="primary"
                  onClick={() =>
                    void navigate({
                      to: '/sign-in',
                      search: { authenticator_id: instance.id }
                    })
                  }
                >
                  {instance.title}
                </Button>
              ))}
            </Space>
          ) : null}
          {!loading && selectedLoginInstance ? (
            <PublicAuthBlock
              key={selectedLoginInstance.id}
              instance={selectedLoginInstance}
              authenticatorSelector={authenticatorSelector}
              onAuthenticated={handleAuthenticated}
            />
          ) : null}
          {!loading && (discoveryFailed || loginInstances.length === 0) ? (
            <BuiltinPasswordSignIn onAuthenticated={handleAuthenticated} />
          ) : null}
        </Space>

        <div style={{ textAlign: 'center', marginTop: 48 }}>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            <a
              href="https://1flowbase.taichuy.com/"
              target="_blank"
              rel="noreferrer"
              style={{ color: token.colorLink, textDecoration: 'none' }}
            >
              {t('sign_in.footer')}
            </a>
          </Typography.Text>
        </div>
      </div>
    </div>
  );
}
