import {
  Suspense,
  lazy,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState
} from 'react';
import { useAuthTranslation } from '../lib/auth-i18n';

import {
  navigatePublicRoute,
  usePublicLocation
} from '../lib/anonymous-navigation';

import { AnonymousLoadingState as LoadingState } from '../components/anonymous-loading/AnonymousLoadingState';
import { useAuthStore } from '../../../state/auth-store';
import {
  fetchCurrentMe,
  fetchCurrentSession,
  type PublicLoginEntry
} from '../api/session';
const BuiltinPasswordSignIn = lazy(() =>
  import('../components/BuiltinPasswordSignIn').then((module) => ({
    default: module.BuiltinPasswordSignIn
  }))
);

import { consumeLoginEntryDiscovery } from '../api/login-entry-discovery';
import { SignInPanel } from '../components/hero/SignInPanel';
import { HeroAnimation } from '../components/HeroAnimation';

import {
  DefaultPasswordSignIn,
  matchesDefaultPasswordSource
} from '../components/default-password/DefaultPasswordSignIn';
import { useAuthenticatedPrewarm } from '../lib/authenticated-prewarm';

const CustomAuthRuntime = lazy(() =>
  import('../components/public-runtime/CustomAuthRuntime').then((module) => ({
    default: module.CustomAuthRuntime
  }))
);

import { prefetchPublicAuthEntry } from '../lib/public-auth-loading';
import { redirectToExternalLogin } from '../lib/external-login';
import './sign-in-page.css';
import { usePublicSystemMetadata } from '../lib/public-system-metadata';

interface SignInPageProps {
  loginEntryId?: string;
}

