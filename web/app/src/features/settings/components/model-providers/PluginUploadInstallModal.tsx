import { DeleteOutlined } from '@ant-design/icons';
import { Alert, Button, Modal, Tooltip, Typography, Upload } from 'antd';
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
          className="model-provider-panel__upload-control"
          maxCount={1}
          fileList={fileList}
          showUploadList={false}
          onChange={({ fileList: nextFiles }) => onChange(nextFiles)}
        >
          {i18nText('settings', 'auto.select_plug_package_upload_install')}
        </Upload.Dragger>
        {fileList.length > 0 ? (
          <div className="model-provider-panel__upload-file-list">
            {fileList.map((file) => (
              <div className="model-provider-panel__upload-file" key={file.uid}>
                <Tooltip title={file.name}>
                  <Typography.Text
                    className="model-provider-panel__upload-file-name"
                    ellipsis
                    title={file.name}
                  >
                    {file.name}
                  </Typography.Text>
                </Tooltip>
                <Button
                  aria-label={i18nText(
                    'settings',
                    'auto.remove_upload_package'
                  )}
                  icon={<DeleteOutlined />}
                  onClick={() =>
                    onChange(
                      fileList.filter((candidate) => candidate.uid !== file.uid)
                    )
                  }
                  size="small"
                  type="text"
                />
              </div>
            ))}
          </div>
        ) : null}
        {resultSummary ? (
          <Alert
            className="model-provider-panel__upload-alert"
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
          <Alert
            className="model-provider-panel__upload-alert"
            type="error"
            showIcon
            title={errorMessage}
          />
        ) : null}
        <Button
          className="model-provider-panel__upload-submit"
          type="primary"
          block
          loading={submitting}
          onClick={onSubmit}
        >
          {i18nText('settings', 'auto.upload_and_install')}
        </Button>
      </div>
    </Modal>
  );
}
