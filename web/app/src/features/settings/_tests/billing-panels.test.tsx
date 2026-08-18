import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const billingApi = vi.hoisted(() => ({
  settingsPricingRulesQueryKey: ['settings', 'billing', 'pricing-rules'],
  settingsPricingCatalogQueryKey: vi.fn((filter) => [
    'settings',
    'billing',
    'pricing-catalog',
    filter
  ]),
  settingsCreditAccountsQueryKey: ['settings', 'billing', 'credit-accounts'],
  settingsCreditLedgerQueryKey: vi.fn((userId?: string) => [
    'settings',
    'billing',
    'credit-ledger',
    userId ?? 'all'
  ]),
  listSettingsPricingRules: vi.fn(),
  createSettingsPricingRule: vi.fn(),
  updateSettingsPricingRule: vi.fn(),
  deleteSettingsPricingRule: vi.fn(),
  getSettingsPricingCatalog: vi.fn(),
  importSettingsPricingCatalog: vi.fn(),
  listSettingsCreditAccounts: vi.fn(),
  listSettingsCreditLedger: vi.fn(),
  executeSettingsCreditCommand: vi.fn()
}));

const membersApi = vi.hoisted(() => ({
  settingsMembersQueryKey: ['settings', 'members'],
  fetchSettingsMembers: vi.fn()
}));

vi.mock('../api/billing', () => billingApi);
vi.mock('../api/members', () => membersApi);

import { AppProviders } from '../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { CreditManagementPanel } from '../components/billing/CreditManagementPanel';
import { PricingCatalogPanel } from '../components/billing/PricingCatalogPanel';
import { PricingRulesPanel } from '../components/billing/PricingRulesPanel';

function authenticate() {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'root-user',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'root-user',
      account: 'root',
      email: 'root@example.com',
      phone: null,
      nickname: 'Root',
      name: 'Root',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'root',
      permissions: []
    }
  });
}

function renderWithProviders(node: ReactNode) {
  return render(<AppProviders>{node}</AppProviders>);
}

