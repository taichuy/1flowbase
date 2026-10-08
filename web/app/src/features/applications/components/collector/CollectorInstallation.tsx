import { useQueryClient } from '@tanstack/react-query';
import { Alert, App, Button, Input, Radio, Space, Typography } from 'antd';
import CopyOutlined from '@ant-design/icons/es/icons/CopyOutlined';
import CheckOutlined from '@ant-design/icons/es/icons/CheckOutlined';
import SafetyOutlined from '@ant-design/icons/es/icons/SafetyOutlined';
import FileTextOutlined from '@ant-design/icons/es/icons/FileTextOutlined';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { ConsoleApplicationCollector } from '@1flowbase/api-client';
import { copyTextToClipboard } from '../../../../shared/ui/clipboard/copy-text';
import { useAuthStore } from '../../../../state/auth-store';
import { applicationDetailQueryKey } from '../../api/applications';
import {
  applicationApiKeysQueryKey,
  createApplicationApiKey
} from '../../api/public-api';
import {
  collectorInstallerCommand,
  type CollectorOperatingSystem
} from './installer-command';

export function CollectorInstallation({
  collector,
  applicationId,
  endpoint,
  apiBaseUrl
}: {
  collector: ConsoleApplicationCollector;
  applicationId: string;
  endpoint: string;
  apiBaseUrl: string;
}) {
  const { t } = useTranslation('applications');
  const { message } = App.useApp();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [apiKey, setApiKey] = useState('');
  const [generatingKey, setGeneratingKey] = useState(false);
  const [keyError, setKeyError] = useState(false);
  const [operatingSystem, setOperatingSystem] =
    useState<CollectorOperatingSystem>('shell');
  const command = collectorInstallerCommand(
    collector,
    operatingSystem,
    endpoint,
    applicationId,
    apiBaseUrl,
    apiKey
  );
  const previewCommand = collectorInstallerCommand(
    collector,
    operatingSystem,
    endpoint,
    applicationId,
    apiBaseUrl,
    apiKey ? '********' : ''
  );
  const documentationUrl = collector.documentation_url
    ? `${apiBaseUrl.replace(/\/$/, '')}${collector.documentation_url}`
    : undefined;
  const copyCommand = () => {
    if (!command) return;
    void copyTextToClipboard(command).then(
      () => message.success(t('agent_logs.command_copied')),
      () => message.error(t('agent_logs.command_copy_failed'))
    );
  };
  const generateKey = async () => {
    if (!csrfToken || generatingKey) return;
    setGeneratingKey(true);
    setKeyError(false);
    try {
      // Keep the returned token in component state, outside query/mutation caches.
      const key = await createApplicationApiKey(
        applicationId,
        t('agent_logs.collector_key_name', { client: collector.display_name }),
        csrfToken
      );
      setApiKey(key.token);
      void queryClient.invalidateQueries({
        queryKey: applicationApiKeysQueryKey(applicationId)
      });
      void queryClient.invalidateQueries({
        queryKey: applicationDetailQueryKey(applicationId)
      });
    } catch {
      setKeyError(true);
    } finally {
      setGeneratingKey(false);
    }
  };
  return (
    <div className="application-collector__installation">
      <section
        className="application-collector__steps"
        aria-label={t('agent_logs.installation_steps')}
      >
        <div className="application-collector__identity">
          <span className="application-collector__logo" aria-hidden="true">
            C
          </span>
          <div>
            <Typography.Title level={4}>
              {collector.display_name}
            </Typography.Title>
            <Typography.Text type="secondary">
              {collector.description}
            </Typography.Text>
          </div>
          <Typography.Link
            href={documentationUrl}
            target="_blank"
            rel="noreferrer"
          >
            {t('agent_logs.documentation')}
          </Typography.Link>
        </div>
        <section className="application-collector__step">
          <span className="application-collector__number" aria-hidden="true">
            1
          </span>
          <div>
            <div className="application-collector__key-header">
              <Typography.Title level={5}>
                {t('agent_logs.prepare_key')}
              </Typography.Title>
              <Button
                type="primary"
                loading={generatingKey}
                disabled={!csrfToken || generatingKey}
                onClick={() => void generateKey()}
              >
                {t('agent_logs.generate_key')}
              </Button>
            </div>
            <Typography.Paragraph>
              {t('agent_logs.prepare_key_description')}
            </Typography.Paragraph>
            <Input.Password
              aria-label={t('auto.api_key')}
              placeholder={t('agent_logs.enter_application_key')}
              autoComplete="off"
              value={apiKey}
              onChange={(event) => setApiKey(event.target.value)}
            />
            {keyError && (
              <Alert
                className="application-collector__key-error"
                type="error"
                showIcon
                title={t('agent_logs.key_generation_failed')}
              />
            )}
          </div>
        </section>
        <section className="application-collector__step">
          <span className="application-collector__number" aria-hidden="true">
            2
          </span>
          <div>
            <Typography.Title level={5}>
              {t('agent_logs.install_command')}
            </Typography.Title>
            <Typography.Paragraph>
              {t('agent_logs.install_command_description')}
            </Typography.Paragraph>
            <Radio.Group
              value={operatingSystem}
              onChange={(event) => setOperatingSystem(event.target.value)}
              optionType="button"
              buttonStyle="solid"
              aria-label={t('agent_logs.operating_system')}
            >
              <Radio.Button value="shell">macOS / Linux (Shell)</Radio.Button>
              <Radio.Button value="powershell">
                Windows (PowerShell)
              </Radio.Button>
            </Radio.Group>
            {command ? (
              <div className="application-collector__command">
                <pre>{previewCommand}</pre>
                <Button
                  icon={<CopyOutlined aria-hidden="true" />}
                  onClick={copyCommand}
                >
                  {t('agent_logs.copy_command')}
                </Button>
              </div>
            ) : (
              <Alert
                type="warning"
                showIcon
                title={t('agent_logs.package_unavailable')}
              />
            )}
          </div>
        </section>
        <section className="application-collector__step">
          <span className="application-collector__number" aria-hidden="true">
            3
          </span>
          <div>
            <Typography.Title level={5}>
              {t('agent_logs.start_collecting')}
            </Typography.Title>
            <Typography.Paragraph>
              {t('agent_logs.start_collecting_description')}
            </Typography.Paragraph>
          </div>
        </section>
      </section>
      <aside
        className="application-collector__benefits"
        aria-label={t('agent_logs.collector_benefits')}
      >
        <Space orientation="vertical" size={24}>
          <div className="application-collector__benefit">
            <CheckOutlined aria-hidden="true" />
            <div>
              <Typography.Title level={5}>
                {t('agent_logs.automatic_collection')}
              </Typography.Title>
              <Typography.Paragraph>
                {t('agent_logs.automatic_collection_description')}
              </Typography.Paragraph>
            </div>
          </div>
          <div className="application-collector__benefit">
            <SafetyOutlined aria-hidden="true" />
            <div>
              <Typography.Title level={5}>
                {t('agent_logs.local_key')}
              </Typography.Title>
              <Typography.Paragraph>
                {t('agent_logs.local_key_description')}
              </Typography.Paragraph>
            </div>
          </div>
          <div className="application-collector__benefit">
            <FileTextOutlined aria-hidden="true" />
            <div>
              <Typography.Title level={5}>
                {t('agent_logs.open_source')}
              </Typography.Title>
              <Typography.Paragraph>
                {t('agent_logs.open_source_description')}
              </Typography.Paragraph>
              <Typography.Link
                href={documentationUrl}
                target="_blank"
                rel="noreferrer"
              >
                {t('agent_logs.documentation')}
              </Typography.Link>
            </div>
          </div>
        </Space>
      </aside>
    </div>
  );
}
