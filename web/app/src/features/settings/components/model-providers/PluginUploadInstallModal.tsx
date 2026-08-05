import { Alert, Button, Modal, Typography, Upload } from 'antd';
import type { UploadFile } from 'antd/es/upload/interface';
import { i18nText } from '../../../../shared/i18n/text';

export function PluginUploadInstallModal({
  open,
  submitting,
  resultSummary,
  errorMessage,
  fileList,
  onClose,
  onChange,
  onSubmit
}: {
  open: boolean;
  submitting: boolean;
  resultSummary: {
    displayName: string;
    version: string;
    trustLabel: string;
    availabilityLabel: string;
  } | null;
  errorMessage: string | null;
  fileList: UploadFile[];
  onClose: () => void;
  onChange: (nextFiles: UploadFile[]) => void;
  onSubmit: () => void;
}) {
  return (
    <Modal
      open={open}
      title={i18nText('settings', 'auto.upload_plugin')}
      onCancel={onClose}
      footer={null}
      destroyOnHidden
    >
      <div className="model-provider-panel__upload-modal">
        <Typography.Paragraph type="secondary">
          {i18nText(
            'settings',
            'auto.supports_one_flowbasepkg_compatible_tar_gz_zip_uploading_host_backend'
          )}
        </Typography.Paragraph>
        <Upload.Dragger
          beforeUpload={() => false}
          maxCount={1}
          fileList={fileList}
          onChange={({ fileList: nextFiles }) => onChange(nextFiles)}
        >
          {i18nText('settings', 'auto.select_plug_package_upload_install')}
        </Upload.Dragger>
        {resultSummary ? (
          <Alert
            type="success"
            showIcon
            title={`${resultSummary.displayName} ${resultSummary.version}`}
            description={i18nText(
              'settings',
              'auto.source_manual_upload_trust_level_status',
              {
                value1: resultSummary.trustLabel,
                value2: resultSummary.availabilityLabel
              }
            )}
          />
        ) : null}
        {errorMessage ? (
          <Alert type="error" showIcon title={errorMessage} />
        ) : null}
        <Button type="primary" block loading={submitting} onClick={onSubmit}>
          {i18nText('settings', 'auto.upload_and_install')}
        </Button>
      </div>
    </Modal>
  );
}
