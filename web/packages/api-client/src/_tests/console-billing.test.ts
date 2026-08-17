import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';
import {
  executeConsoleCreditCommand,
  listConsoleCreditAccounts,
  listConsolePricingRules
} from '../console-billing';

describe('console billing client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(async (input) => input as never);

  test('uses canonical pricing and credit routes', async () => {
    await expect(listConsolePricingRules()).resolves.toMatchObject({
      path: '/api/console/settings/billing/pricing-rules'
    });
    await expect(listConsoleCreditAccounts()).resolves.toMatchObject({
      path: '/api/console/settings/billing/credit-accounts'
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
