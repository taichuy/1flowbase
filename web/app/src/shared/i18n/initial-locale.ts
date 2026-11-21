import type { ConsoleMe } from '@1flowbase/api-client/auth';
import { resolveAppLocale } from './locales';
import {
  readLocalePreferenceFromStorage,
  resolveUserLocalePreference,
  readLocalePreferenceFromUrl
} from '../user-preferences/locale-preference';

export function getInitialAppLocale() {
  return resolveAppLocale(
    readLocalePreferenceFromStorage() ?? readLocalePreferenceFromUrl()
  );
}

export function resolveAuthenticatedLocale(
  me: Pick<ConsoleMe, 'preferred_locale' | 'meta'> | null | undefined
) {
  return resolveAppLocale(
    resolveUserLocalePreference(me?.preferred_locale, me?.meta) ??
      readLocalePreferenceFromStorage() ??
      readLocalePreferenceFromUrl()
  );
}
