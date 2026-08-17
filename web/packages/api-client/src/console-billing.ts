import { apiFetch } from './transport';

export interface ConsolePricingRule {
  id: string;
  provider_code: string;
  upstream_model_id: string;
  input_token_unit_size: number;
  input_token_unit_price: string;
  output_token_unit_size: number;
  output_token_unit_price: string;
  cache_hit_token_unit_size: number;
  cache_hit_token_unit_price: string;
  currency_code: 'USD';
  effective_from: string;
  effective_to: string | null;
  timezone: string;
  weekday_mask: number;
  local_time_start: string | null;
  local_time_end: string | null;
  priority: number;
  enabled: boolean;
  source_kind: 'official' | 'manual';
  source_catalog_id: string | null;
  source_version: string | null;
  source_checksum: string | null;
  extensions: Record<string, unknown>;
  created_by: string | null;
  created_at: string;
  updated_at: string;
}

export type ConsolePricingRuleInput = Omit<
  ConsolePricingRule,
  'created_by' | 'created_at' | 'updated_at'
>;

export interface ConsolePricingCatalog {
  schema_version: string;
  catalog_version: string;
  currency_code: 'USD';
  rules: ConsolePricingRuleInput[];
}

export interface ConsolePricingRulesFilter {
  provider_code?: string;
  upstream_model_id?: string;
  enabled?: boolean;
  source_kind?: 'official' | 'manual';
  page?: number;
  page_size?: number;
}

export interface ConsolePricingRulesPage {
  items: ConsolePricingRule[];
  total_count: number;
  page: number;
  page_size: number;
}

export interface ConsoleCreditAccount {
  id: string;
  workspace_id: string;
  user_id: string;
  credit_unit: 'USD';
  charge_enabled: boolean;
  current_balance: string;
  reserved_amount: string;
  available_balance: string;
  credit_insufficient: boolean;
  revision: number;
  created_at: string;
  updated_at: string;
}

export interface ConsoleCreditTransaction {
  id: string;
  transaction_id: string;
  account_id: string;
  workspace_id: string;
  user_id: string;
  billing_session_id: string | null;
  actor_user_id: string | null;
  actor_plugin_id: string | null;
  transaction_type: string;
  amount: string;
  balance_after: string;
  reserved_after: string;
  credit_unit: 'USD';
  reason: string;
  source_type: string | null;
  source_id: string | null;
  idempotency_key: string;
  status: string;
  metadata: Record<string, unknown>;
  created_at: string;
}

export interface ConsoleCreditCommandInput {
  amount?: string;
  reason: string;
  source_type?: string;
  source_id?: string;
  idempotency_key: string;
  metadata?: Record<string, unknown>;
}

export function listConsolePricingRules(
  filter: ConsolePricingRulesFilter = {},
  baseUrl?: string
) {
  const search = new URLSearchParams();
  if (filter.provider_code) search.set('provider_code', filter.provider_code);
  if (filter.upstream_model_id)
    search.set('upstream_model_id', filter.upstream_model_id);
  if (filter.enabled !== undefined)
    search.set('enabled', String(filter.enabled));
  if (filter.source_kind) search.set('source_kind', filter.source_kind);
  search.set('page', String(filter.page ?? 1));
  search.set('page_size', String(filter.page_size ?? 20));
  return apiFetch<ConsolePricingRulesPage>({
    path: `/api/console/settings/billing/pricing-rules?${search.toString()}`,
    baseUrl
  });
}

export function createConsolePricingRule(
  input: Omit<ConsolePricingRuleInput, 'id'> & { id?: string },
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsolePricingRule>({
    path: '/api/console/settings/billing/pricing-rules',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateConsolePricingRule(
  id: string,
  input: ConsolePricingRuleInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsolePricingRule>({
    path: `/api/console/settings/billing/pricing-rules/${id}`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsolePricingRule(
  id: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<{ deleted: boolean }>({
    path: `/api/console/settings/billing/pricing-rules/${id}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function getConsolePricingCatalog(baseUrl?: string) {
  return apiFetch<ConsolePricingCatalog>({
    path: '/api/console/settings/billing/pricing-catalog',
    baseUrl
  });
}

export function importConsolePricingCatalog(
  catalogIds: string[],
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<{ imported: number; deleted: number }>({
    path: '/api/console/settings/billing/pricing-catalog/import',
    method: 'POST',
    body: { catalog_ids: catalogIds },
    csrfToken,
    baseUrl
  });
}

export function listConsoleCreditAccounts(baseUrl?: string) {
  return apiFetch<ConsoleCreditAccount[]>({
    path: '/api/console/settings/billing/credit-accounts',
    baseUrl
  });
}

export function listConsoleCreditLedger(userId?: string, baseUrl?: string) {
  const query = userId ? `?user_id=${encodeURIComponent(userId)}` : '';
  return apiFetch<ConsoleCreditTransaction[]>({
    path: `/api/console/settings/billing/credit-ledger${query}`,
    baseUrl
  });
}

export function executeConsoleCreditCommand(
  userId: string,
  command: 'grant' | 'charge' | 'adjust' | 'enable' | 'disable' | 'refund',
  input: ConsoleCreditCommandInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleCreditTransaction>({
    path: `/api/console/settings/billing/credits/${userId}/${command}`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}
