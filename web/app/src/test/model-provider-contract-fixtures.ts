import type {
  ConsoleModelProviderCatalogResponse,
  ConsoleModelProviderOptions
} from '@1flowbase/api-client';
import modelProviderCatalogContractJson from '@1flowbase/model-provider-contracts/catalog.multiple-providers.json';
import modelProviderOptionsContractJson from '@1flowbase/model-provider-contracts/options.multiple-providers.json';

const modelProviderCatalogContractBase = modelProviderCatalogContractJson as {
  locale_meta: ConsoleModelProviderCatalogResponse['locale_meta'];
  i18n_catalog: ConsoleModelProviderCatalogResponse['i18n_catalog'];
  entries: Array<
    Omit<
      ConsoleModelProviderCatalogResponse['entries'][number],
      'desired_state' | 'availability_status'
    > & {
      enabled?: boolean;
    }
  >;
};

export const modelProviderCatalogContract = {
  ...modelProviderCatalogContractBase,
  entries: modelProviderCatalogContractBase.entries.map((entry) => ({
    ...entry,
    desired_state: entry.enabled === false ? 'disabled' : 'enabled',
    availability_status: entry.enabled === false ? 'disabled' : 'available'
  }))
} satisfies ConsoleModelProviderCatalogResponse;

export const modelProviderOptionsContract =
  modelProviderOptionsContractJson as ConsoleModelProviderOptions;

export const modelProviderCatalogEntries = modelProviderCatalogContract.entries;
export const modelProviderOptionsProviders =
  modelProviderOptionsContract.providers;
export const primaryContractProviderGroups =
  modelProviderOptionsContract.providers[0].model_groups;
export const primaryContractProviderModels = primaryContractProviderGroups.map(
  (group) => group.model
);
export const primaryContractProviderEnabledModelIds =
  primaryContractProviderModels.map((model) => model.model_id);
