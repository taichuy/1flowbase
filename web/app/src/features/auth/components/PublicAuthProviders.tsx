import type { PropsWithChildren } from 'react';
import { I18nextProvider } from 'react-i18next';
import { AppThemeProvider } from '@1flowbase/ui/app-theme-provider';

import { appI18n } from '../../../shared/i18n/app-i18n';

export function PublicAuthProviders({ children }: PropsWithChildren) {
  return (
    <AppThemeProvider>
      <I18nextProvider i18n={appI18n}>{children}</I18nextProvider>
    </AppThemeProvider>
  );
}
