import i18next, { type Resource } from 'i18next';
import { initReactI18next } from 'react-i18next';

import authEnUS from '../../features/auth/i18n/en_US.json';
import authZhHans from '../../features/auth/i18n/zh_Hans.json';
import { FALLBACK_APP_LOCALE, SUPPORTED_APP_LOCALES } from './locales';
import { getInitialAppLocale } from './initial-locale';
import {
  applicationTranslationLoaders,
  type ApplicationNamespace
} from './application-i18n-resources';
import { applicationNamespacesForPath } from './application-i18n-demand';
import { createApplicationResourceLoader } from './application-resource-loader';
import type { AppLocale } from './locales';

const publicTranslationResources = {
  zh_Hans: { auth: authZhHans },
  en_US: { auth: authEnUS }
} as const;

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

export const applicationI18nLoader = createApplicationResourceLoader(
  appI18n,
  applicationTranslationLoaders
);

export function prepareApplicationI18n(locale: AppLocale, pathname: string) {
  return applicationI18nLoader.activate(
    locale,
    applicationNamespacesForPath(pathname)
  );
}

// Explicit full registration for tests/catalog tooling only. Production entry and
// route loaders use prepareApplicationI18n instead of pulling every namespace.
export async function loadApplicationI18nResources(): Promise<void> {
  const namespaces = Object.keys(
    applicationTranslationLoaders
  ) as ApplicationNamespace[];
  await Promise.all(
    SUPPORTED_APP_LOCALES.map((locale) =>
      applicationI18nLoader.load(locale, namespaces)
    )
  );
}
