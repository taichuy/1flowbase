type CatalogTextSource = {
  locale_meta: Record<string, unknown>;
  i18n_catalog: Record<string, unknown>;
};

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null
    ? (value as Record<string, unknown>)
    : null;
}

function readLocalizedText(bundle: Record<string, unknown>, key: string) {
  let current: unknown = bundle;

  for (const segment of key.split('.')) {
    current = asRecord(current)?.[segment];
    if (current === undefined) {
      return null;
    }
  }

  return typeof current === 'string' ? current : null;
}

export function lookupCatalogText(
  response: CatalogTextSource,
  namespace: string,
  keys: readonly string[]
): string | null {
  const localeCatalog = asRecord(asRecord(response.i18n_catalog)?.[namespace]);
  if (!localeCatalog) {
    return null;
  }

  const locales = new Set(
    [
      response.locale_meta.resolved_locale,
      response.locale_meta.fallback_locale,
      'zh_Hans',
      'en_US',
      ...Object.keys(localeCatalog)
    ].filter((locale): locale is string => typeof locale === 'string')
  );

  // Locale priority wins over key priority, including provider and plugin labels.
  for (const locale of locales) {
    const bundle = asRecord(localeCatalog[locale]);
    if (!bundle) {
      continue;
    }

    for (const key of keys) {
      const text = readLocalizedText(bundle, key);
      if (text) {
        return text;
      }
    }
  }

  return null;
}
