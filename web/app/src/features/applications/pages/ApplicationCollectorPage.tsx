import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  getConsoleExtensionRiskChallenge,
  type ConsoleApplicationCollector,
  type ConsoleExtensionCompatibilityOverride,
  type ConsoleExtensionRiskOverride
} from '@1flowbase/api-client';
import {
  Alert,
  App,
  Button,
  Empty,
  Skeleton,
  Space,
  Tabs,
  Tag,
  Typography
} from 'antd';
import { useTranslation } from 'react-i18next';
import { useAuthStore } from '../../../state/auth-store';
import {
  applicationCatalogQueryKey,
  fetchApplicationCatalog,
  getApplicationsApiBaseUrl,
  installApplicationCollector
} from '../api/applications';
import { CollectorInstallation } from '../components/collector/CollectorInstallation';
import '../components/collector/application-collector.css';

type InstallOperation = {
  collector: ConsoleApplicationCollector;
  update: boolean;
  risk_override?: ConsoleExtensionRiskOverride;
  compatibility_override?: ConsoleExtensionCompatibilityOverride;
};

export function ApplicationCollectorPage({
  applicationId
}: {
  applicationId: string;
}) {
  const { t } = useTranslation('applications');
  const { modal } = App.useApp();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [catalogId, setCatalogId] = useState<string | null>(null);
  const [installError, setInstallError] = useState(false);
  const catalog = useQuery({
    queryKey: applicationCatalogQueryKey,
    queryFn: fetchApplicationCatalog,
    retry: false
  });
  const apiBaseUrl = new URL(
    getApplicationsApiBaseUrl() || window.location.origin,
    window.location.origin
  ).href.replace(/\/$/, '');
  const endpoint = `${apiBaseUrl}/api/logs/v1/events`;
  const collectors = catalog.data?.collectors;
  const selected = collectors?.find(
    (collector) => collector.catalog_id === catalogId
  );
  const visibleCollectors = catalogId
    ? collectors?.filter((collector) => collector.catalog_id === catalogId)
    : collectors;
  // Tabs also emits its keys as DOM IDs; catalog IDs contain ':' and '/'.
  const tabs =
    collectors?.map((collector, index) => ({
      key: `collector-${index}`,
      catalog_id: collector.catalog_id,
      label: collector.display_name
    })) ?? [];
  const install = useMutation({
    mutationFn: async (operation: InstallOperation) => {
      if (!csrfToken) throw new Error('authenticated session required');
      return installApplicationCollector(
        {
          category: operation.collector.category,
          catalog_id: operation.collector.catalog_id,
          version: operation.collector.version,
          risk_override: operation.risk_override,
          compatibility_override: operation.compatibility_override
        },
        csrfToken,
        operation.update
      );
    },
    onMutate: () => setInstallError(false),
    onSuccess: async (_, operation) => {
      await queryClient.invalidateQueries({
        queryKey: applicationCatalogQueryKey
      });
      await queryClient.invalidateQueries({
        queryKey: ['settings', 'extension-center']
      });
      setCatalogId(operation.collector.catalog_id);
    },
    onError: (error, operation) => {
      const challenge = getConsoleExtensionRiskChallenge(error);
      if (
        !challenge ||
        operation.risk_override ||
        operation.compatibility_override
      ) {
        setInstallError(true);
        return;
      }
      const allowed = challenge.warnings.every(
        (warning) => warning.overridable
      );
      modal.confirm({
        title: t('agent_logs.install_confirmation'),
        content: (
          <Space orientation="vertical">
            {challenge.warnings.map((warning) => (
              <Typography.Text key={warning.code}>
                {warning.message}
              </Typography.Text>
            ))}
            {challenge.compatibility && (
              <Typography.Paragraph>
                {t('agent_logs.host_requirement', {
                  version: challenge.compatibility.minimum_host_version
                })}
              </Typography.Paragraph>
            )}
          </Space>
        ),
        okText: t('agent_logs.confirm_platform_install'),
        okButtonProps: { disabled: !allowed },
        onOk: () =>
          install.mutateAsync({
            ...operation,
            ...(challenge.warnings.length
              ? {
                  risk_override: {
                    reason: 'user_confirmed',
                    acknowledged_warnings: challenge.warnings.map(
                      (warning) => warning.code
                    )
                  }
                }
              : {}),
            ...(challenge.compatibility
              ? {
                  compatibility_override: {
                    reason: challenge.compatibility.reason,
                    acknowledged_current_host_version:
                      challenge.compatibility.current_host_version,
                    acknowledged_minimum_host_version:
                      challenge.compatibility.minimum_host_version
                  }
                }
              : {})
          })
      });
    }
  });
  const statusLabel = (collector: ConsoleApplicationCollector) => {
    switch (collector.installation_status) {
      case 'installed':
        return t('agent_logs.platform_installed');
      case 'missing':
        return t('agent_logs.platform_missing');
      case 'not_installed':
        return t('agent_logs.platform_not_installed');
    }
  };
  return (
    <div className="application-collector">
      <Typography.Title level={4}>{t('agent_logs.collector')}</Typography.Title>
      <Typography.Paragraph type="secondary">
        {t('agent_logs.collector_description')}
      </Typography.Paragraph>
      <Tabs
        activeKey={
          tabs.find((tab) => tab.catalog_id === catalogId)?.key ?? 'all'
        }
        onChange={(key) =>
          setCatalogId(tabs.find((tab) => tab.key === key)?.catalog_id ?? null)
        }
        items={[
          { key: 'all', label: t('auto.all') },
          ...tabs.map(({ key, label }) => ({ key, label }))
        ]}
      />
      {installError && (
        <Alert
          type="error"
          showIcon
          title={t('agent_logs.platform_install_failed')}
        />
      )}
      {catalog.isPending ? (
        <div role="status" aria-label={t('agent_logs.collectors_loading')}>
          <Skeleton active />
        </div>
      ) : catalog.isError ? (
        <Alert
          type="error"
          showIcon
          title={t('agent_logs.collectors_error')}
          action={
            <Button onClick={() => void catalog.refetch()}>
              {t('agent_logs.retry_collectors')}
            </Button>
          }
        />
      ) : selected?.installation_status === 'installed' ? (
        <CollectorInstallation
          collector={selected}
          applicationId={applicationId}
          endpoint={endpoint}
          apiBaseUrl={apiBaseUrl}
          onBack={() => setCatalogId(null)}
        />
      ) : (
        visibleCollectors &&
        (visibleCollectors.length === 0 ? (
          <Empty description={t('agent_logs.no_collectors')} />
        ) : (
          <div className="application-collector__catalog">
            {visibleCollectors.map((collector) => {
              const updating = !!collector.installed_version;
              const busy =
                install.isPending &&
                install.variables.collector.catalog_id === collector.catalog_id;
              return (
                <article
                  key={collector.catalog_id}
                  className="application-collector__card"
                >
                  <div className="application-collector__identity">
                    <span
                      className="application-collector__logo"
                      aria-hidden="true"
                    >
                      {collector.display_name.slice(0, 1)}
                    </span>
                    <div>
                      <Typography.Title level={5}>
                        {collector.display_name}
                      </Typography.Title>
                      <Typography.Text type="secondary">
                        {t('agent_logs.collector_version', {
                          version: collector.version
                        })}
                      </Typography.Text>
                    </div>
                    <Tag>{statusLabel(collector)}</Tag>
                  </div>
                  <Typography.Paragraph>
                    {collector.description}
                  </Typography.Paragraph>
                  <Typography.Paragraph type="secondary">
                    {t('agent_logs.platform_status_description')}
                  </Typography.Paragraph>
                  {collector.installation_status === 'missing' && (
                    <Alert
                      type="warning"
                      showIcon
                      title={t('agent_logs.package_unavailable')}
                    />
                  )}
                  {collector.installation_status === 'installed' ? (
                    <Space
                      orientation="vertical"
                      className="application-collector__actions"
                    >
                      <Button
                        type="primary"
                        block
                        onClick={() => setCatalogId(collector.catalog_id)}
                      >
                        {t('agent_logs.download_cli')}
                      </Button>
                      {collector.installable &&
                        collector.version !== collector.installed_version && (
                          <Button
                            block
                            loading={busy}
                            disabled={
                              !collector.can_update ||
                              !csrfToken ||
                              install.isPending
                            }
                            onClick={() =>
                              install.mutate({ collector, update: true })
                            }
                          >
                            {t('agent_logs.update_platform_package')}
                          </Button>
                        )}
                    </Space>
                  ) : (
                    collector.installation_status === 'not_installed' && (
                      <Button
                        type="primary"
                        block
                        loading={busy}
                        disabled={
                          !collector.installable ||
                          !collector.can_install ||
                          !csrfToken ||
                          install.isPending
                        }
                        onClick={() =>
                          install.mutate({ collector, update: updating })
                        }
                      >
                        {t('agent_logs.install_collector')}
                      </Button>
                    )
                  )}
                  {collector.installation_status !== 'installed' &&
                    !collector.can_install && (
                      <Typography.Paragraph type="secondary">
                        {t('agent_logs.install_permission_required')}
                      </Typography.Paragraph>
                    )}
                </article>
              );
            })}
          </div>
        ))
      )}
    </div>
  );
}
