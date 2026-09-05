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

import { LoadingState } from '../../../shared/ui/loading-state/LoadingState';
import { useAuthStore } from '../../../state/auth-store';
import {
  fetchCurrentMe,
  fetchCurrentSession,
  fetchLoginEntries,
  type PublicLoginEntry
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
  loginEntryId?: string;
}

export function SignInPage({ loginEntryId }: SignInPageProps) {
  const navigate = useNavigate();
  const { t } = useTranslation('auth');
  const setAuthenticated = useAuthStore((state) => state.setAuthenticated);
  const [loginEntries, setLoginEntries] = useState<PublicLoginEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [discoveryFailed, setDiscoveryFailed] = useState(false);
  const selectedLoginEntry = useMemo(
    () =>
      loginEntries.length === 1
        ? loginEntries[0]
        : (loginEntries.find((item) => item.id === loginEntryId) ?? null),
    [loginEntryId, loginEntries]
  );
  const selectedInstanceUsesLegacyContract = Boolean(
    selectedLoginEntry &&
    diagnoseLegacyBlockModuleSource(selectedLoginEntry.public_ui_block)
  );

  useEffect(() => {
    let active = true;
    fetchLoginEntries()
      .then((payload) => {
        if (!active) return;
        setLoginEntries(payload.login_entries);
        setDiscoveryFailed(false);
      })
      .catch(() => {
        if (!active) return;
        setLoginEntries([]);
        setDiscoveryFailed(true);
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, []);

  const requestLoginEntrySelector = useCallback(() => {
    void navigate({
      to: '/sign-in',
      search: { login_entry_id: undefined },
      replace: true
    });
  }, [navigate]);

  const loginEntrySelector = useMemo(
    () =>
      loginEntries.length > 1 ? { request: requestLoginEntrySelector } : null,
    [loginEntries.length, requestLoginEntrySelector]
  );

  useEffect(() => {
    if (loading || discoveryFailed || !loginEntryId) return;
    const pointsToEnabledLoginEntry =
      loginEntries.length > 1 &&
      loginEntries.some((instance) => instance.id === loginEntryId);
    if (!pointsToEnabledLoginEntry) requestLoginEntrySelector();
  }, [
    loginEntryId,
    discoveryFailed,
    loading,
    loginEntries,
    requestLoginEntrySelector
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
          {loading ? <LoadingState compact /> : null}
          {loginEntries.length > 1 && !selectedLoginEntry ? (
            <div className="auth-sign-in-selector">
              {loginEntries.map((instance) => (
                <button
                  key={instance.id}
                  className="auth-sign-in-selector-button"
                  type="button"
                  onClick={() =>
                    void navigate({
                      to: '/sign-in',
                      search: { login_entry_id: instance.id }
                    })
                  }
                >
                  {instance.title}
                </button>
              ))}
            </div>
          ) : null}
          {!loading &&
          selectedLoginEntry &&
          selectedInstanceUsesLegacyContract ? (
            <BuiltinPasswordSignIn
              loginEntryId={selectedLoginEntry.id}
              loginEntrySelector={loginEntrySelector}
              onAuthenticated={handleAuthenticated}
            />
          ) : null}
          {!loading &&
          selectedLoginEntry &&
          !selectedInstanceUsesLegacyContract ? (
            <Suspense fallback={<LoadingState compact />}>
              <PublicAuthBlock
                key={selectedLoginEntry.id}
                instance={selectedLoginEntry}
                loginEntrySelector={loginEntrySelector}
                onAuthenticated={handleAuthenticated}
              />
            </Suspense>
          ) : null}
          {!loading && (discoveryFailed || loginEntries.length === 0) ? (
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
