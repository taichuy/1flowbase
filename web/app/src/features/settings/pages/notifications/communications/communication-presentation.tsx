import { useMutation } from '@tanstack/react-query';
import { Button, Space, Tag, Tooltip, Typography } from 'antd';
import AimOutlined from '@ant-design/icons/es/icons/AimOutlined';
import FileOutlined from '@ant-design/icons/es/icons/FileOutlined';
import { useTranslation } from 'react-i18next';
import type { CommunicationMessage } from '@1flowbase/api-client';
import { getCommunicationFile } from '../../../../notifications/api/communications';
import { communicationRunHref } from './communication-navigation';

export function CommunicationRunLink({
  item
}: {
  item: { application_id: string | null; run_id: string | null };
}) {
  const { t } = useTranslation('notifications');
  const href = communicationRunHref(item);
  return href ? (
    <Tooltip title={t('view_agent_flow_logs')}>
      <Button
        type="text"
        size="small"
        href={href}
        aria-label={t('view_agent_flow_logs')}
        icon={<AimOutlined />}
      />
    </Tooltip>
  ) : null;
}
export function ProcessingStatus({
  status
}: {
  status: CommunicationMessage['processing_status'];
}) {
  const { t } = useTranslation('notifications');
  if (!status) return <>—</>;
  const labels = {
    pending: t('processing_pending'),
    processing: t('processing_active'),
    succeeded: t('processing_succeeded'),
    failed: t('processing_failed')
  };
  return (
    <Tag
      color={
        status === 'failed'
          ? 'error'
          : status === 'succeeded'
            ? 'success'
            : status === 'pending'
              ? 'warning'
              : 'var(--status-running)'
      }
    >
      {labels[status]}
    </Tag>
  );
}
export function DeliveryStatus({
  status
}: {
  status: CommunicationMessage['send_status'];
}) {
  const { t } = useTranslation('notifications');
  if (!status) return <>—</>;
  const labels = {
    none: t('delivery_none'),
    pending: t('delivery_pending'),
    sending: t('delivery_sending'),
    succeeded: t('send_success'),
    failed: t('send_failure'),
    unknown: t('result_unconfirmed')
  };
  return (
    <Tag
      color={
        status === 'failed'
          ? 'error'
          : status === 'succeeded'
            ? 'success'
            : status === 'pending' || status === 'unknown'
              ? 'warning'
              : status === 'sending'
                ? 'var(--status-running)'
                : undefined
      }
    >
      {labels[status]}
    </Tag>
  );
}
export function MessageStatus({
  message
}: {
  message: Pick<
    CommunicationMessage,
    'processing_status' | 'send_status' | 'failure_code'
  >;
}) {
  const { t } = useTranslation('notifications');
  return (
    <Space wrap>
      <ProcessingStatus status={message.processing_status} />
      <DeliveryStatus status={message.send_status} />
      {message.failure_code && (
        <Typography.Text type="danger">
          {t('failure_reason')}: {failureLabel(message.failure_code, t)}
        </Typography.Text>
      )}
    </Space>
  );
}
export function CommunicationFile({
  message
}: {
  message: CommunicationMessage;
}) {
  const { t } = useTranslation('notifications');
  const link = useMutation({
    mutationFn: () => getCommunicationFile(message.instance_id, message.id)
  });
  const file = message.file;
  if (!file) return null;
  return (
    <Space wrap className="communication-chat__file">
      <FileOutlined />
      <Typography.Text>{file.file_name}</Typography.Text>
      <Tag>
        {file.status === 'ready'
          ? t('file_ready')
          : file.status === 'failed'
            ? t('file_failed')
            : t('file_pending')}
      </Tag>
      {file.status === 'ready' && !link.data?.url && (
        <Button loading={link.isPending} onClick={() => link.mutate()}>
          {t('file_access')}
        </Button>
      )}
      {link.data?.url && (
        <Typography.Link
          href={link.data.url}
          target="_blank"
          rel="noopener noreferrer"
        >
          {t('download_file')}
        </Typography.Link>
      )}
      {(file.failure_code || link.isError || (link.data && !link.data.url)) && (
        <Typography.Text type="danger">
          {file.failure_code
            ? failureLabel(file.failure_code, t)
            : t('file_access_failed')}
        </Typography.Text>
      )}
    </Space>
  );
}

export function failureLabel(code: string, t: (key: string) => string) {
  switch (code) {
    case 'run_interrupted':
      return t('failure_run_interrupted');
    case 'send_interrupted':
      return t('failure_send_interrupted');
    case 'download_failed':
      return t('failure_download_failed');
    case 'upload_failed':
      return t('failure_upload_failed');
    case 'run_failed':
      return t('failure_run_failed');
    case 'no_final_answer':
      return t('failure_no_final_answer');
    case 'provider_rejected':
      return t('failure_provider_rejected');
    default:
      return t('failure_reason_unavailable');
  }
}
