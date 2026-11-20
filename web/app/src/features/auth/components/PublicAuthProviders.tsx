import { useState, type PropsWithChildren } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { I18nextProvider } from 'react-i18next';
import { AppThemeProvider } from '@1flowbase/ui/app-theme-provider';

import { appI18n } from '../../../shared/i18n/app-i18n';

import { systemMetadataQueryKey } from '../../../shared/api/system-metadata';
import { findDocumentArtwork } from './hero/document-motion';

const openingClients = new WeakMap<HTMLElement, QueryClient>();
function publicQueryClient() {
  const art = findDocumentArtwork();
  const existing = art && openingClients.get(art);
  if (existing) return existing;
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  if (art) {
    openingClients.set(art, client);
    if (art.metadata) client.setQueryData(systemMetadataQueryKey, art.metadata);
    else if (art.metadataFlight) {
      const pending = art.metadataFlight;
      void client.prefetchQuery({
        queryKey: systemMetadataQueryKey,
        queryFn: () => pending,
        staleTime: 60_000
      });
    }
  }
  return client;
}

export function PublicAuthProviders({ children }: PropsWithChildren) {
  const [queryClient] = useState(publicQueryClient);
  return (
    <AppThemeProvider>
      <QueryClientProvider client={queryClient}>
        <I18nextProvider i18n={appI18n}>{children}</I18nextProvider>
      </QueryClientProvider>
    </AppThemeProvider>
  );
}
