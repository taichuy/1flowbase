import { Button, Modal, Progress, Space, Typography } from 'antd';
import { useState } from 'react';

import type {
  SettingsModelProviderCatalogEntry,
  SettingsModelProviderResetCreditCountResult,
  SettingsModelProviderUsageWindowsResult
} from '../../api/model-providers';
import { i18nText } from '../../../../shared/i18n/text';

type AccountOperation = 'usage' | 'count' | 'consume' | null;

type ModelProviderAccountOperationsCardProps = {
  catalogEntry: SettingsModelProviderCatalogEntry;
  usageSnapshot?: SettingsModelProviderUsageWindowsResult | null;
  onUsageSnapshot?: (snapshot: SettingsModelProviderUsageWindowsResult) => void;
  onRefreshUsage?: () => Promise<SettingsModelProviderUsageWindowsResult>;
  onCountResetCredits?: () => Promise<SettingsModelProviderResetCreditCountResult>;
  onConsumeResetCredit?: (input: {
    idempotency_key: string;
  }) => Promise<unknown>;
};

function formatWindowLabel(limitWindowSeconds: number) {
  if (limitWindowSeconds === 18_000) {
    return '5h';
  }

  if (limitWindowSeconds === 604_800) {
    return '7d';
  }

  return `${Math.round(limitWindowSeconds / 3600)}h`;
}

function formatUsedPercent(usedPercent: number) {
  return Number.isInteger(usedPercent)
    ? String(usedPercent)
    : usedPercent.toFixed(1);
}

function newResetCreditIdempotencyKey() {
  return crypto.randomUUID();
}

export function ModelProviderAccountOperationsCard({
  catalogEntry,
  usageSnapshot: suppliedUsageSnapshot,
  onUsageSnapshot,
  onRefreshUsage,
  onCountResetCredits,
  onConsumeResetCredit
}: ModelProviderAccountOperationsCardProps) {
  const supportsUsage =
    catalogEntry.operational_capabilities.includes('usage_windows');
  const supportsResetCredits =
    catalogEntry.operational_capabilities.includes('reset_credits');
  const [fetchedUsageSnapshot, setFetchedUsageSnapshot] =
    useState<SettingsModelProviderUsageWindowsResult | null>(null);
  const [availableCount, setAvailableCount] = useState<number | null>(null);
  const [operation, setOperation] = useState<AccountOperation>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  if (
    (!supportsUsage || !onRefreshUsage) &&
    (!supportsResetCredits || !onCountResetCredits || !onConsumeResetCredit)
  ) {
    return null;
  }

  async function refreshUsage() {
    if (!onRefreshUsage) {
      return false;
    }

    setOperation('usage');
    setErrorMessage(null);
    try {
      const snapshot = await onRefreshUsage();
      setFetchedUsageSnapshot(snapshot);
      onUsageSnapshot?.(snapshot);
      return true;
    } catch (error) {
      setErrorMessage(
        error instanceof Error
          ? error.message
          : i18nText('settings', 'auto.provider_account_operation_failed')
      );
      return false;
    } finally {
      setOperation(null);
    }
  }

  async function refreshResetCreditCount() {
    if (!onCountResetCredits) {
      return false;
    }

    setOperation('count');
    setErrorMessage(null);
    try {
      const result = await onCountResetCredits();
      setAvailableCount(result.available_count);
      return true;
    } catch (error) {
      setErrorMessage(
        error instanceof Error
          ? error.message
          : i18nText('settings', 'auto.provider_account_operation_failed')
      );
      return false;
    } finally {
      setOperation(null);
    }
  }

  function confirmResetCreditConsume() {
    if (!onConsumeResetCredit || !availableCount || operation) {
      return;
    }

    Modal.confirm({
      title: i18nText('settings', 'auto.confirm_reset'),
      content: i18nText(
        'settings',
        'auto.provider_reset_credit_confirm_content'
      ),
      okText: i18nText('settings', 'auto.confirm'),
      cancelText: i18nText('settings', 'auto.cancel'),
      onOk: async () => {
        setOperation('consume');
        setErrorMessage(null);
        try {
          await onConsumeResetCredit({
            idempotency_key: newResetCreditIdempotencyKey()
          });
          await Promise.all([refreshUsage(), refreshResetCreditCount()]);
        } catch (error) {
          setErrorMessage(
            error instanceof Error
              ? error.message
              : i18nText('settings', 'auto.provider_account_operation_failed')
          );
        } finally {
          setOperation(null);
        }
      }
    });
  }

  return (
    <div className="model-provider-drawer__card">
      <div className="model-provider-drawer__card-title">
        <span>{i18nText('settings', 'auto.provider_account_usage')}</span>
      </div>
      <div className="model-provider-drawer__card-body">
        <Space orientation="vertical" size={12} style={{ width: '100%' }}>
          {(suppliedUsageSnapshot ?? fetchedUsageSnapshot)?.windows.map(
            (window) => (
              <div key={window.limit_window_seconds}>
                <Space
                  align="center"
                  style={{ display: 'flex', justifyContent: 'space-between' }}
                >
                  <Typography.Text strong>
                    {formatWindowLabel(window.limit_window_seconds)}
                  </Typography.Text>
                  <Typography.Text type="secondary">
                    {i18nText('settings', 'auto.provider_usage_percent', {
                      value1: formatUsedPercent(window.used_percent)
                    })}
                  </Typography.Text>
                </Space>
                <Progress
                  percent={window.used_percent}
                  showInfo={false}
                  status="normal"
                />
                {window.reset_at ? (
                  <Typography.Text type="secondary">
                    {i18nText('settings', 'auto.provider_usage_resets_at', {
                      value1: window.reset_at
                    })}
                  </Typography.Text>
                ) : null}
              </div>
            )
          )}
          {(suppliedUsageSnapshot ?? fetchedUsageSnapshot)?.windows.length ===
          0 ? (
            <Typography.Text type="secondary">
              {i18nText('settings', 'auto.provider_usage_unavailable')}
            </Typography.Text>
          ) : null}
          <Space wrap>
            {supportsUsage && onRefreshUsage ? (
              <Button
                loading={operation === 'usage'}
                disabled={operation !== null && operation !== 'usage'}
                onClick={() => {
                  void refreshUsage();
                }}
              >
                {i18nText('settings', 'auto.provider_usage_refresh')}
              </Button>
            ) : null}
            {supportsResetCredits && onCountResetCredits ? (
              <Button
                loading={operation === 'count'}
                disabled={operation !== null && operation !== 'count'}
                onClick={() => {
                  void refreshResetCreditCount();
                }}
              >
                {availableCount === null
                  ? i18nText('settings', 'auto.provider_reset_credit_count')
                  : i18nText(
                      'settings',
                      'auto.provider_reset_credit_count_value',
                      {
                        value1: availableCount
                      }
                    )}
              </Button>
            ) : null}
            {supportsResetCredits && onConsumeResetCredit ? (
              <Button
                danger
                loading={operation === 'consume'}
                disabled={!availableCount || operation !== null}
                onClick={confirmResetCreditConsume}
              >
                {i18nText('settings', 'auto.reset')}
              </Button>
            ) : null}
          </Space>
          {errorMessage ? (
            <Typography.Text type="danger">{errorMessage}</Typography.Text>
          ) : null}
        </Space>
      </div>
    </div>
  );
}
