import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';
import {
  createConsolePricingRule,
  updateConsolePricingRule,
  type ConsolePricingRuleInput,
  executeConsoleCreditCommand,
  getConsolePricingCatalog,
  importConsolePricingCatalog,
  listConsoleCreditAccounts,
  listConsolePricingRules
} from '../console-billing';

describe('console billing client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('uses canonical pricing and credit routes', async () => {
    await expect(listConsolePricingRules()).resolves.toMatchObject({
      path: '/api/console/settings/billing/pricing-rules?page=1&page_size=20'
    });
    await expect(
      getConsolePricingCatalog({
        provider_code: 'openai',
        upstream_model_id: 'gpt',
        page: 2,
        page_size: 20
      })
    ).resolves.toMatchObject({
      path: '/api/console/settings/billing/pricing-catalog?provider_code=openai&upstream_model_id=gpt&page=2&page_size=20'
    });
    await expect(listConsoleCreditAccounts()).resolves.toMatchObject({
      path: '/api/console/settings/billing/credit-accounts'
    });
    await expect(
      importConsolePricingCatalog(['catalog-rule-1'], 'csrf')
    ).resolves.toMatchObject({
      path: '/api/console/settings/billing/pricing-catalog/import',
      method: 'POST',
      body: { catalog_ids: ['catalog-rule-1'] },
      csrfToken: 'csrf'
    });
  });

  test('AC5 sends four canonical defaults and sparse conditional rules unchanged', async () => {
    const body: ConsolePricingRuleInput = {
      id: 'rule-1',
      provider_code: 'anthropic',
      upstream_model_id: 'fable',
      input_token_unit_size: 1000000,
      input_token_unit_price: '10',
      output_token_unit_size: 1000000,
      output_token_unit_price: '50',
      cache_hit_token_unit_size: 1000000,
      cache_hit_token_unit_price: '0.25',
      cache_write_token_unit_size: 1000000,
      cache_write_token_unit_price: '12.50',
      rules: [
        {
          when: { cache_write_ttl_seconds: 3600 },
          overrides: { cache_write_token_unit_price: '20' }
        }
      ],
      currency_code: 'USD',
      effective_from: '2026-09-01T00:00:00Z',
      effective_to: null,
      timezone: 'UTC',
      weekday_mask: 127,
      local_time_start: null,
      local_time_end: null,
      priority: 0,
      enabled: true,
      source_kind: 'manual',
      source_catalog_id: null,
      source_version: null,
      source_checksum: null,
      extensions: {}
    };
    await expect(createConsolePricingRule(body, 'csrf')).resolves.toEqual({
      path: '/api/console/settings/billing/pricing-rules',
      method: 'POST',
      body,
      csrfToken: 'csrf',
      baseUrl: undefined
    });
    await expect(
      updateConsolePricingRule(body.id, body, 'csrf')
    ).resolves.toEqual({
      path: '/api/console/settings/billing/pricing-rules/rule-1',
      method: 'PATCH',
      body,
      csrfToken: 'csrf',
      baseUrl: undefined
    });
  });

  test('sends a structured idempotent credit command', async () => {
    const body = {
      amount: '2.50',
      reason: 'daily_checkin',
      source_type: 'checkin',
      source_id: '2026-08-17',
      idempotency_key: 'checkin:user-1:2026-08-17'
    };
    await expect(
      executeConsoleCreditCommand('user-1', 'grant', body, 'csrf')
    ).resolves.toMatchObject({
      path: '/api/console/settings/billing/credits/user-1/grant',
      method: 'POST',
      body,
      csrfToken: 'csrf'
    });
  });
});
