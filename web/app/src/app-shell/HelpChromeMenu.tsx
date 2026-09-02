import CloudDownloadOutlined from '@ant-design/icons/es/icons/CloudDownloadOutlined';
import ExportOutlined from '@ant-design/icons/es/icons/ExportOutlined';
import FileTextOutlined from '@ant-design/icons/es/icons/FileTextOutlined';
import GithubOutlined from '@ant-design/icons/es/icons/GithubOutlined';
import QuestionCircleOutlined from '@ant-design/icons/es/icons/QuestionCircleOutlined';
import SafetyCertificateOutlined from '@ant-design/icons/es/icons/SafetyCertificateOutlined';
import { fetchConsoleReleaseStatus } from '@1flowbase/api-client';
import { useQuery } from '@tanstack/react-query';
import { useState, type ReactNode } from 'react';
import { Menu, Spin } from 'antd';
import { i18nText } from '../shared/i18n/text';

const HELP_LINKS = [
  {
    key: 'github',
    label: 'github',
    icon: <GithubOutlined />,
    href: 'https://github.com/taichuy/1flowbase'
  },
  {
    key: 'docs',
    label: i18nText('appShell', 'auto.documentation'),
    icon: <FileTextOutlined />,
    href: 'https://1flowbase.taichuy.com/docs/'
  },
  {
    key: 'license',
    label: i18nText('appShell', 'auto.license_agreement'),
    icon: <SafetyCertificateOutlined />,
    href: 'https://1flowbase.taichuy.com/license/'
  }
] satisfies Array<{
  key: string;
  label: string;
  icon: ReactNode;
  href: string;
}>;

function HelpPopupLink({
  href,
  icon,
  label
}: {
  href: string;
  icon: ReactNode;
  label: ReactNode;
}) {
  return (
    <a
      className="app-shell-help-popup__link"
      href={href}
      target="_blank"
      rel="noreferrer"
    >
      <span className="app-shell-help-popup__link-icon">{icon}</span>
      <span>{label}</span>
    </a>
  );
}

function HelpPopupText({ icon, label }: { icon: ReactNode; label: ReactNode }) {
  return (
    <span className="app-shell-help-popup__link">
      <span className="app-shell-help-popup__link-icon">{icon}</span>
      <span>{label}</span>
    </span>
  );
}

function formatReleaseVersion(version: string | null | undefined) {
  const normalizedVersion = version?.trim();
  if (!normalizedVersion) {
    return null;
  }
  return normalizedVersion.startsWith('v')
    ? normalizedVersion
    : `v${normalizedVersion}`;
}

export function HelpChromeMenu() {
  const [helpOpen, setHelpOpen] = useState(false);
  const releaseStatusQuery = useQuery({
    queryKey: ['console-release-status'],
    queryFn: () => fetchConsoleReleaseStatus(),
    staleTime: 60 * 1000,
    refetchOnMount: 'always',
    refetchOnWindowFocus: true,
    retry: false,
    enabled: helpOpen
  });
  const releaseStatus = releaseStatusQuery.data;
  const currentVersion = formatReleaseVersion(releaseStatus?.current_version);
  const latestVersion =
    releaseStatus?.has_update && releaseStatus.latest_version
      ? formatReleaseVersion(releaseStatus.latest_version)
      : null;
  const latestReleaseUrl = releaseStatus?.release_info?.html_url ?? null;

  return (
    <Menu
      className="app-shell-help-menu"
      mode="horizontal"
      openKeys={helpOpen ? ['help'] : []}
      onOpenChange={(openKeys) => setHelpOpen(openKeys.includes('help'))}
      selectable={false}
      items={[
        {
          key: 'help',
          label: (
            <span
              className="app-shell-help-block"
              aria-label={i18nText('appShell', 'auto.help')}
            >
              <QuestionCircleOutlined />
            </span>
          ),
          popupClassName: 'app-shell-help-popup',
          children: [
            ...HELP_LINKS.map((item) => ({
              key: item.key,
              label: (
                <HelpPopupLink
                  href={item.href}
                  icon={item.icon}
                  label={item.label}
                />
              )
            })),
            {
              key: 'release-current',
              label: (
                <HelpPopupText
                  icon={<CloudDownloadOutlined />}
                  label={currentVersion ?? <Spin size="small" />}
                />
              )
            },
            ...(latestVersion && latestReleaseUrl
              ? [
                  {
                    key: 'release-latest',
                    label: (
                      <HelpPopupLink
                        href={latestReleaseUrl}
                        icon={<ExportOutlined />}
                        label={latestVersion}
                      />
                    )
                  }
                ]
              : [])
          ]
        }
      ]}
      disabledOverflow
    />
  );
}
