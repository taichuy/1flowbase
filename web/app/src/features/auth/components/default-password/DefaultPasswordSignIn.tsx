import { useLayoutEffect, useMemo, useRef, useState } from 'react';
import PasswordLocalAuth, {
  sha256Text,
  sourceSha256
} from 'virtual:1flowbase-default-password';
import {
  createDefaultPasswordContext,
  type DefaultPasswordSignInProps
} from './context';
import './default-password.css';

/** No trimming/normalization: a single changed byte belongs to the legacy host. */
export function matchesDefaultPasswordSource(source: string): boolean {
  return sha256Text(source) === sourceSha256;
}

export function DefaultPasswordSignIn({
  instance,
  loginEntrySelector = null,
  onAuthenticated
}: DefaultPasswordSignInProps) {
  const matches = useMemo(
    () => matchesDefaultPasswordSource(instance.public_ui_block),
    [instance.public_ui_block]
  );
  const [epoch, setEpoch] = useState({
    instance,
    loginEntrySelector,
    onAuthenticated,
    value: 0
  });
  // The legacy host replaces its context/form when any capability owner changes,
  // even if an entry object is replaced with another object having the same id.
  if (
    epoch.instance !== instance ||
    epoch.loginEntrySelector !== loginEntrySelector ||
    epoch.onAuthenticated !== onAuthenticated
  ) {
    setEpoch({
      instance,
      loginEntrySelector,
      onAuthenticated,
      value: epoch.value + 1
    });
  }
  // P3 owns the source-mismatch fallback; this component never renders a known
  // official form over a custom provider if accidentally called without gating.
  if (!matches) return null;
  return (
    <PasswordInstance
      key={epoch.value}
      instance={instance}
      loginEntrySelector={loginEntrySelector}
      onAuthenticated={onAuthenticated}
    />
  );
}

function PasswordInstance({
  instance,
  loginEntrySelector,
  onAuthenticated
}: DefaultPasswordSignInProps) {
  const active = useRef(false);
  useLayoutEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  const ctx = useMemo(
    () =>
      createDefaultPasswordContext(
        { instance, loginEntrySelector, onAuthenticated },
        () => active.current
      ),
    [instance, loginEntrySelector, onAuthenticated]
  );
  return (
    <div
      className="auth-default-password"
      data-testid="default-password-sign-in"
    >
      <PasswordLocalAuth ctx={ctx} />
    </div>
  );
}
