import { i18nText } from '../../../../shared/i18n/text';

export function formatPluginAvailabilityStatus(status: string) {
  switch (status) {
    case 'available':
      return { color: 'green', label: i18nText('settings', 'auto.available') };
    case 'pending_restart':
      return {
        color: 'gold',
        label: i18nText('settings', 'auto.to_be_restarted')
      };
    case 'load_failed':
      return {
        color: 'red',
        label: i18nText('settings', 'auto.loading_failed')
      };
    case 'artifact_missing':
      return {
        color: 'red',
        label: i18nText('settings', 'auto.product_missing')
      };
    case 'install_incomplete':
      return {
        color: 'orange',
        label: i18nText('settings', 'auto.incomplete_installation')
      };
    default:
      return { color: 'default', label: i18nText('settings', 'auto.disabled') };
  }
}

export function formatPluginArtifactAvailabilityStatus(status: string) {
  if (status === 'ready') {
    return {
      color: 'green',
      label: i18nText('settings', 'auto.available')
    };
  }

  return {
    color: 'red',
    label: i18nText('settings', 'auto.unavailable')
  };
}

export function isPluginArtifactUnavailable(status: string) {
  return status !== 'ready';
}
