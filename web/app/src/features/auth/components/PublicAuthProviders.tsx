import '../../../styles/antd-defaults.css';
import { useLayoutEffect, useState, type PropsWithChildren } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { I18nextProvider } from 'react-i18next';
import { AppThemeProvider } from '@1flowbase/ui/app-theme-provider';
import { appI18n } from '../../../shared/i18n/app-i18n';
import { useAuthLocale } from '../lib/auth-i18n';

// Only custom auth and MCP mount this full provider stack. Public metadata is a
// document promise/state handoff, not a second React Query cache with a TTL.
export function PublicAuthProviders({ children }: PropsWithChildren) {
  const [queryClient] = useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } })
  );
  const locale = useAuthLocale();
  useLayoutEffect(() => {
    void appI18n.changeLanguage(locale);
  }, [locale]);
  return (
    <AppThemeProvider>
      <QueryClientProvider client={queryClient}>
        <I18nextProvider i18n={appI18n}>{children}</I18nextProvider>
      </QueryClientProvider>
    </AppThemeProvider>
  );
}
