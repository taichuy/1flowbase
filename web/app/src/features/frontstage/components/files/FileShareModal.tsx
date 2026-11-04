import { useEffect, useRef, useState } from 'react';
import {
  Alert,
  App,
  Button,
  Form,
  InputNumber,
  Modal,
  Space,
  Tooltip,
  Typography
} from 'antd';
import CopyOutlined from '@ant-design/icons/es/icons/CopyOutlined';
import copy from 'copy-to-clipboard';
import {
  createConsoleFileShareLink,
  type ConsoleFileTable,
  type FileShareLink
} from '@1flowbase/api-client';
import { i18nText } from '../../../../shared/i18n/text';
import { formatDateTime } from '../../../../shared/i18n/format';
import { useNativeBlockSurface } from '../../lib/native-modules/native-block-surface-context';

interface Props {
  fileTableId: string;
  recordId: string;
  filename: string;
  csrfToken: string;
  record_permissions: ConsoleFileTable['record_permissions'];
  onClose: () => void;
}

export function FileShareModal({
  fileTableId,
  recordId,
  filename,
  csrfToken,
  record_permissions,
  onClose
}: Props) {
  const { message } = App.useApp();
  const surface = useNativeBlockSurface();
  const [form] = Form.useForm<{ expires_in_seconds: number }>();
  const [link, setLink] = useState<FileShareLink>();
  const [error, setError] = useState<string>();
  const [generating, setGenerating] = useState(false);
  const pending = useRef<AbortController | null>(null);
  useEffect(() => () => pending.current?.abort(), []);
  const absoluteUrl = link
    ? new URL(link.preview_url ?? link.url, window.location.origin).href
    : '';

  async function generate(input: { expires_in_seconds: number }) {
    if (!record_permissions.can_share) return;
    pending.current?.abort();
    const controller = new AbortController();
    pending.current = controller;
    setGenerating(true);
    setError(undefined);
    setLink(undefined);
    try {
      const result = await createConsoleFileShareLink(
        fileTableId,
        recordId,
        input,
        csrfToken,
        controller.signal
      );
      if (!controller.signal.aborted) setLink(result);
    } catch (failure) {
      if (!controller.signal.aborted)
        setError(
          failure instanceof Error
            ? failure.message
            : i18nText('frontstage', 'files.link_failed')
        );
    } finally {
      if (!controller.signal.aborted) setGenerating(false);
    }
  }

  return (
    <Modal
      open
      title={i18nText('frontstage', 'files.share')}
      getContainer={surface?.overlayHost.getPopupContainer}
      onCancel={onClose}
      footer={
        <Space>
          <Button
            type="primary"
            disabled={!record_permissions.can_share}
            loading={generating}
            onClick={() => form.submit()}
          >
            {i18nText('frontstage', 'files.generate_link')}
          </Button>
          <Button onClick={onClose}>
            {i18nText('frontstage', 'auto.cancel')}
          </Button>
        </Space>
      }
    >
      <Space orientation="vertical" size={16} style={{ width: '100%' }}>
        <Typography.Text>{filename}</Typography.Text>
        <Typography.Paragraph type="secondary" style={{ margin: 0 }}>
          {i18nText('frontstage', 'files.share_hint')}
        </Typography.Paragraph>
        <Form
          form={form}
          disabled={!record_permissions.can_share}
          layout="vertical"
          initialValues={{ expires_in_seconds: 600 }}
          onFinish={(values) => void generate(values)}
          onValuesChange={() => setLink(undefined)}
        >
          <Form.Item
            name="expires_in_seconds"
            label={i18nText('frontstage', 'files.expires_in_seconds')}
            rules={[
              {
                validator: (_, value: number | null) =>
                  value != null && Number.isSafeInteger(value) && value > 0
                    ? Promise.resolve()
                    : Promise.reject(
                        new Error(
                          i18nText('frontstage', 'files.positive_duration')
                        )
                      )
              }
            ]}
          >
            <InputNumber
              disabled={generating}
              min={1}
              step={60}
              style={{ width: '100%' }}
            />
          </Form.Item>
        </Form>
        {error ? <Alert showIcon type="error" title={error} /> : null}
        {link ? (
          <Space orientation="vertical" style={{ width: '100%' }}>
            <Typography.Link
              aria-label={i18nText('frontstage', 'files.share_url')}
              href={absoluteUrl}
              target="_blank"
              rel="noopener noreferrer"
              style={{ minWidth: 0, overflowWrap: 'anywhere' }}
            >
              {absoluteUrl}
            </Typography.Link>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <Tooltip
                title={i18nText('frontstage', 'files.copy_link')}
                getPopupContainer={surface?.overlayHost.getPopupContainer}
              >
                <Button
                  type="text"
                  size="small"
                  icon={<CopyOutlined aria-hidden="true" />}
                  aria-label={i18nText('frontstage', 'files.copy_link')}
                  style={{ flexShrink: 0 }}
                  onClick={async () => {
                    try {
                      if (await copy(absoluteUrl))
                        message.success(
                          i18nText('frontstage', 'files.link_copied')
                        );
                      else
                        message.error(
                          i18nText('frontstage', 'files.copy_failed')
                        );
                    } catch {
                      message.error(
                        i18nText('frontstage', 'files.copy_failed')
                      );
                    }
                  }}
                />
              </Tooltip>
              <Typography.Text type="secondary">
                {i18nText('frontstage', 'files.expires_at', {
                  time: formatDateTime(link.expires_at)
                })}
              </Typography.Text>
            </div>
          </Space>
        ) : null}
      </Space>
    </Modal>
  );
}
