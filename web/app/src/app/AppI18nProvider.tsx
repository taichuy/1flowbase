import ConfigProvider from 'antd/es/config-provider';
import enUS from 'antd/locale/en_US';
import zhCN from 'antd/locale/zh_CN';
import 'dayjs/locale/zh-cn';
import type { PropsWithChildren } from 'react';
import { useEffect, useState, useSyncExternalStore } from 'react';
import { I18nextProvider } from 'react-i18next';

import { useAuthStore } from '../state/auth-store';
import { WorkspaceShellSkeleton } from '../features/auth/components/anonymous-loading/WorkspaceShellSkeleton';
import {
  appI18n,
  isShellTopbarI18nReady,
  prepareApplicationI18n,
  prepareShellTopbarI18n
} from '../shared/i18n/app-i18n';
import { resolveAppLocale, type AppLocale } from '../shared/i18n/locales';
import {
  LOCALE_PREFERENCE_CHANGE_EVENT,
  readLocalePreferenceFromStorage,
  readLocalePreferenceFromUrl,
  resolveUserLocalePreference,
  writeLocalePreferenceToStorage
} from '../shared/user-preferences/locale-preference';

function getAntdLocale(locale: AppLocale) {
  return locale === 'en_US' ? enUS : zhCN;
}

function subscribeToLocalePreference(onStoreChange: () => void) {
  window.addEventListener(LOCALE_PREFERENCE_CHANGE_EVENT, onStoreChange);
  window.addEventListener('storage', onStoreChange);
  return () => {
    window.removeEventListener(LOCALE_PREFERENCE_CHANGE_EVENT, onStoreChange);
    window.removeEventListener('storage', onStoreChange);
  };
}

function getServerLocalePreference() {
  return null;
}

function subscribeToAppliedLanguage(onStoreChange: () => void) {
  appI18n.on('languageChanged', onStoreChange);
  return () => {
    appI18n.off('languageChanged', onStoreChange);
  };
}
function getAppliedLocale() {
  return resolveAppLocale(appI18n.language);
}

export function AppI18nProvider({ children }: PropsWithChildren) {
  const preferredLocale = useAuthStore((state) => state.me?.preferred_locale);
  const userMeta = useAuthStore((state) => state.me?.meta);
  const storedLocalePreference = useSyncExternalStore(
    subscribeToLocalePreference,
    readLocalePreferenceFromStorage,
    getServerLocalePreference
  );
  const urlLocalePreference = readLocalePreferenceFromUrl();
  const userLocalePreference = resolveUserLocalePreference(
    preferredLocale,
    userMeta
  );
  const resolvedUserLocalePreference =
    userLocalePreference ?? storedLocalePreference ?? urlLocalePreference;
  const appLocale = resolveAppLocale(resolvedUserLocalePreference);

  const appliedLocale = useSyncExternalStore(
    subscribeToAppliedLanguage,
    getAppliedLocale,
    getAppliedLocale
  );
  const [ready, setReady] = useState(
    () =>
      appliedLocale === appLocale &&
      isShellTopbarI18nReady(appLocale, window.location.pathname)
  );
  const [initialError, setInitialError] = useState<unknown>(null);

  useEffect(() => {
    let active = true;
    const loader = ready ? prepareApplicationI18n : prepareShellTopbarI18n;
    void loader(appLocale, window.location.pathname)
      .then((applied) => {
        if (active && applied) setReady(true);
      })
      .catch((error: unknown) => {
        // A failed switch must retain the displayed page, drafts and old language.
        if (active && !ready) setInitialError(error);
      });
    return () => {
      active = false;
    };
  }, [appLocale, ready]);

  useEffect(() => {
    if (!userLocalePreference) {
      return;
    }

    writeLocalePreferenceToStorage(userLocalePreference);
  }, [userLocalePreference]);

  if (initialError) throw initialError;
  if (!ready) return <WorkspaceShellSkeleton mode="full" />;

  return (
    <I18nextProvider i18n={appI18n}>
      <ConfigProvider locale={getAntdLocale(appliedLocale)}>
        {children}
      </ConfigProvider>
    </I18nextProvider>
  );
}
