import { useCallback, useSyncExternalStore } from 'react';
import { en_US, zh_Hans } from 'virtual:1flowbase-auth-translations';
import { getInitialAppLocale } from '../../../shared/i18n/initial-locale';
import { LOCALE_PREFERENCE_CHANGE_EVENT } from '../../../shared/user-preferences/locale-preference';
import type { AppLocale } from '../../../shared/i18n/locales';
import { subscribePublicLocation } from './anonymous-navigation';

// These are the existing JSON values, not a second copy/catalog. MCP keeps its
// namespace in appI18n behind its full-runtime boundary.
const resources = {
  en_US,
  zh_Hans
};
type Options = Record<string, unknown> & { defaultValue?: string };

function subscribeLocale(listener: () => void) {
  const unsubscribe = subscribePublicLocation(listener);
  window.addEventListener(LOCALE_PREFERENCE_CHANGE_EVENT, listener);
  window.addEventListener('storage', listener);
  return () => {
    unsubscribe();
    window.removeEventListener(LOCALE_PREFERENCE_CHANGE_EVENT, listener);
    window.removeEventListener('storage', listener);
  };
}
export function useAuthLocale() {
  return useSyncExternalStore(
    subscribeLocale,
    getInitialAppLocale,
    () => 'en_US' as const
  );
}
function lookup(locale: AppLocale, key: string): string | undefined {
  let value: unknown = resources[locale];
  for (const part of key.split('.'))
    value =
      value && typeof value === 'object' && Object.hasOwn(value, part)
        ? (value as Record<string, unknown>)[part]
        : undefined;
  return typeof value === 'string' ? value : undefined;
}
export function authText(
  key: string,
  options: Options = {},
  locale = getInitialAppLocale()
): string {
  const text =
    lookup(locale, key) ?? lookup('en_US', key) ?? options.defaultValue ?? key;
  // appI18n uses escapeValue:false; React performs output escaping.
  return text.replace(/{{\s*([^{}]+?)\s*}}/g, (placeholder, name: string) => {
    const value = name
      .split('.')
      .reduce<unknown>(
        (current, part) =>
          current && typeof current === 'object' && Object.hasOwn(current, part)
            ? (current as Record<string, unknown>)[part]
            : undefined,
        options
      );
    return value === undefined
      ? Object.hasOwn(options, name)
        ? ''
        : placeholder
      : value === null
        ? ''
        : String(value);
  });
}
export function useAuthTranslation() {
  const locale = useAuthLocale();
  const t = useCallback(
    (key: string, options?: Options) => authText(key, options, locale),
    [locale]
  );
  return { t };
}
