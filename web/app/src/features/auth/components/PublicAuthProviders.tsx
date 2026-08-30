import type { PropsWithChildren } from 'react';
import { I18nextProvider } from 'react-i18next';

import { appI18n } from '../../../shared/i18n/app-i18n';

export function PublicAuthProviders({ children }: PropsWithChildren) {
  return <I18nextProvider i18n={appI18n}>{children}</I18nextProvider>;
}
