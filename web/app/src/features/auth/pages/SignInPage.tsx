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
import { AUTH_ERROR_ICON_PATH } from '../components/anonymous-loading/AnonymousLoadingView';
import { useAuthStore } from '../../../state/auth-store';
import { startEarlyConsoleStartupFlight } from '../api/early-console-startup-flight';
import {
  fetchCurrentMe,
  fetchCurrentSession,
  type PublicLoginEntry,
  type PublicOnboardingMode
} from '../api/session';
const BuiltinPasswordSignIn = lazy(() =>
  import('../components/BuiltinPasswordSignIn').then((module) => ({
    default: module.BuiltinPasswordSignIn
  }))
);

import { consumeLoginEntryDiscovery } from '../api/login-entry-discovery';
import { SignInPanel } from '../components/hero/SignInPanel';
import { HeroAnimation } from '../components/HeroAnimation';

import { DefaultPasswordSignIn } from '../components/default-password/DefaultPasswordSignIn';
import { matchesDefaultPasswordSource } from '../components/default-password/source-identity';
import {
  pauseAuthenticatedPrewarm,
  useAuthenticatedPrewarm
} from '../lib/authenticated-prewarm';

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
  inviteToken?: string;
}

export function SignInPage({ loginEntryId, inviteToken }: SignInPageProps) {
  const { data: metadata } = usePublicSystemMetadata();
  const navigate = navigatePublicRoute;
  const location = usePublicLocation();
  const { t } = useAuthTranslation();
  const locationUrl = new URL(location, window.location.origin);
  const externalError = locationUrl.searchParams.get('auth_error');
  const urlInviteToken =
    inviteToken?.trim() ||
    locationUrl.searchParams.get('invite')?.trim() ||
    undefined;
  const invInvalid = t('external_auth.invitation_invalid');
  const externalErrorMessages: Record<string, string> = externalError
    ? {
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
        user_disabled: t('external_auth.user_disabled'),
        invitation_required: t('external_auth.invitation_required'),
        invitation_invalid: invInvalid,
        invitation_expired: invInvalid,
        invitation_exhausted: invInvalid,
        invitation_revoked: invInvalid,
        invitation_email_mismatch: invInvalid
      }
    : {};
  const setAuthenticated = useAuthStore((state) => state.setAuthenticated);
  const [loginEntries, setLoginEntries] = useState<PublicLoginEntry[]>([]);
  const [onboardingMode, setOnboardingMode] = useState<
    PublicOnboardingMode | undefined
  >(undefined);
  const [loading, setLoading] = useState(true);
  const [discoveryFailed, setDiscoveryFailed] = useState(false);
  const [startingEntryId, setStartingEntryId] = useState<string | null>(null);
  const [startFailed, setStartFailed] = useState(false);
  const startingRef = useRef(false);
  const isStarting = startingEntryId !== null;
  const effectiveInviteToken = urlInviteToken || undefined;

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
          if (effectiveInviteToken) {
            redirectToExternalLogin(entry.id, undefined, effectiveInviteToken);
          } else {
            redirectToExternalLogin(entry.id);
          }
        } catch {
          startingRef.current = false;
          setStartingEntryId(null);
          setStartFailed(true);
        }
      } else {
        void navigate({ to: '/sign-in', search: { login_entry_id: entry.id } });
      }
    },
    [effectiveInviteToken, navigate]
  );
  const selectedLoginEntry = useMemo(
    () =>
      loginEntries.length === 1
        ? loginEntries[0]
        : (loginEntries.find((item) => item.id === loginEntryId) ?? null),
    [loginEntryId, loginEntries]
  );
  const hasInviteToken = Boolean(effectiveInviteToken);
  const effectiveLoginEntry = useMemo(() => {
    if (!selectedLoginEntry) return null;
    if (
      (hasInviteToken ||
        onboardingMode === 'personal_workspace' ||
        onboardingMode === 'default_workspace') &&
      selectedLoginEntry.auth_type === 'password-local' &&
      selectedLoginEntry.public_variables.self_registration_enabled !== true
    ) {
      return {
        ...selectedLoginEntry,
        public_variables: {
          ...selectedLoginEntry.public_variables,
          self_registration_enabled: true
        }
      };
    }
    return selectedLoginEntry;
  }, [hasInviteToken, onboardingMode, selectedLoginEntry]);
  const isDefaultPassword = useMemo(
    () =>
      Boolean(
        effectiveLoginEntry &&
        matchesDefaultPasswordSource(effectiveLoginEntry.public_ui_block)
      ),
    [effectiveLoginEntry]
  );

  const formHost = useRef<HTMLDivElement>(null);
  useAuthenticatedPrewarm(formHost, effectiveLoginEntry);
  // Each selected entry owns its own authentication callbacks.
  const authOwner = useMemo(
    () => ({ active: false, selectedLoginEntry }),
    [selectedLoginEntry]
  );
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
        setOnboardingMode(payload.onboarding_mode);
        setDiscoveryFailed(false);
      })
      .catch(() => {
        if (!active) return;
        setLoginEntries([]);
        setOnboardingMode(undefined);
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
      pauseAuthenticatedPrewarm();
      const [me, currentSession] = await Promise.all([
        fetchCurrentMe(),
        fetchCurrentSession()
      ]);
      if (!authOwner.active) return;
      const snapshot = {
        csrfToken: currentSession.csrf_token,
        actor: currentSession.actor,
        me,
        availableRoles: currentSession.available_roles
      };
      startEarlyConsoleStartupFlight(snapshot, '/');
      setAuthenticated(snapshot);
      await navigate({ to: '/' });
    },
    [authOwner, navigate, setAuthenticated]
  );

  useEffect(() => {
    const nextTitle =
      metadata?.page_title?.trim() || metadata?.system_name?.trim();
    if (nextTitle && document.title !== nextTitle) {
      document.title = nextTitle;
    }
  }, [metadata?.page_title, metadata?.system_name]);

  return (
    <div className="auth-sign-in-page">
      <HeroAnimation
        hide_login_tagline={metadata?.hide_login_tagline !== false}
        login_hero_title={metadata?.login_hero_title}
        login_hero_tagline={metadata?.login_hero_tagline}
      />
      <div ref={formHost} style={{ display: 'contents' }}>
        <SignInPanel
          inert={isStarting}
          footer={t('sign_in.footer')}
          hideFooter={
            metadata === undefined || Boolean(metadata.hide_powered_by)
          }
          filing_html={metadata?.filing_html}
        >
          {urlInviteToken ? (
            <div
              className="auth-sign-in-invite-banner"
              data-testid="sign-in-invitation-banner"
              role="status"
            >
              <span>{t('sign_in.invitation_detected')}</span>
            </div>
          ) : null}
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
          {!loading &&
          effectiveLoginEntry?.entry_action === 'external_login' ? (
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
                aria-busy={startingEntryId === effectiveLoginEntry.id}
                onClick={() => activateLoginEntry(effectiveLoginEntry)}
              >
                {effectiveLoginEntry.title}
              </button>
            </div>
          ) : null}
          {!loading && loginEntries.length > 1 && !effectiveLoginEntry ? (
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
          effectiveLoginEntry?.entry_action === 'render_ui' &&
          isDefaultPassword ? (
            <DefaultPasswordSignIn
              key={effectiveLoginEntry.id}
              instance={effectiveLoginEntry}
              loginEntrySelector={loginEntrySelector}
              onAuthenticated={handleAuthenticated}
              invitationToken={effectiveInviteToken}
            />
          ) : null}
          {!loading &&
          effectiveLoginEntry?.entry_action === 'render_ui' &&
          !isDefaultPassword ? (
            <Suspense fallback={<LoadingState compact />}>
              <CustomAuthRuntime
                key={effectiveLoginEntry.id}
                instance={effectiveLoginEntry}
                loginEntrySelector={loginEntrySelector}
                onAuthenticated={handleAuthenticated}
                invitationToken={effectiveInviteToken}
              />
            </Suspense>
          ) : null}
          {!loading && (discoveryFailed || loginEntries.length === 0) ? (
            <Suspense fallback={<LoadingState compact />}>
              <BuiltinPasswordSignIn
                onAuthenticated={handleAuthenticated}
                invitationToken={effectiveInviteToken}
              />
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
        <path d={AUTH_ERROR_ICON_PATH} />
      </svg>
      <span>{children}</span>
    </div>
  );
}
