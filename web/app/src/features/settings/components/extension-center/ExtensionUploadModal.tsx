import { extensionRiskWarningText } from '../../lib/extension-risk-warning';
import { useRef, useState } from 'react';
import { extensionOperationErrorText } from '../../lib/extension-operation-error';
import {
  Alert,
  Button,
  Flex,
  Modal,
  Progress,
  Table,
  Typography,
  Upload
} from 'antd';
import UploadOutlined from '@ant-design/icons/es/icons/UploadOutlined';
import { useTranslation } from 'react-i18next';
import {
  getSettingsExtensionRiskChallenge,
  uploadSettingsExtension
} from '../../api/extensions/extensions';

type Phase =
  | 'ready'
  | 'uploading'
  | 'installing'
  | 'confirming'
  | 'success'
  | 'error';
type Challenge = NonNullable<
  ReturnType<typeof getSettingsExtensionRiskChallenge>
>;
type ProgressValue = Parameters<
  NonNullable<Parameters<typeof uploadSettingsExtension>[4]>
>[0];
interface UploadItem {
  id: string;
  file: File;
  phase: Phase;
  progress: ProgressValue;
  challenge: Challenge | null;
  error: string | null;
}

function fileSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(2)} KB`;
  return `${(bytes / 1024 ** 2).toFixed(2)} MB`;
}

export function ExtensionUploadModal({
  csrfToken,
  onClose,
  onInstalled
}: {
  csrfToken: string;
  onClose(): void;
  onInstalled(): Promise<void>;
}) {
  const { t } = useTranslation('settings');
  const [items, setItems] = useState<UploadItem[]>([]);
  const [busy, setBusy] = useState(false);
  const [refreshError, setRefreshError] = useState(false);
  const running = useRef(false);
  const confirming = items.find((item) => item.phase === 'confirming');
  const blocked =
    confirming?.challenge?.warnings.some((warning) => !warning.overridable) ??
    false;
  const pending = items.filter(
    (item) => item.phase === 'ready' || item.phase === 'error'
  );
  const finished =
    items.length > 0 && items.every((item) => item.phase === 'success');

  function patch(id: string, value: Partial<UploadItem>) {
    setItems((current) =>
      current.map((item) => (item.id === id ? { ...item, ...value } : item))
    );
  }

  async function start() {
    if (running.current || !csrfToken || blocked) return;
    const batch = confirming ? [confirming, ...pending] : pending;
    if (!batch.length) return;
    running.current = true;
    setBusy(true);
    setRefreshError(false);
    let installed = false;
    try {
      for (const item of batch) {
        const challenge = item.challenge;
        const overrides = challenge
          ? {
              risk_override: {
                reason: 'user_confirmed',
                acknowledged_warnings: challenge.warnings
                  .filter((warning) => warning.overridable)
                  .map((warning) => warning.code)
              },
              ...(challenge.compatibility
                ? {
                    compatibility_override: {
                      reason: challenge.compatibility.reason,
                      acknowledged_current_host_version:
                        challenge.compatibility.current_host_version,
                      acknowledged_minimum_host_version:
                        challenge.compatibility.minimum_host_version
                    }
                  }
                : {})
            }
          : {};
        patch(item.id, {
          phase: 'uploading',
          progress: { loaded: 0 },
          error: null
        });
        try {
          await uploadSettingsExtension(
            item.file,
            {},
            csrfToken,
            overrides,
            (progress) => {
              patch(item.id, {
                progress,
                ...(progress.complete ? { phase: 'installing' } : {})
              });
            }
          );
          patch(item.id, { phase: 'success', challenge: null });
          installed = true;
        } catch (error) {
          const nextChallenge = getSettingsExtensionRiskChallenge(error);
          patch(item.id, {
            phase: nextChallenge ? 'confirming' : 'error',
            challenge: nextChallenge,
            error: !nextChallenge ? extensionOperationErrorText(error, t) : null
          });
          if (nextChallenge) break;
        }
      }
      if (installed) {
        try {
          await onInstalled();
        } catch {
          setRefreshError(true);
        }
      }
    } finally {
      running.current = false;
      setBusy(false);
    }
  }

  function status(item: UploadItem) {
    if (item.phase === 'uploading') {
      const percent = item.progress.total
        ? Math.floor((item.progress.loaded * 100) / item.progress.total)
        : 0;
      return (
        <Flex vertical gap={2}>
          <Typography.Text>
            {t('auto.extension_upload_uploading')}
          </Typography.Text>
          {
            <Progress
              size="small"
              percent={percent}
              status="normal"
              format={(value) =>
                item.progress.total
                  ? `${value}%`
                  : `${item.progress.loaded.toLocaleString()} B`
              }
            />
          }
        </Flex>
      );
    }
    const label = {
      ready: t('auto.extension_upload_ready'),
      uploading: t('auto.extension_upload_uploading'),
      installing: t('auto.extension_upload_installing'),
      confirming: t('auto.extension_upload_confirmation'),
      success: t('auto.extension_upload_success'),
      error: t('auto.extension_upload_failed')
    }[item.phase];
    return (
      <Flex vertical gap={4}>
        <Typography.Text
          type={
            item.phase === 'error'
              ? 'danger'
              : item.phase === 'success'
                ? 'success'
                : undefined
          }
        >
          {label}
        </Typography.Text>
        {item.error && (
          <Typography.Text type="danger">{item.error}</Typography.Text>
        )}
      </Flex>
    );
  }

  return (
    <Modal
      open
      title={t('auto.upload_plugin')}
      width={760}
      onCancel={onClose}
      closable={!busy}
      mask={{ closable: !busy }}
      keyboard={!busy}
      footer={
        <Flex justify="flex-end" gap={8}>
          <Button disabled={busy} onClick={onClose}>
            {finished ? t('auto.extension_upload_back') : t('auto.cancel')}
          </Button>
          <Button
            type="primary"
            loading={busy}
            disabled={!csrfToken || blocked || (!pending.length && !confirming)}
            onClick={() => void start()}
          >
            {confirming
              ? t('auto.extension_upload_confirm_install')
              : t('auto.extension_upload_start')}
          </Button>
        </Flex>
      }
    >
      <Flex vertical gap={16}>
        <Flex justify="space-between" gap={8} wrap>
          <Upload
            multiple
            accept=".1flowbasepkg,.tar.gz,.tgz,.zip"
            showUploadList={false}
            disabled={busy}
            beforeUpload={(file) => {
              setItems((current) => [
                ...current,
                {
                  id: file.uid,
                  file,
                  phase: 'ready',
                  progress: { loaded: 0 },
                  challenge: null,
                  error: null
                }
              ]);
              return Upload.LIST_IGNORE;
            }}
          >
            <Button icon={<UploadOutlined />} disabled={busy}>
              {t('auto.extension_upload_select')}
            </Button>
          </Upload>
          <Button disabled={busy || !items.length} onClick={() => setItems([])}>
            {t('auto.extension_upload_clear')}
          </Button>
        </Flex>
        <Table<UploadItem>
          rowKey="id"
          size="small"
          pagination={false}
          dataSource={items}
          scroll={{ x: 620 }}
          columns={[
            {
              title: t('auto.name'),
              width: 250,
              render: (_: unknown, item: UploadItem) => (
                <Typography.Text
                  ellipsis={{ tooltip: item.file.name }}
                  style={{ maxWidth: '100%' }}
                >
                  {item.file.name}
                </Typography.Text>
              )
            },
            {
              title: t('auto.size'),
              width: 100,
              render: (_: unknown, item: UploadItem) => fileSize(item.file.size)
            },
            {
              title: t('auto.status'),
              width: 170,
              render: (_: unknown, item: UploadItem) => (
                <span role="status" aria-live="polite">
                  {status(item)}
                </span>
              )
            },
            {
              title: t('auto.operation'),
              width: 80,
              render: (_: unknown, item: UploadItem) => (
                <Button
                  type="link"
                  disabled={busy}
                  onClick={() =>
                    setItems((current) =>
                      current.filter((row) => row.id !== item.id)
                    )
                  }
                >
                  {t('auto.extension_upload_remove')}
                </Button>
              )
            }
          ]}
        />
        {confirming?.challenge && (
          <Alert
            type="warning"
            showIcon
            title={t('auto.risk_warnings')}
            description={
              <Flex vertical gap={8}>
                <Typography.Text>{confirming.file.name}</Typography.Text>
                {confirming.challenge.warnings.map((warning) => (
                  <Typography.Text key={warning.code}>
                    {extensionRiskWarningText(warning.code, warning.message, t)}
                  </Typography.Text>
                ))}
                {confirming.challenge.compatibility && (
                  <Typography.Text>
                    {t('auto.extension_upload_compatibility', {
                      current:
                        confirming.challenge.compatibility.current_host_version,
                      minimum:
                        confirming.challenge.compatibility.minimum_host_version
                    })}
                  </Typography.Text>
                )}
                {blocked && (
                  <Typography.Text>
                    {t('auto.extension_upload_blocked')}
                  </Typography.Text>
                )}
              </Flex>
            }
          />
        )}
        {items.some((item) => item.phase === 'success') && (
          <Typography.Text type="secondary">
            {t('auto.extension_upload_installed_only')}
          </Typography.Text>
        )}
        {refreshError && (
          <Alert
            type="error"
            showIcon
            title={t('auto.extension_upload_refresh_failed')}
          />
        )}
      </Flex>
    </Modal>
  );
}
