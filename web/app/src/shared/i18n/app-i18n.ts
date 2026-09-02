import i18next, { type Resource } from 'i18next';
import { initReactI18next } from 'react-i18next';

import authEnUS from '../../features/auth/i18n/en_US.json';
import authZhHans from '../../features/auth/i18n/zh_Hans.json';
import {
  FALLBACK_APP_LOCALE,
  resolveAppLocale,
  SUPPORTED_APP_LOCALES
} from './locales';
import {
  readLocalePreferenceFromStorage,
  readLocalePreferenceFromUrl
} from '../user-preferences/locale-preference';

const publicTranslationResources = {
  zh_Hans: { auth: authZhHans },
  en_US: { auth: authEnUS }
} as const;

let applicationResourcesFlight: Promise<void> | undefined;

function getInitialAppLocale() {
  return resolveAppLocale(
    readLocalePreferenceFromStorage() ?? readLocalePreferenceFromUrl()
  );
}

export const appI18n = i18next.createInstance();

void appI18n.use(initReactI18next).init({
  resources: publicTranslationResources as unknown as Resource,
  lng: getInitialAppLocale(),
  fallbackLng: FALLBACK_APP_LOCALE,
  ns: ['auth'],
  supportedLngs: [...SUPPORTED_APP_LOCALES],
  defaultNS: 'me',
  initAsync: false,
  interpolation: {
    escapeValue: false
  },
  react: {
    useSuspense: false
  }
});

export function loadApplicationI18nResources(): Promise<void> {
  applicationResourcesFlight ??= import('./application-i18n-resources')
    .then(({ applicationTranslationResources }) => {
      for (const [locale, namespaces] of Object.entries(
        applicationTranslationResources
      )) {
        for (const [namespace, resource] of Object.entries(namespaces)) {
          appI18n.addResourceBundle(locale, namespace, resource, true, true);
        }
      }
    })
    .catch((error) => {
      applicationResourcesFlight = undefined;
      throw error;
    });

  return applicationResourcesFlight;
}
