import { useState, type PropsWithChildren } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { I18nextProvider } from 'react-i18next';
import { AppThemeProvider } from '@1flowbase/ui/app-theme-provider';

import { appI18n } from '../../../shared/i18n/app-i18n';

export function PublicAuthProviders({ children }: PropsWithChildren) {
  const [queryClient] = useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } })
  );
  return (
    <AppThemeProvider>
      <QueryClientProvider client={queryClient}>
        <I18nextProvider i18n={appI18n}>{children}</I18nextProvider>
      </QueryClientProvider>
    </AppThemeProvider>
  );
}
