import { resolveAppLocale } from './locales';
import {
  readLocalePreferenceFromStorage,
  readLocalePreferenceFromUrl
} from '../user-preferences/locale-preference';

export function getInitialAppLocale() {
  return resolveAppLocale(
    readLocalePreferenceFromStorage() ?? readLocalePreferenceFromUrl()
  );
}
