import { switchSettingsPluginFamilyVersion } from '../../api/extensions/plugins';
import { useMutation } from '@tanstack/react-query';
import { extensionOperationErrorText } from '../../lib/extension-operation-error';
import { App, Button, Modal } from 'antd';
import { useTranslation } from 'react-i18next';
import {
  enableSettingsInstalledExtension,
  type SettingsInstalledExtension
} from '../../api/extensions/extensions';

export function InstalledExtensionVersionSwitch({
  installation,
  version,
  csrfToken,
  onChanged
}: {
  installation: SettingsInstalledExtension;
  version: SettingsInstalledExtension['installed_versions'][number];
  csrfToken: string;
  onChanged(): Promise<void>;
}) {
  const { t } = useTranslation('settings');
  const { message } = App.useApp();
  const mutation = useMutation({
    mutationFn: async () =>
      installation.contract_version === '1flowbase.extension-bus/v1' &&
      installation.desired_state === 'disabled'
        ? enableSettingsInstalledExtension(version.id, csrfToken)
        : switchSettingsPluginFamilyVersion(
            installation.artifact_id,
            version.id,
            csrfToken
          ),
    onSuccess: async () => {
      message.success(t('auto.extension_operation_completed'));
      await onChanged();
    },
    onError: (error) => message.error(extensionOperationErrorText(error, t))
  });
  return (
    <Button
      type="link"
      disabled={version.is_current || version.status !== 'installed'}
      loading={mutation.isPending}
      onClick={() =>
        Modal.confirm({
          title: t('auto.extension_switch_version'),
          content: t('auto.extension_switch_version_confirmation', {
            version: version.version
          }),
          okText: t('auto.extension_switch_version'),
          cancelText: t('auto.cancel'),
          onOk: () => mutation.mutateAsync()
        })
      }
    >
      {t('auto.extension_switch_version')}
    </Button>
  );
}
