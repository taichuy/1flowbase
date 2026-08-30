import {
  Suspense,
  lazy,
  useCallback,
  useEffect,
  useMemo,
  useState
} from 'react';
import { useTranslation } from 'react-i18next';

import { useNavigate } from '@tanstack/react-router';
import { diagnoseLegacyBlockModuleSource } from '@1flowbase/page-runtime/source-contract';

import { useAuthStore } from '../../../state/auth-store';
import {
  fetchCurrentMe,
  fetchCurrentSession,
  fetchLoginInstances,
  type PublicLoginInstance
} from '../api/session';
import { BuiltinPasswordSignIn } from '../components/BuiltinPasswordSignIn';

const HeroAnimation = lazy(() =>
  import('../components/HeroAnimation').then((module) => ({
    default: module.HeroAnimation
  }))
);

const PublicAuthBlock = lazy(() =>
  import('../components/PublicAuthBlock').then((module) => ({
    default: module.PublicAuthBlock
  }))
);

import './sign-in-page.css';

interface SignInPageProps {
  authenticatorId?: string;
}

export function SignInPage({ authenticatorId }: SignInPageProps) {
  const navigate = useNavigate();
  const { t } = useTranslation('auth');
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
  const selectedInstanceUsesLegacyContract = Boolean(
    selectedLoginInstance &&
    diagnoseLegacyBlockModuleSource(selectedLoginInstance.public_ui_block)
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
    async (_session: {
      csrf_token: string;
      effective_display_role: string;
      current_workspace_id: string;
    }) => {
      const [me, currentSession] = await Promise.all([
        fetchCurrentMe(),
        fetchCurrentSession()
      ]);
      setAuthenticated({
        csrfToken: currentSession.csrf_token,
        actor: currentSession.actor,
        me,
        availableRoles: currentSession.available_roles
      });
      await navigate({ to: '/' });
    },
    [navigate, setAuthenticated]
  );

  return (
    <div className="auth-sign-in-page">
      <Suspense fallback={null}>
        <HeroAnimation />
      </Suspense>
      <div
        className="auth-sign-in-panel"
        style={{
          display: 'flex',
          flexDirection: 'column',
          justifyContent: 'center',
          position: 'relative',
          zIndex: 10
        }}
      >
        <div className="auth-sign-in-content">
          {loginInstances.length > 1 && !selectedLoginInstance ? (
            <div className="auth-sign-in-selector">
              {loginInstances.map((instance) => (
                <button
                  key={instance.id}
                  className="auth-sign-in-selector-button"
                  type="button"
                  onClick={() =>
                    void navigate({
                      to: '/sign-in',
                      search: { authenticator_id: instance.id }
                    })
                  }
                >
                  {instance.title}
                </button>
              ))}
            </div>
          ) : null}
          {!loading &&
          selectedLoginInstance &&
          selectedInstanceUsesLegacyContract ? (
            <BuiltinPasswordSignIn
              authenticatorSelector={authenticatorSelector}
              onAuthenticated={handleAuthenticated}
            />
          ) : null}
          {!loading &&
          selectedLoginInstance &&
          !selectedInstanceUsesLegacyContract ? (
            <Suspense fallback={null}>
              <PublicAuthBlock
                key={selectedLoginInstance.id}
                instance={selectedLoginInstance}
                authenticatorSelector={authenticatorSelector}
                onAuthenticated={handleAuthenticated}
              />
            </Suspense>
          ) : null}
          {!loading && (discoveryFailed || loginInstances.length === 0) ? (
            <BuiltinPasswordSignIn onAuthenticated={handleAuthenticated} />
          ) : null}
        </div>

        <div style={{ textAlign: 'center', marginTop: 48 }}>
          <span className="auth-sign-in-footer">
            <a
              href="https://1flowbase.taichuy.com/"
              target="_blank"
              rel="noreferrer"
            >
              {t('sign_in.footer')}
            </a>
          </span>
        </div>
      </div>
    </div>
  );
}
