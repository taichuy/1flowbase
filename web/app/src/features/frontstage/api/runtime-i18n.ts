import { getRuntimeI18nCatalog } from '@1flowbase/api-client';

import { getFrontstageApiBaseUrl } from './page-tree';

export async function fetchFrontstageRuntimeI18nCatalog(locale: string) {
  const response = await getRuntimeI18nCatalog(
    { locale },
    getFrontstageApiBaseUrl()
  );
  // This consumer does not send If-None-Match; it requires a complete snapshot.
  if (response.kind !== 'ok') {
    throw new Error('Runtime i18n catalog returned no snapshot.');
  }
  if (response.value.locale !== locale) {
    throw new Error('Runtime i18n catalog returned a different locale.');
  }
  return response.value;
}
