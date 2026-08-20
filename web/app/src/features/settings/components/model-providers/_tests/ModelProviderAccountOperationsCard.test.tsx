import { Modal } from 'antd';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, test, vi } from 'vitest';

import { modelProviderCatalogEntries } from '../../../../../test/model-provider-contract-fixtures';
import { ModelProviderAccountOperationsCard } from '../ModelProviderAccountOperationsCard';

const accountOperationCatalogEntry = {
  ...modelProviderCatalogEntries[0],
  operational_capabilities: ['usage_windows', 'reset_credits']
};

describe('ModelProviderAccountOperationsCard', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  test('AC-005～007 refreshes only successful snapshots and consumes one reset credit after confirmation', async () => {
    const usage = vi
      .fn()
      .mockResolvedValueOnce({
        queried_at: '2026-08-20T10:00:00Z',
        windows: [
          {
            limit_window_seconds: 18_000,
            used_percent: 42,
            reset_at: '2026-08-20T13:00:00Z'
          },
          {
            limit_window_seconds: 604_800,
            used_percent: 61,
            reset_at: null
          }
        ]
      })
      .mockRejectedValueOnce(new Error('provider request timed out'))
      .mockResolvedValue({
        queried_at: '2026-08-20T10:05:00Z',
        windows: [
          {
            limit_window_seconds: 18_000,
            used_percent: 7,
            reset_at: '2026-08-20T13:00:00Z'
          }
        ]
      });
    const count = vi
      .fn()
      .mockResolvedValueOnce({ available_count: 2 })
      .mockResolvedValue({ available_count: 1 });
    const consume = vi.fn().mockResolvedValue({ consumed: true });
    const confirm = vi
      .spyOn(Modal, 'confirm')
      .mockImplementation(() => ({ destroy: vi.fn() }) as never);

    render(
      <ModelProviderAccountOperationsCard
        catalogEntry={accountOperationCatalogEntry}
        onRefreshUsage={usage}
        onCountResetCredits={count}
        onConsumeResetCredit={consume}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /查询|Refresh/ }));
    await screen.findByText('5h');
    expect(screen.getByText(/42%/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /查询|Refresh/ }));
    await screen.findByText('provider request timed out');
    expect(screen.getByText(/42%/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /次数|Credits/ }));
    await screen.findByRole('button', { name: /次数 2|Credits 2/ });
    expect(screen.getByRole('button', { name: /重置|Reset/ })).toBeEnabled();

    fireEvent.click(screen.getByRole('button', { name: /重置|Reset/ }));
    const confirmOptions = confirm.mock.calls[0]?.[0] as {
      onOk?: () => Promise<void>;
    };
    await confirmOptions.onOk?.();

    await waitFor(() => {
      expect(consume).toHaveBeenCalledTimes(1);
      expect(consume).toHaveBeenCalledWith({
        idempotency_key: expect.any(String)
      });
      expect(usage).toHaveBeenCalledTimes(3);
      expect(count).toHaveBeenCalledTimes(2);
    });
  });
});
