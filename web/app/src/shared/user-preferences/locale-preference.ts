import type { ConsoleUserMeta } from '@1flowbase/api-client';

export type ProfileLocalePreference = 'zh_Hans' | 'en_US';

const LOCALE_PREFERENCE_STORAGE_KEY = '1flowbase.ui.locale_preference';
export const LOCALE_PREFERENCE_CHANGE_EVENT = '1flowbase:locale-preference-changed';

const URL_LANGUAGE_TO_PROFILE_LOCALE = {
  zh: 'zh_Hans',
  'zh-cn': 'zh_Hans',
  'zh-hans': 'zh_Hans',
  en: 'en_US',
  'en-us': 'en_US'
} as const;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isProfileLocalePreference(value: unknown): value is ProfileLocalePreference {
  return value === 'zh_Hans' || value === 'en_US';
}

function normalizeUrlLanguage(value: string | null): keyof typeof URL_LANGUAGE_TO_PROFILE_LOCALE | null {
  if (!value) {
    return null;
  }

  const normalized = value.trim().replace('_', '-').toLowerCase();

  return normalized in URL_LANGUAGE_TO_PROFILE_LOCALE
    ? (normalized as keyof typeof URL_LANGUAGE_TO_PROFILE_LOCALE)
    : null;
}

function readLocalePreferenceFromMeta(
  meta: ConsoleUserMeta | undefined
): ProfileLocalePreference | null {
  const ui = isRecord(meta?.ui) ? meta.ui : undefined;
  const locale = isRecord(ui?.locale) ? ui.locale : undefined;
  const preferredLocale = locale?.preferred_locale;

  return isProfileLocalePreference(preferredLocale) ? preferredLocale : null;
}

export function mergeLocalePreferenceMeta(
  meta: ConsoleUserMeta | undefined,
  preferredLocale: ProfileLocalePreference
): ConsoleUserMeta {
  const currentMeta = isRecord(meta) ? meta : {};
  const currentUi = isRecord(currentMeta.ui) ? currentMeta.ui : {};
  const currentLocale = isRecord(currentUi.locale) ? currentUi.locale : {};

  return {
    ...currentMeta,
    ui: {
      ...currentUi,
      locale: {
        ...currentLocale,
        preferred_locale: preferredLocale
      }
    }
  };
}

export function readLocalePreferenceFromStorage(): ProfileLocalePreference | null {
  if (typeof window === 'undefined') {
    return null;
  }

  const storedValue = window.localStorage.getItem(LOCALE_PREFERENCE_STORAGE_KEY);

  return isProfileLocalePreference(storedValue) ? storedValue : null;
}

export function readLocalePreferenceFromUrl(): ProfileLocalePreference | null {
  if (typeof window === 'undefined') {
    return null;
  }

  const language = normalizeUrlLanguage(
    new URLSearchParams(window.location.search).get('language')
  );

  return language ? URL_LANGUAGE_TO_PROFILE_LOCALE[language] : null;
}

export function writeLocalePreferenceToStorage(
  preferredLocale: ProfileLocalePreference
): void {
  if (typeof window === 'undefined') {
    return;
  }

  window.localStorage.setItem(LOCALE_PREFERENCE_STORAGE_KEY, preferredLocale);
  window.dispatchEvent(
    new CustomEvent(LOCALE_PREFERENCE_CHANGE_EVENT, {
      detail: preferredLocale
    })
  );
}

export function resolveUserLocalePreference(
  preferredLocale: string | null | undefined,
  meta: ConsoleUserMeta | undefined
): ProfileLocalePreference | null {
  const metaLocale = readLocalePreferenceFromMeta(meta);

  if (metaLocale) {
    return metaLocale;
  }

  return isProfileLocalePreference(preferredLocale) ? preferredLocale : null;
}
