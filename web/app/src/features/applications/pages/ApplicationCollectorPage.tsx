import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Alert, Button, Empty, Skeleton, Tabs, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import {
  applicationCatalogQueryKey,
  fetchApplicationCatalog,
  getApplicationsApiBaseUrl
} from '../api/applications';
import { CollectorInstallation } from '../components/collector/CollectorInstallation';
import '../components/collector/application-collector.css';

export function ApplicationCollectorPage({
  applicationId
}: {
  applicationId: string;
}) {
  const { t } = useTranslation('applications');
  const [sourceClient, setSourceClient] = useState('all');
  const [collectorCode, setCollectorCode] = useState<string | null>(null);
  const catalog = useQuery({
    queryKey: applicationCatalogQueryKey,
    queryFn: fetchApplicationCatalog,
    retry: false
  });
  const endpoint = new URL(
    `${getApplicationsApiBaseUrl().replace(/\/$/, '')}/api/logs/v1/events`,
    window.location.origin
  ).href;
  const collectors = catalog.data?.collectors;
  const selected = collectors?.find(
    (collector) => collector.collector_code === collectorCode
  );
  return (
    <div className="application-collector">
      <Typography.Title level={4}>{t('agent_logs.collector')}</Typography.Title>
      <Typography.Paragraph type="secondary">
        {t('agent_logs.collector_description')}
      </Typography.Paragraph>
      <Tabs
        activeKey={sourceClient}
        onChange={(key) => {
          setSourceClient(key);
          setCollectorCode(null);
        }}
        items={[
          { key: 'all', label: t('auto.all') },
          { key: 'codex', label: 'Codex' }
        ]}
      />
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
      ) : collectors && selected ? (
        <CollectorInstallation
          collector={selected}
          applicationId={applicationId}
          endpoint={endpoint}
          onBack={() => setCollectorCode(null)}
        />
      ) : (
        collectors && (
          <>
            {collectors.filter(
              (collector) =>
                sourceClient === 'all' ||
                collector.source_client === sourceClient
            ).length === 0 ? (
              <Empty description={t('agent_logs.no_collectors')} />
            ) : (
              <div className="application-collector__catalog">
                {collectors
                  .filter(
                    (collector) =>
                      sourceClient === 'all' ||
                      collector.source_client === sourceClient
                  )
                  .map((collector) => (
                    <article
                      key={collector.collector_code}
                      className="application-collector__card"
                    >
                      <div className="application-collector__identity">
                        <span
                          className="application-collector__logo"
                          aria-hidden="true"
                        >
                          C
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
                      </div>
                      <Typography.Paragraph>
                        {collector.description}
                      </Typography.Paragraph>
                      <Button
                        type="primary"
                        block
                        onClick={() =>
                          setCollectorCode(collector.collector_code)
                        }
                      >
                        {t('agent_logs.install_collector')}
                      </Button>
                    </article>
                  ))}
              </div>
            )}
          </>
        )
      )}
    </div>
  );
}
