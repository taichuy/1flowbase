import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import { useAuthStore } from '../../../../state/auth-store';
import { resolveAppLocale } from '../../../../shared/i18n/locales';
import { fetchFrontstageRuntimeI18nCatalog } from '../../api/runtime-i18n';
import { createBlockI18n } from '../../lib/runtime-i18n/translator';

export function useBlockI18n(workspaceId: string, active: boolean) {
  const { i18n } = useTranslation();
  const locale = resolveAppLocale(i18n.resolvedLanguage ?? i18n.language);
  const actor = useAuthStore((state) => state.actor);
  const me = useAuthStore((state) => state.me);
  const sessionStatus = useAuthStore((state) => state.sessionStatus);
  const enabled =
    active &&
    sessionStatus === 'authenticated' &&
    actor?.current_workspace_id === workspaceId;
  const permissions = me?.permissions.slice().sort().join(',') ?? '';
  const query = useQuery({
    queryKey: [
      'frontstage',
      workspaceId,
      'runtime-i18n',
      actor?.id,
      permissions,
      locale
    ],
    queryFn: () => fetchFrontstageRuntimeI18nCatalog(locale),
    enabled,
    staleTime: 5 * 60 * 1000,
    retry: false
  });
  // Never expose a prior identity/workspace/locale snapshot while the new one loads.
  const catalog = enabled && !query.isError ? query.data : undefined;
  const status = query.isError ? 'error' : catalog ? 'ready' : 'loading';
  return useMemo(
    () => createBlockI18n({ locale, status, messages: catalog?.messages }),
    [catalog, locale, status]
  );
}
