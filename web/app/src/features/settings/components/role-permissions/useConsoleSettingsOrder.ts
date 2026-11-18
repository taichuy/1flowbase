import { useMutation, type QueryClient } from '@tanstack/react-query';
import type { MessageInstance } from 'antd/es/message/interface';

import { useConsoleOperationAccess } from '../../../../state/console-operation-access';
import { i18nText } from '../../../../shared/i18n/text';
import { settingsConsoleNavigationQueryKey } from '../../api/console-navigation';
import {
  replaceSettingsConsolePolicyOrder,
  settingsConsolePolicyCatalogQueryKey,
  type SettingsConsolePolicyCatalog,
  type SettingsConsolePolicyCatalogLocale
} from '../../api/identity-access/permissions';
import { reorderItems } from './policy-order';
import { consoleSettingsFeatureId, isConsoleSettingsCatalogGroup } from './console-policy-draft';

export function useConsoleSettingsOrder({
  csrfToken,
  locale,
  catalog,
  queryClient,
  messageApi
}: {
  csrfToken: string | null;
  locale: SettingsConsolePolicyCatalogLocale;
  catalog: SettingsConsolePolicyCatalog | undefined;
  queryClient: QueryClient;
  messageApi: MessageInstance;
}) {
  const can = useConsoleOperationAccess();
  const canReorder = can('roles.console_settings_order.replace');
  const mutation = useMutation({
    mutationFn: async (groupIds: string[]) => {
      if (!csrfToken || !catalog)
        throw new Error('missing console settings order context');
      return replaceSettingsConsolePolicyOrder(
        catalog.settings_order_revision,
        groupIds,
        csrfToken,
        locale
      );
    },
    onSuccess: async (nextCatalog) => {
      queryClient.setQueryData(
        settingsConsolePolicyCatalogQueryKey(locale),
        nextCatalog
      );
      await queryClient.invalidateQueries({
        queryKey: settingsConsoleNavigationQueryKey
      });
      messageApi.success(
        i18nText('settings', 'auto.order_updated_successfully')
      );
    },
    onError: async () => {
      await queryClient.invalidateQueries({
        queryKey: settingsConsolePolicyCatalogQueryKey(locale)
      });
      messageApi.error(i18nText('settings', 'auto.order_update_failed'));
    }
  });

  const reorder = (oldIndex: number, newIndex: number) => {
    if (!canReorder || mutation.isPending || !catalog) return;
    const settingsGroups = catalog.groups.filter(
      isConsoleSettingsCatalogGroup
    );
    const reordered = reorderItems(settingsGroups, oldIndex, newIndex);
    queryClient.setQueryData<SettingsConsolePolicyCatalog>(
      settingsConsolePolicyCatalogQueryKey(locale),
      {
        ...catalog,
        groups: [
          ...reordered,
          ...catalog.groups.filter((group) => !isConsoleSettingsCatalogGroup(group))
        ]
      }
    );
    mutation.mutate(reordered.map((group) => consoleSettingsFeatureId(group)!));
  };

  return { reorder, canReorder, isPending: mutation.isPending };
}
