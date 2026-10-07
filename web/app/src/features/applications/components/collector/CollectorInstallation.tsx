import { Link } from '@tanstack/react-router';
import { App, Button, Radio, Space, Typography } from 'antd';
import ArrowLeftOutlined from '@ant-design/icons/es/icons/ArrowLeftOutlined';
import CopyOutlined from '@ant-design/icons/es/icons/CopyOutlined';
import CheckOutlined from '@ant-design/icons/es/icons/CheckOutlined';
import SafetyOutlined from '@ant-design/icons/es/icons/SafetyOutlined';
import FileTextOutlined from '@ant-design/icons/es/icons/FileTextOutlined';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { ConsoleApplicationCollector } from '@1flowbase/api-client';
import { copyTextToClipboard } from '../../../../shared/ui/clipboard/copy-text';
import {
  collectorInstallerCommand,
  type CollectorOperatingSystem
} from './installer-command';

export function CollectorInstallation({
  collector,
  applicationId,
  endpoint,
  onBack
}: {
  collector: ConsoleApplicationCollector;
  applicationId: string;
  endpoint: string;
  onBack: () => void;
}) {
  const { t } = useTranslation('applications');
  const { message } = App.useApp();
  const [operatingSystem, setOperatingSystem] =
    useState<CollectorOperatingSystem>('shell');
  const command = collectorInstallerCommand(
    collector,
    operatingSystem,
    endpoint,
    applicationId
  );
  const copyCommand = () => {
    void copyTextToClipboard(command).then(
      () => message.success(t('agent_logs.command_copied')),
      () => message.error(t('agent_logs.command_copy_failed'))
    );
  };
  return (
    <>
      <Button type="text" icon={<ArrowLeftOutlined />} onClick={onBack}>
        {t('agent_logs.back_to_collectors')}
      </Button>
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
              href={collector.documentation_url}
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
              <Typography.Title level={5}>
                {t('agent_logs.prepare_key')}
              </Typography.Title>
              <Typography.Paragraph>
                {t('agent_logs.prepare_key_description')}
              </Typography.Paragraph>
              <Link
                to="/applications/$applicationId/api"
                params={{ applicationId }}
              >
                {t('agent_logs.open_api_keys')}
              </Link>
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
              <div className="application-collector__command">
                <pre>{command}</pre>
                <Button icon={<CopyOutlined />} onClick={copyCommand}>
                  {t('agent_logs.copy_command')}
                </Button>
              </div>
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
              <CheckOutlined />
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
              <SafetyOutlined />
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
              <FileTextOutlined />
              <div>
                <Typography.Title level={5}>
                  {t('agent_logs.open_source')}
                </Typography.Title>
                <Typography.Paragraph>
                  {t('agent_logs.open_source_description')}
                </Typography.Paragraph>
                <Typography.Link
                  href={collector.documentation_url}
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
    </>
  );
}
