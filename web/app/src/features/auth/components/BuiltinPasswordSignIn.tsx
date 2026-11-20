import './builtin-password.css';
import { useEffect, useRef, useState, type FormEvent } from 'react';
import { useAuthTranslation } from '../lib/auth-i18n';

import {
  signInWithPassword,
  type PasswordSignInResponse
} from '../api/session';

interface BuiltinPasswordSignInProps {
  loginEntryId?: string;
  loginEntrySelector?: { request: () => void } | null;
  onAuthenticated: (session: PasswordSignInResponse) => void | Promise<void>;
}

export function BuiltinPasswordSignIn({
  loginEntryId,
  loginEntrySelector = null,
  onAuthenticated
}: BuiltinPasswordSignInProps) {
  const { t } = useAuthTranslation();
  const [identifier, setIdentifier] = useState('');
  const [password, setPassword] = useState('');
  const [pending, setPending] = useState(false);
  const [failed, setFailed] = useState(false);
  const submitting = useRef(false);
  const generation = useRef(0);
  useEffect(() => {
    generation.current += 1;
    submitting.current = false;
    setPending(false);
    setFailed(false);
    return () => {
      generation.current += 1;
    };
  }, [loginEntryId]);

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (submitting.current) return;
    submitting.current = true;
    const attempt = generation.current;
    setPending(true);
    setFailed(false);
    try {
      const session = await signInWithPassword({
        login_entry_id: loginEntryId,
        identifier,
        password
      });
      if (attempt === generation.current) await onAuthenticated(session);
    } catch {
      if (attempt === generation.current) setFailed(true);
    } finally {
      if (attempt === generation.current) {
        submitting.current = false;
        setPending(false);
      }
    }
  };

  return (
    <div
      className="builtin-password-sign-in"
      data-testid="builtin-password-sign-in"
    >
      {loginEntrySelector ? (
        <button
          aria-label={t('sign_in.back_to_login_options')}
          className="builtin-password-sign-in__back"
          onClick={loginEntrySelector.request}
          type="button"
        >
          <span aria-hidden="true">←</span>
        </button>
      ) : null}
      <h2 className="builtin-password-sign-in__title">
        {t('sign_in.fallback_title')}
      </h2>
      {failed ? (
        <div className="builtin-password-sign-in__error" role="alert">
          {t('sign_in.fallback_authentication_failed')}
        </div>
      ) : null}
      <form
        aria-label="Escape password sign in"
        className="builtin-password-sign-in__form"
        onSubmit={submit}
      >
        <label className="builtin-password-sign-in__field">
          {t('sign_in.fallback_identifier_label')}
          <input
            required
            autoComplete="username"
            className="builtin-password-sign-in__input"
            value={identifier}
            onChange={(event) => setIdentifier(event.target.value)}
          />
        </label>
        <label className="builtin-password-sign-in__field">
          {t('sign_in.fallback_password_label')}
          <input
            required
            autoComplete="current-password"
            className="builtin-password-sign-in__input"
            type="password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
          />
        </label>
        <button
          className="builtin-password-sign-in__submit"
          disabled={pending}
          type="submit"
        >
          {t('sign_in.fallback_submit')}
        </button>
      </form>
    </div>
  );
}
