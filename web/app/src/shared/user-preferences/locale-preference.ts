import type { ConsoleUserMeta } from '@1flowbase/api-client';

export type ProfileLocalePreference = 'zh_Hans' | 'en_US';

const STORAGE_KEY = '1flowbase.ui.locale_preference';
export const LOCALE_PREFERENCE_CHANGE_EVENT = '1flowbase:locale-preference-changed';

const URL_MAP: Record<string, ProfileLocalePreference> = {
  zh: 'zh_Hans',
  'zh-cn': 'zh_Hans',
  'zh-hans': 'zh_Hans',
  en: 'en_US',
  'en-us': 'en_US'
};

const isRec = (v: unknown): v is Record<string, unknown> =>
  typeof v === 'object' && v !== null && !Array.isArray(v);

const isPref = (v: unknown): v is ProfileLocalePreference =>
  v === 'zh_Hans' || v === 'en_US';

export function mergeLocalePreferenceMeta(
  meta: ConsoleUserMeta | undefined,
  preferredLocale: ProfileLocalePreference
): ConsoleUserMeta {
  const m = isRec(meta) ? meta : {};
  const ui = isRec(m.ui) ? m.ui : {};
  const loc = isRec(ui.locale) ? ui.locale : {};
  return { ...m, ui: { ...ui, locale: { ...loc, preferred_locale: preferredLocale } } };
}

export function readLocalePreferenceFromStorage(): ProfileLocalePreference | null {
  if (typeof window === 'undefined') return null;
  const v = window.localStorage.getItem(STORAGE_KEY);
  return isPref(v) ? v : null;
}

export function readLocalePreferenceFromUrl(): ProfileLocalePreference | null {
  if (typeof window === 'undefined') return null;
  const raw = new URLSearchParams(window.location.search).get('language');
  return raw ? (URL_MAP[raw.trim().replace('_', '-').toLowerCase()] ?? null) : null;
}

export function writeLocalePreferenceToStorage(
  preferredLocale: ProfileLocalePreference
): void {
  if (typeof window === 'undefined') return;
  window.localStorage.setItem(STORAGE_KEY, preferredLocale);
  window.dispatchEvent(
    new CustomEvent(LOCALE_PREFERENCE_CHANGE_EVENT, { detail: preferredLocale })
  );
}

export function resolveUserLocalePreference(
  preferredLocale: string | null | undefined,
  meta: ConsoleUserMeta | undefined
): ProfileLocalePreference | null {
  const ui = isRec(meta?.ui) ? meta.ui : undefined;
  const loc = isRec(ui?.locale) ? ui.locale : undefined;
  if (isPref(loc?.preferred_locale)) return loc.preferred_locale;
  return isPref(preferredLocale) ? preferredLocale : null;
}
