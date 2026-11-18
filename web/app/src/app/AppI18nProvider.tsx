import ConfigProvider from 'antd/es/config-provider';
import enUS from 'antd/locale/en_US';
import zhCN from 'antd/locale/zh_CN';
import 'dayjs/locale/zh-cn';
import type { PropsWithChildren } from 'react';
import { useEffect, useSyncExternalStore } from 'react';
import { I18nextProvider } from 'react-i18next';

import { useAuthStore } from '../state/auth-store';
import { appI18n } from '../shared/i18n/app-i18n';
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

  useEffect(() => {
    void appI18n.changeLanguage(appLocale);
  }, [appLocale]);

  useEffect(() => {
    if (!userLocalePreference) {
      return;
    }

    writeLocalePreferenceToStorage(userLocalePreference);
  }, [userLocalePreference]);

  return (
    <I18nextProvider i18n={appI18n}>
      <ConfigProvider locale={getAntdLocale(appLocale)}>
        {children}
      </ConfigProvider>
    </I18nextProvider>
  );
}