describe('billing settings panels', () => {
  beforeEach(() => {
    resetAuthStore();
    authenticate();
    billingApi.listSettingsPricingRules.mockResolvedValue({
      items: [
        {
          id: 'rule-1',
          provider_code: 'openai',
          upstream_model_id: 'gpt-test',
          input_token_unit_size: 1_000,
          input_token_unit_price: '0.000000000000000000',
          output_token_unit_size: 1_000_000,
          output_token_unit_price: '5',
          cache_hit_token_unit_size: 1_000_000_000,
          cache_hit_token_unit_price: '0.000001000000000000',
          currency_code: 'USD',
          effective_from: '2026-01-01T00:00:00Z',
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
          extensions: {},
          created_by: 'root-user',
          created_at: '2026-08-17T00:00:00Z',
          updated_at: '2026-08-17T00:00:00Z'
        }
      ],
      total_count: 1,
      page: 1,
      page_size: 20
    });
    billingApi.getSettingsPricingCatalog.mockResolvedValue({
      schema_version: '1flowbase.model-pricing-page/v1',
      catalog_version: '2026-08-18.1',
      currency_code: 'USD',
      items: [
        {
          id: '10000000-0000-4000-8000-000000000001',
          provider_code: 'zero',
          upstream_model_id: 'any',
          input_token_unit_size: 1_000_000,
          input_token_unit_price: '0',
          output_token_unit_size: 1_000_000,
          output_token_unit_price: '0',
          cache_hit_token_unit_size: 1_000_000,
          cache_hit_token_unit_price: '0',
          currency_code: 'USD',
          effective_from: '2026-08-17T00:00:00Z',
          effective_to: null,
          timezone: 'UTC',
          weekday_mask: 127,
          local_time_start: null,
          local_time_end: null,
          priority: 0,
          enabled: true,
          source_kind: 'official',
          source_catalog_id: '10000000-0000-4000-8000-000000000001',
          source_version: '2026-08-18.1',
          source_checksum: `sha256:${'a'.repeat(64)}`,
          extensions: {}
        }
      ],
      total_count: 1,
      page: 1,
      page_size: 20
    });
    billingApi.importSettingsPricingCatalog.mockResolvedValue({
      imported: 1,
      deleted: 0
    });
    membersApi.fetchSettingsMembers.mockResolvedValue([
      { id: 'user-1', account: 'member', name: 'Member One' }
    ]);
    billingApi.listSettingsCreditAccounts.mockResolvedValue([
      {
        id: 'account-1',
        workspace_id: 'workspace-1',
        user_id: 'user-1',
        credit_unit: 'USD',
        charge_enabled: true,
        current_balance: '5.000000000000000000',
        reserved_amount: '1.000000000000000000',
        available_balance: '4.000000000000000000',
        credit_insufficient: false,
        revision: 1,
        created_at: '2026-08-17T00:00:00Z',
        updated_at: '2026-08-17T00:00:00Z'
      }
    ]);
    billingApi.listSettingsCreditLedger.mockResolvedValue([]);
    billingApi.executeSettingsCreditCommand.mockResolvedValue({});
  });

  test('shows fixed pricing columns and opens the validated rule editor', async () => {
    renderWithProviders(<PricingRulesPanel canManage />);
    expect(await screen.findByText('gpt-test')).toBeInTheDocument();
    expect(screen.getByText('1K / 0$')).toBeInTheDocument();
    expect(screen.getByText('1M / 5.00$')).toBeInTheDocument();
    expect(screen.getByText('1B / 0.000001$')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /新增计费规则/ }));
    expect(await screen.findByRole('dialog')).toBeInTheDocument();
    expect(screen.getByLabelText('输入 Token 单位')).toBeInTheDocument();
    expect(screen.getByLabelText('缓存命中单价')).toBeInTheDocument();
  });

  test('submits pricing filters through the shared server-paginated table', async () => {
    renderWithProviders(<PricingRulesPanel canManage />);
    await screen.findByText('gpt-test');
    fireEvent.change(screen.getByLabelText('上游模型 ID'), {
      target: { value: 'gpt-5' }
    });
    fireEvent.click(screen.getByRole('button', { name: /筛\s*选/ }));
    await waitFor(() =>
      expect(billingApi.listSettingsPricingRules).toHaveBeenLastCalledWith(
        expect.objectContaining({
          upstream_model_id: 'gpt-5',
          page: 1,
          page_size: 20
        })
      )
    );
  });

  test('loads and filters the official catalog through backend pagination', async () => {
    renderWithProviders(<PricingCatalogPanel />);
    expect(await screen.findByText('zero')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /更\s*新/ }));
    await waitFor(() =>
      expect(billingApi.importSettingsPricingCatalog).toHaveBeenCalledWith(
        ['10000000-0000-4000-8000-000000000001'],
        'csrf-123'
      )
    );
    fireEvent.change(screen.getByLabelText('厂家 Code'), {
      target: { value: 'openai' }
    });
    fireEvent.change(screen.getByLabelText('上游模型 ID'), {
      target: { value: 'gpt' }
    });
    fireEvent.click(screen.getByRole('button', { name: /筛\s*选/ }));
    await waitFor(() =>
      expect(billingApi.getSettingsPricingCatalog).toHaveBeenLastCalledWith({
        provider_code: 'openai',
        upstream_model_id: 'gpt',
        page: 1,
        page_size: 20
      })
    );
  });

  test('uses backend available balance and submits a credit command', async () => {
    renderWithProviders(<CreditManagementPanel canManage />);
    expect(await screen.findByText('$4.00')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '增加额度' }));
    const dialog = await screen.findByRole('dialog');
    const amount = within(dialog).getByLabelText('金额（USD）');
    fireEvent.change(amount, { target: { value: '2.50' } });
    fireEvent.change(within(dialog).getByLabelText('原因'), {
      target: { value: 'test grant' }
    });
    fireEvent.click(screen.getByRole('button', { name: '确 定' }));
    await waitFor(() =>
      expect(billingApi.executeSettingsCreditCommand).toHaveBeenCalledWith(
        'user-1',
        'grant',
        expect.objectContaining({ amount: '2.50' }),
        'csrf-123'
      )
    );
  });
});