export function SignInPage({ loginEntryId }: SignInPageProps) {
  const { data: metadata } = usePublicSystemMetadata();
  const navigate = navigatePublicRoute;
  const location = usePublicLocation();
  const { t } = useAuthTranslation();
  const externalError = new URL(
    location,
    window.location.origin
  ).searchParams.get('auth_error');
  const externalErrorMessages: Record<string, string> = {
    registration_disabled: t('external_auth.registration_disabled'),
    account_too_new: t('external_auth.account_too_new'),
    email_required: t('external_auth.email_required'),
    identity_exists: t('external_auth.identity_exists'),
    identity_unlinked: t('external_auth.identity_unlinked'),
    registration_ineligible: t('external_auth.registration_ineligible'),
    cancelled: t('external_auth.cancelled'),
    invalid_state: t('external_auth.invalid_state'),
    entry_unavailable: t('external_auth.entry_unavailable'),
    provider_unavailable: t('external_auth.provider_unavailable'),
    session_expired: t('external_auth.session_expired'),
    user_disabled: t('external_auth.user_disabled')
  };
  const setAuthenticated = useAuthStore((state) => state.setAuthenticated);
  const [loginEntries, setLoginEntries] = useState<PublicLoginEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [discoveryFailed, setDiscoveryFailed] = useState(false);
  const [startingEntryId, setStartingEntryId] = useState<string | null>(null);
  const [startFailed, setStartFailed] = useState(false);
  const startingRef = useRef(false);
  const isStarting = startingEntryId !== null;

  useEffect(() => {
    const reset = () => {
      startingRef.current = false;
      setStartingEntryId(null);
    };
    window.addEventListener('pageshow', reset);
    return () => window.removeEventListener('pageshow', reset);
  }, []);

  const activateLoginEntry = useCallback(
    (entry: PublicLoginEntry) => {
      if (startingRef.current) return;
      if (entry.entry_action === 'external_login') {
        startingRef.current = true;
        setStartingEntryId(entry.id);
        setStartFailed(false);
        try {
          redirectToExternalLogin(entry.id);
        } catch {
          startingRef.current = false;
          setStartingEntryId(null);
          setStartFailed(true);
        }
      } else {
        void navigate({ to: '/sign-in', search: { login_entry_id: entry.id } });
      }
    },
    [navigate]
  );
  const selectedLoginEntry = useMemo(
    () =>
      loginEntries.length === 1
        ? loginEntries[0]
        : (loginEntries.find((item) => item.id === loginEntryId) ?? null),
    [loginEntryId, loginEntries]
  );
  const isDefaultPassword = useMemo(
    () =>
      Boolean(
        selectedLoginEntry &&
        matchesDefaultPasswordSource(selectedLoginEntry.public_ui_block)
      ),
    [selectedLoginEntry]
  );

  const formHost = useRef<HTMLDivElement>(null);
  useAuthenticatedPrewarm(formHost, selectedLoginEntry);
  const authOwner = useMemo(() => ({ active: false }), [selectedLoginEntry]);
  useEffect(() => {
    authOwner.active = true;
    return () => {
      authOwner.active = false;
    };
  }, [authOwner]);

  useEffect(() => {
    let active = true;
    consumeLoginEntryDiscovery()
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
      if (!authOwner.active) return;
      const [me, currentSession] = await Promise.all([
        fetchCurrentMe(),
        fetchCurrentSession()
      ]);
      if (!authOwner.active) return;
      setAuthenticated({
        csrfToken: currentSession.csrf_token,
        actor: currentSession.actor,
        me,
        availableRoles: currentSession.available_roles
      });
      await navigate({ to: '/' });
    },
    [authOwner, navigate, setAuthenticated]
  );

  return (
    <div className="auth-sign-in-page">
      <HeroAnimation
        hide_login_tagline={metadata?.hide_login_tagline !== false}
      />
      <div ref={formHost} style={{ display: 'contents' }}>
        <SignInPanel
          inert={isStarting}
          footer={t('sign_in.footer')}
          filing_html={metadata?.filing_html}
        >
          {externalError ? (
            <AuthError>
              {externalErrorMessages[externalError] ??
                t('external_auth.failed')}
            </AuthError>
          ) : null}
          {startFailed ? (
            <AuthError>{t('external_auth.failed')}</AuthError>
          ) : null}
          {loading ? <LoadingState compact /> : null}
          {!loading && selectedLoginEntry?.entry_action === 'external_login' ? (
            <div className="auth-sign-in-selector">
              {loginEntrySelector ? (
                <button
                  className="auth-sign-in-selector-button"
                  type="button"
                  disabled={isStarting}
                  onClick={requestLoginEntrySelector}
                >
                  {t('external_auth.other_methods')}
                </button>
              ) : null}
              <button
                className="auth-sign-in-selector-button"
                type="button"
                disabled={isStarting}
                aria-busy={startingEntryId === selectedLoginEntry.id}
                onClick={() => activateLoginEntry(selectedLoginEntry)}
              >
                {selectedLoginEntry.title}
              </button>
            </div>
          ) : null}
          {!loading && loginEntries.length > 1 && !selectedLoginEntry ? (
            <div className="auth-sign-in-selector">
              {loginEntries.map((instance) => (
                <button
                  key={instance.id}
                  className="auth-sign-in-selector-button"
                  type="button"
                  disabled={isStarting}
                  aria-busy={startingEntryId === instance.id}
                  onPointerEnter={() => {
                    if (instance.entry_action === 'render_ui')
                      prefetchPublicAuthEntry(instance.public_ui_block);
                  }}
                  onFocus={() => {
                    if (instance.entry_action === 'render_ui')
                      prefetchPublicAuthEntry(instance.public_ui_block);
                  }}
                  onClick={() => activateLoginEntry(instance)}
                >
                  {instance.title}
                </button>
              ))}
            </div>
          ) : null}
          {!loading &&
          selectedLoginEntry?.entry_action === 'render_ui' &&
          isDefaultPassword ? (
            <DefaultPasswordSignIn
              key={selectedLoginEntry.id}
              instance={selectedLoginEntry}
              loginEntrySelector={loginEntrySelector}
              onAuthenticated={handleAuthenticated}
            />
          ) : null}
          {!loading &&
          selectedLoginEntry?.entry_action === 'render_ui' &&
          !isDefaultPassword ? (
            <Suspense fallback={<LoadingState compact />}>
              <CustomAuthRuntime
                key={selectedLoginEntry.id}
                instance={selectedLoginEntry}
                loginEntrySelector={loginEntrySelector}
                onAuthenticated={handleAuthenticated}
              />
            </Suspense>
          ) : null}
          {!loading && (discoveryFailed || loginEntries.length === 0) ? (
            <Suspense fallback={<LoadingState compact />}>
              <BuiltinPasswordSignIn onAuthenticated={handleAuthenticated} />
            </Suspense>
          ) : null}
        </SignInPanel>
      </div>
      {isStarting ? (
        <LoadingState fullscreen className="auth-sign-in-loading-overlay" />
      ) : null}
    </div>
  );
}

function AuthError({ children }: { children: string }) {
  return (
    <div className="auth-sign-in-error" role="alert">
      <svg
        aria-hidden="true"
        viewBox="64 64 896 896"
        fill="currentColor"
        fillRule="evenodd"
      >
        <path d="M512 64c247.4 0 448 200.6 448 448S759.4 960 512 960 64 759.4 64 512 264.6 64 512 64zm127.98 274.82h-.04l-.08.06L512 466.75 384.14 338.88c-.04-.05-.06-.06-.08-.06a.12.12 0 00-.07 0c-.03 0-.05.01-.09.05l-45.02 45.02a.2.2 0 00-.05.09.12.12 0 000 .07v.02a.27.27 0 00.06.06L466.75 512 338.88 639.86c-.05.04-.06.06-.06.08a.12.12 0 000 .07c0 .03.01.05.05.09l45.02 45.02a.2.2 0 00.09.05.12.12 0 00.07 0c.02 0 .04-.01.08-.05L512 557.25l127.86 127.87c.04.04.06.05.08.05a.12.12 0 00.07 0c.03 0 .05-.01.09-.05l45.02-45.02a.2.2 0 00.05-.09.12.12 0 000-.07v-.02a.27.27 0 00-.05-.06L557.25 512l127.87-127.86c.04-.04.05-.06.05-.08a.12.12 0 000-.07c0-.03-.01-.05-.05-.09l-45.02-45.02a.2.2 0 00-.09-.05.12.12 0 00-.07 0z" />
      </svg>
      <span>{children}</span>
    </div>
  );
}
