import { createElement } from 'react';

import { Modal } from 'antd';

import { i18nText } from '../../../../shared/i18n/text';
import type {
  SettingsOfficialPluginCatalogEntry,
  SettingsPluginCompatibilityOverride,
  SettingsPluginFamilyEntry
} from '../../api/plugins';
import { OfficialPluginInstallConfirmContent } from './OfficialPluginUpgradeConfirmation';

const BELOW_MINIMUM_HOST_VERSION = 'below_minimum_host_version';
const INSTALL_CONFIRM_MODAL_WIDTH = 640;

function isBelowMinimumHostVersion(entry: SettingsOfficialPluginCatalogEntry) {
  return entry.compatibility_status === BELOW_MINIMUM_HOST_VERSION;
}

export function confirmOfficialPluginUpgrade({
  modal,
  entry,
  family,
  upgrading,
  onUpgradeLatest
}: {
  modal: ReturnType<typeof Modal.useModal>[0];
  entry: SettingsOfficialPluginCatalogEntry;
  family: SettingsPluginFamilyEntry;
  upgrading: boolean;
  onUpgradeLatest: (
    entry: SettingsOfficialPluginCatalogEntry,
    compatibilityOverride?: SettingsPluginCompatibilityOverride
  ) => void;
}) {
  const belowMinimumHostVersion = isBelowMinimumHostVersion(entry);
  const buttonLabel = belowMinimumHostVersion
    ? i18nText('settings', 'auto.still_update')
    : i18nText('settings', 'auto.upgrade_latest_version');
  const compatibilityOverride = belowMinimumHostVersion
    ? ({
        reason: BELOW_MINIMUM_HOST_VERSION,
        acknowledged_current_host_version: entry.current_host_version,
        acknowledged_minimum_host_version: entry.minimum_host_version
      } satisfies SettingsPluginCompatibilityOverride)
    : undefined;

  void modal.confirm({
    title: i18nText('settings', 'auto.upgrade_plugin'),
    icon: null,
    centered: true,
    width: INSTALL_CONFIRM_MODAL_WIDTH,
    okText: buttonLabel,
    cancelText: i18nText('settings', 'auto.cancel'),
    okButtonProps: {
      loading: upgrading,
      disabled: !family.has_update
    },
    content: createElement(OfficialPluginInstallConfirmContent, {
      entry,
      family,
      belowMinimumHostVersion
    }),
    onOk: async () => {
      onUpgradeLatest(entry, compatibilityOverride);
    }
  });
}
