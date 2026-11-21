import type { i18n } from 'i18next';
import { FALLBACK_APP_LOCALE, type AppLocale } from './locales';

type ResourceLoaders<Namespace extends string> = Record<
  Namespace,
  Record<AppLocale, () => Promise<object>>
>;

// Factory-scoped flights also let cold fixtures use an empty i18next instance,
// independent of test/setup.ts (which intentionally registers all dictionaries).
export function createApplicationResourceLoader<Namespace extends string>(
  instance: i18n,
  loaders: ResourceLoaders<Namespace>
) {
  const flights = new Map<string, Promise<void>>();
  const demanded = new Set<Namespace>();
  let activation = 0;

  function ready(locale: AppLocale, namespaces: readonly Namespace[]) {
    return [locale, FALLBACK_APP_LOCALE].every((language) =>
      namespaces.every((namespace) =>
        instance.hasResourceBundle(language, namespace)
      )
    );
  }

  function loadBundle(locale: AppLocale, namespace: Namespace): Promise<void> {
    if (instance.hasResourceBundle(locale, namespace)) return Promise.resolve();
    const key = `${locale}:${namespace}`;
    const existing = flights.get(key);
    if (existing) return existing;
    const flight = loaders[namespace][locale]()
      .then((resource) => {
        instance.addResourceBundle(locale, namespace, resource, true, true);
      })
      .finally(() => {
        flights.delete(key);
      });
    flights.set(key, flight);
    return flight;
  }

  async function load(locale: AppLocale, namespaces: readonly Namespace[]) {
    namespaces.forEach((namespace) => demanded.add(namespace));
    await Promise.all(
      [...new Set([locale, FALLBACK_APP_LOCALE])].flatMap((language) =>
        namespaces.map((namespace) => loadBundle(language, namespace))
      )
    );
  }

  async function activate(
    locale: AppLocale,
    namespaces: readonly Namespace[] = []
  ) {
    const request = ++activation;
    namespaces.forEach((namespace) => demanded.add(namespace));
    // Retained windows/routes remain mounted on a language switch. Include their
    // already-demanded namespaces, and any route requested while this load waits.
    do {
      await load(locale, [...demanded]);
      if (request !== activation) return false;
    } while (!ready(locale, [...demanded]));
    if (instance.language !== locale) await instance.changeLanguage(locale);
    return true;
  }

  return { load, ready, activate };
}
