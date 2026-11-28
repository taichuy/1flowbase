import i18next, { type Resource } from 'i18next';
import { initReactI18next } from 'react-i18next';

import authEnUS from '../../features/auth/i18n/en_US.json';
import authZhHans from '../../features/auth/i18n/zh_Hans.json';
import {
  FALLBACK_APP_LOCALE,
  resolveAppLocale,
  SUPPORTED_APP_LOCALES
} from './locales';
import { getInitialAppLocale } from './initial-locale';
import {
  applicationTranslationLoaders,
  type ApplicationNamespace
} from './application-i18n-resources';
import {
  applicationNamespacesForPath,
  applicationTopbarNamespacesForPath
} from './application-i18n-demand';
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

export function isShellTopbarI18nReady(
  locale: AppLocale,
  pathname: string
): boolean {
  const namespaces = applicationTopbarNamespacesForPath(pathname);
  return import.meta.env.VITEST
    ? applicationI18nLoader.ready(locale, namespaces)
    : applicationI18nLoader.readyPrimary(locale, namespaces);
}

export function prepareShellTopbarI18n(locale: AppLocale, pathname: string) {
  const namespaces = applicationTopbarNamespacesForPath(pathname);
  return import.meta.env.VITEST
    ? applicationI18nLoader.activate(locale, namespaces)
    : applicationI18nLoader.activatePrimary(locale, namespaces);
}

export function isApplicationI18nReady(
  locale: AppLocale,
  pathname: string
): boolean {
  const namespaces = applicationNamespacesForPath(pathname);
  return import.meta.env.VITEST
    ? applicationI18nLoader.ready(locale, namespaces)
    : applicationI18nLoader.readyPrimary(locale, namespaces);
}

export function prepareApplicationI18n(locale: AppLocale, pathname: string) {
  const namespaces = applicationNamespacesForPath(pathname);
  return import.meta.env.VITEST
    ? applicationI18nLoader.activate(locale, namespaces)
    : applicationI18nLoader.activatePrimary(locale, namespaces);
}

export function ensureApplicationNamespaces(
  namespaces: readonly ApplicationNamespace[]
): Promise<void> {
  const locale = resolveAppLocale(
    appI18n.resolvedLanguage ?? appI18n.language
  );
  return import.meta.env.VITEST
    ? applicationI18nLoader.load(locale, namespaces)
    : applicationI18nLoader.demandPrimary(locale, namespaces);
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
