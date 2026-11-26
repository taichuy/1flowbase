import type {
  ConsolePricingCatalogFilter,
  ConsolePricingTargetsFilter
} from '@1flowbase/api-client';

export {
  createConsolePricingRule as createSettingsPricingRule,
  deleteConsolePricingRule as deleteSettingsPricingRule,
  executeConsoleCreditCommand as executeSettingsCreditCommand,
  getConsolePricingCatalog as getSettingsPricingCatalog,
  importConsolePricingCatalog as importSettingsPricingCatalog,
  syncConsolePricingCatalog as syncSettingsPricingCatalog,
  listConsoleCreditAccounts as listSettingsCreditAccounts,
  listConsoleCreditLedger as listSettingsCreditLedger,
  listConsolePricingRules as listSettingsPricingRules,
  listConsolePricingTargets as listSettingsPricingTargets,
  updateConsolePricingRule as updateSettingsPricingRule,
  type ConsoleCreditAccount as SettingsCreditAccount,
  type ConsoleCreditTransaction as SettingsCreditTransaction,
  type ConsolePricingCatalog as SettingsPricingCatalog,
  type ConsolePricingCatalogFilter as SettingsPricingCatalogFilter,
  type ConsolePricingRule as SettingsPricingRule,
  type ConsolePricingRuleInput as SettingsPricingRuleInput,
  type ConsolePricingRulesFilter as SettingsPricingRulesFilter,
  type ConsolePricingRulesPage as SettingsPricingRulesPage,
  type ConsolePricingTarget as SettingsPricingTarget,
  type ConsolePricingTargetProvider as SettingsPricingTargetProvider,
  type ConsolePricingTargetsFilter as SettingsPricingTargetsFilter,
  type ConsolePricingTargetsPage as SettingsPricingTargetsPage
} from '@1flowbase/api-client';

export const settingsPricingRulesQueryKey = [
  'settings',
  'billing',
  'pricing-rules'
] as const;
export const settingsPricingTargetsQueryKey = (
  filter: ConsolePricingTargetsFilter
) => ['settings', 'billing', 'pricing-targets', filter] as const;
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
