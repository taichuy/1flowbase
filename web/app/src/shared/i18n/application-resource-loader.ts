import type { i18n } from 'i18next';
import { FALLBACK_APP_LOCALE, type AppLocale } from './locales';

type ResourceLoaders<Namespace extends string> = Record<
  Namespace,
  Record<AppLocale, () => Promise<object>>
>;

interface ApplicationResourceLoaderOptions {
  scheduleBackground?: (task: () => void) => void;
}

// Factory-scoped flights also let cold fixtures use an empty i18next instance,
// independent of test/setup.ts (which intentionally registers all dictionaries).
export function createApplicationResourceLoader<Namespace extends string>(
  instance: i18n,
  loaders: ResourceLoaders<Namespace>,
  options: ApplicationResourceLoaderOptions = {}
) {
  const flights = new Map<string, Promise<void>>();
  const demanded = new Set<Namespace>();
  const customScheduleBackground = options.scheduleBackground;
  let activation = 0;

  if (!customScheduleBackground && typeof instance.on === 'function') {
    instance.on('missingKey', (_lngs, namespace) => {
      const ns = namespace as Namespace;
      if (
        ns &&
        ns in loaders &&
        instance.language !== FALLBACK_APP_LOCALE &&
        !instance.hasResourceBundle(FALLBACK_APP_LOCALE, ns)
      ) {
        void loadBundle(FALLBACK_APP_LOCALE, ns).catch(() => undefined);
      }
    });
  }

  function readyPrimary(locale: AppLocale, namespaces: readonly Namespace[]) {
    return namespaces.every((namespace) =>
      instance.hasResourceBundle(locale, namespace)
    );
  }

  function ready(locale: AppLocale, namespaces: readonly Namespace[]) {
    return [locale, FALLBACK_APP_LOCALE].every((language) =>
      readyPrimary(language, namespaces)
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

  async function loadSingleLocale(
    locale: AppLocale,
    namespaces: readonly Namespace[]
  ) {
    await Promise.all(
      namespaces.map((namespace) => loadBundle(locale, namespace))
    );
  }

  async function demandPrimary(
    locale: AppLocale,
    namespaces: readonly Namespace[]
  ) {
    namespaces.forEach((namespace) => demanded.add(namespace));
    await loadSingleLocale(locale, namespaces);
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

  async function activatePrimary(
    locale: AppLocale,
    namespaces: readonly Namespace[] = []
  ) {
    const request = ++activation;
    namespaces.forEach((namespace) => demanded.add(namespace));
    do {
      await loadSingleLocale(locale, [...demanded]);
      if (request !== activation) return false;
    } while (!readyPrimary(locale, [...demanded]));
    if (instance.language !== locale) await instance.changeLanguage(locale);
    if (
      customScheduleBackground &&
      locale !== FALLBACK_APP_LOCALE &&
      !readyPrimary(FALLBACK_APP_LOCALE, [...demanded])
    ) {
      const snapshotNamespaces = [...demanded];
      customScheduleBackground(() => {
        void loadSingleLocale(FALLBACK_APP_LOCALE, snapshotNamespaces).catch(
          () => undefined
        );
      });
    }
    return true;
  }

  return { load, demandPrimary, ready, readyPrimary, activate, activatePrimary };
}
