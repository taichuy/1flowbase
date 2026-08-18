import type { ConsolePricingCatalogFilter } from '@1flowbase/api-client';

export {
  createConsolePricingRule as createSettingsPricingRule,
  deleteConsolePricingRule as deleteSettingsPricingRule,
  executeConsoleCreditCommand as executeSettingsCreditCommand,
  getConsolePricingCatalog as getSettingsPricingCatalog,
  importConsolePricingCatalog as importSettingsPricingCatalog,
  listConsoleCreditAccounts as listSettingsCreditAccounts,
  listConsoleCreditLedger as listSettingsCreditLedger,
  listConsolePricingRules as listSettingsPricingRules,
  updateConsolePricingRule as updateSettingsPricingRule,
  type ConsoleCreditAccount as SettingsCreditAccount,
  type ConsoleCreditTransaction as SettingsCreditTransaction,
  type ConsolePricingCatalog as SettingsPricingCatalog,
  type ConsolePricingCatalogFilter as SettingsPricingCatalogFilter,
  type ConsolePricingRule as SettingsPricingRule,
  type ConsolePricingRuleInput as SettingsPricingRuleInput,
  type ConsolePricingRulesFilter as SettingsPricingRulesFilter,
  type ConsolePricingRulesPage as SettingsPricingRulesPage
} from '@1flowbase/api-client';

export const settingsPricingRulesQueryKey = [
  'settings',
  'billing',
  'pricing-rules'
] as const;
export const settingsPricingCatalogQueryKey = (
  filter: ConsolePricingCatalogFilter
) => ['settings', 'billing', 'pricing-catalog', filter] as const;
export const settingsCreditAccountsQueryKey = [
  'settings',
  'billing',
  'credit-accounts'
] as const;
export const settingsCreditLedgerQueryKey = (userId?: string) =>
  ['settings', 'billing', 'credit-ledger', userId ?? 'all'] as const;
