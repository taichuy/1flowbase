import { SettingOutlined } from '@ant-design/icons';
import { Link } from '@tanstack/react-router';
import type { MenuProps } from 'antd';

import type { SettingsSectionRegistryItem } from '../features/settings/lib/settings-sections';
import { i18nText } from '../shared/i18n/text';

type SettingsChromeMenuRegistryState = 'loading' | 'error' | 'ready';

function isCurrentSettingsSection(
  pathname: string,
  section: SettingsSectionRegistryItem
) {
  return pathname === section.to || pathname.startsWith(`${section.to}/`);
}

function renderSettingsChromeLink({
  section,
  pathname,
  useRouterLinks
}: {
  section: SettingsSectionRegistryItem;
  pathname: string;
  useRouterLinks: boolean;
}) {
  const isCurrent = isCurrentSettingsSection(pathname, section);

  if (useRouterLinks) {
    return (
      <Link
        className="app-shell-settings-popup__link"
        to={section.to}
        aria-current={isCurrent ? 'page' : undefined}
      >
        {i18nText('settings', section.label_key)}
      </Link>
    );
  }

  return (
    <a
      className="app-shell-settings-popup__link"
      href={section.to}
      aria-current={isCurrent ? 'page' : undefined}
    >
      {i18nText('settings', section.label_key)}
    </a>
  );
}

export function createSettingsChromeMenuItems({
  pathname,
  useRouterLinks,
  sections,
  registryState = 'ready'
}: {
  pathname: string;
  useRouterLinks: boolean;
  sections: SettingsSectionRegistryItem[];
  registryState?: SettingsChromeMenuRegistryState;
}): MenuProps['items'] {
  const children =
    registryState === 'ready'
      ? sections.map((section) => ({
          key: section.key,
          label: renderSettingsChromeLink({
            section,
            pathname,
            useRouterLinks
          })
        }))
      : [
          {
            key:
              registryState === 'error'
                ? 'settings-navigation-error'
                : 'settings-navigation-loading',
            disabled: true,
            label:
              registryState === 'error'
                ? i18nText('appShell', 'auto.console_navigation_load_failed')
                : i18nText('appShell', 'auto.console_navigation_loading')
          }
        ];

  return [
    {
      key: 'settings',
      label: (
        <span
          className="app-shell-settings-block"
          aria-label={i18nText('appShell', 'auto.settings')}
        >
          <SettingOutlined />
        </span>
      ),
      popupClassName: 'app-shell-settings-popup',
      children
    }
  ];
}
