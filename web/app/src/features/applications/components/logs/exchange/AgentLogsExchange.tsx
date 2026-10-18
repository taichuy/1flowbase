import UploadOutlined from '@ant-design/icons/es/icons/UploadOutlined';
import { useRef, useState } from 'react';
import {
  Alert,
  Button,
  Descriptions,
  Form,
  Modal,
  Select,
  Space,
  Typography,
  Tooltip
} from 'antd';
import { useTranslation } from 'react-i18next';
import {
  ApiClientError,
  type ApplicationLogPartialImportReceipt,
  type ApplicationLogImportReceipt
} from '@1flowbase/api-client';
import { useAuthStore } from '../../../../../state/auth-store';
import {
  exportApplicationLogsOtlp,
  importApplicationLogsOtlp
} from '../../../api/log-exchange';
import { saveApplicationRunExport } from '../../../lib/run-export-download';

type Operation = 'native_export' | 'otlp_export' | 'otlp_import';
interface Props {
  applicationId: string;
  allowImport?: boolean;
  onExportArchive?: () => Promise<void>;
  exportingArchive?: boolean;
  applications: Array<{ id: string; name: string }>;
  selectedRecords: Array<{
    id: string;
    application_id: string;
    log_conversation_id: string | null;
  }>;
  onImported: () => void;
}
interface Receipt {
  application_id: string;
  kind: 'download' | 'import' | 'error' | 'partial_import';
  partial_import?: ApplicationLogPartialImportReceipt;
  value?: ApplicationLogImportReceipt;
  filename?: string;
}

function readPartialImportReceipt(
  error: unknown
): ApplicationLogPartialImportReceipt | null {
  if (
    !(error instanceof ApiClientError) ||
    error.status !== 409 ||
    error.code !== 'agent_logs_partial_import'
  )
    return null;
  const body = error.body;
  if (!body || typeof body !== 'object' || !('details' in body)) return null;
  const details = body.details;
  if (!details || typeof details !== 'object') return null;
  if (
    !('accepted_events' in details) ||
    !('duplicate_events' in details) ||
    !('committed_records' in details)
  )
    return null;
  const { accepted_events, duplicate_events, committed_records } = details;
  if (
    ![accepted_events, duplicate_events, committed_records].every(
      (value) =>
        typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
    )
  )
    return null;
  return {
    accepted_events: accepted_events as number,
    duplicate_events: duplicate_events as number,
    committed_records: committed_records as number
  };
}

export function AgentLogsExchange(props: Props) {
  const { t } = useTranslation('applications');
  const [open, setOpen] = useState(false);
  return (
    <>
      {props.onExportArchive ? (
        <Tooltip title={t('auto.export_selected_runs_trace_dump')}>
          <Button
            aria-label={t('auto.export_selected_runs_trace_dump')}
            icon={<UploadOutlined aria-hidden="true" />}
            disabled={props.selectedRecords.length === 0}
            loading={props.exportingArchive}
            onClick={() => setOpen(true)}
          />
        </Tooltip>
      ) : (
        <Button onClick={() => setOpen(true)}>{t('logs.exchange.open')}</Button>
      )}
      {open && <ExchangeModal {...props} onClose={() => setOpen(false)} />}
    </>
  );
}

function ExchangeModal({
  applicationId,
  allowImport = true,
  onExportArchive,
  applications,
  selectedRecords,
  onImported,
  onClose
}: Props & { onClose: () => void }) {
  const { t } = useTranslation('applications');
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const [operation, setOperation] = useState<Operation>(
    onExportArchive
      ? 'native_export'
      : selectedRecords.length || !allowImport
        ? 'otlp_export'
        : 'otlp_import'
  );
  const [target, setTarget] = useState(applicationId);
  const [otlp, setOtlp] = useState<object | null>(null);
  const [fileError, setFileError] = useState(false);
  const [busy, setBusy] = useState(false);
  const [receipts, setReceipts] = useState<Receipt[]>([]);
  const running = useRef(false);
  const fileReadVersion = useRef(0);
  const importing = operation === 'otlp_import';
  const missingConversation = selectedRecords.some(
    (record) => !record.log_conversation_id
  );
  const ready = Boolean(
    csrfToken &&
    (!importing
      ? selectedRecords.length &&
        (operation === 'native_export' || !missingConversation)
      : target) &&
    (operation !== 'otlp_import' || otlp)
  );

  async function exchange() {
    if (!ready || running.current || !csrfToken) return;
    running.current = true;
    setBusy(true);
    setReceipts([]);
    if (operation === 'native_export') {
      try {
        await onExportArchive?.();
      } finally {
        running.current = false;
        setBusy(false);
      }
      return;
    }
    const groups = new Map<string, string[]>();
    if (importing) groups.set(target, []);
    else
      for (const record of selectedRecords) {
        const owner = applicationId || record.application_id;
        groups.set(
          owner,
          Array.from(
            new Set([...(groups.get(owner) ?? []), record.log_conversation_id!])
          )
        );
      }
    try {
      for (const [owner, log_conversation_ids] of groups) {
        let receipt: Receipt;
        try {
          if (operation === 'otlp_export') {
            const download = await exportApplicationLogsOtlp(
              owner,
              { log_conversation_ids },
              csrfToken
            );
            const filename = download.filename ?? `${owner}-logs-otlp.json`;
            saveApplicationRunExport(download, filename);
            receipt = { application_id: owner, kind: 'download', filename };
          } else {
            const value = await importApplicationLogsOtlp(
              owner,
              otlp!,
              csrfToken
            );
            receipt = { application_id: owner, kind: 'import', value };
            onImported();
          }
        } catch (error) {
          const partial_import = importing
            ? readPartialImportReceipt(error)
            : null;
          if (partial_import) {
            receipt = {
              application_id: owner,
              kind: 'partial_import',
              partial_import
            };
            if (partial_import.committed_records > 0) onImported();
          } else {
            receipt = { application_id: owner, kind: 'error' };
          }
        }
        setReceipts((previous) => [...previous, receipt]);
      }
    } finally {
      running.current = false;
      setBusy(false);
    }
  }

  return (
    <Modal
      open
      title={t('logs.exchange.open')}
      onCancel={onClose}
      closable={!busy}
      mask={{ closable: !busy }}
      keyboard={!busy}
      footer={
        <Space>
          <Button disabled={busy} onClick={onClose}>
            {t('auto.close')}
          </Button>
          <Button
            type="primary"
            disabled={!ready || busy}
            loading={busy}
            onClick={() => void exchange()}
          >
            {t(
              onExportArchive
                ? 'logs.exchange.confirm_download'
                : 'logs.exchange.execute'
            )}
          </Button>
        </Space>
      }
    >
      <Form layout="vertical" disabled={busy}>
        <Form.Item
          label={t(
            onExportArchive ? 'logs.exchange.format' : 'logs.exchange.operation'
          )}
        >
          <Select
            aria-label={t(
              onExportArchive
                ? 'logs.exchange.format'
                : 'logs.exchange.operation'
            )}
            value={operation}
            onChange={(value: Operation) => {
              setOperation(value);
              setReceipts([]);
            }}
            options={[
              ...(onExportArchive
                ? [{ value: 'native_export', label: '1flowbase' }]
                : []),
              {
                value: 'otlp_export',
                label: onExportArchive ? 'OTLP' : t('logs.exchange.otlp_export')
              },
              ...(allowImport
                ? [
                    {
                      value: 'otlp_import',
                      label: t('logs.exchange.otlp_import')
                    }
                  ]
                : [])
            ]}
          />
        </Form.Item>
        {importing ? (
          <Form.Item label={t('logs.import_target_application')}>
            {applicationId ? (
              <Typography.Text>
                {applications.find((app) => app.id === applicationId)?.name ??
                  applicationId}
              </Typography.Text>
            ) : (
              <Select
                aria-label={t('logs.import_target_application')}
                value={target || undefined}
                onChange={setTarget}
                options={applications.map((app) => ({
                  value: app.id,
                  label: app.name
                }))}
              />
            )}
          </Form.Item>
        ) : (
          <>
            <Typography.Paragraph>
              {t('logs.exchange.selection', { count: selectedRecords.length })}
            </Typography.Paragraph>
            {operation === 'otlp_export' && missingConversation && (
              <Alert
                type="warning"
                title={t('logs.exchange.missing_conversation')}
              />
            )}
          </>
        )}
        {operation === 'otlp_import' && (
          <Form.Item label={t('logs.exchange.file')}>
            <input
              aria-label={t('logs.exchange.file')}
              type="file"
              accept="application/json,.json"
              disabled={busy}
              onChange={async (event) => {
                const version = ++fileReadVersion.current;
                const file = event.target.files?.[0];
                setOtlp(null);
                setFileError(false);
                if (!file) return;
                try {
                  const value: unknown = JSON.parse(await file.text());
                  if (version !== fileReadVersion.current) return;
                  if (
                    !value ||
                    typeof value !== 'object' ||
                    Array.isArray(value)
                  )
                    throw new Error('object required');
                  setOtlp(value);
                } catch {
                  if (version === fileReadVersion.current) setFileError(true);
                }
              }}
            />
            {fileError && (
              <Alert type="error" title={t('logs.exchange.invalid_file')} />
            )}
          </Form.Item>
        )}
      </Form>
      <Space orientation="vertical" style={{ width: '100%' }} aria-live="polite">
        {receipts.map((receipt) => (
          <div key={receipt.application_id}>
            <Typography.Text strong>
              {applications.find((app) => app.id === receipt.application_id)
                ?.name ?? receipt.application_id}
            </Typography.Text>
            {receipt.kind === 'partial_import' ? (
              <>
                <Alert
                  type="error"
                  title={t('logs.exchange.partial_import')}
                  description={t('logs.exchange.partial_import_retry')}
                />
                <Descriptions
                  column={1}
                  size="small"
                  items={[
                    {
                      key: 'accepted_events',
                      label: t('logs.exchange.accepted_events'),
                      children: receipt.partial_import!.accepted_events
                    },
                    {
                      key: 'duplicate_events',
                      label: t('logs.exchange.duplicate_events'),
                      children: receipt.partial_import!.duplicate_events
                    },
                    {
                      key: 'committed_records',
                      label: t('logs.exchange.committed_records'),
                      children: receipt.partial_import!.committed_records
                    }
                  ]}
                />
              </>
            ) : receipt.kind === 'error' ? (
              <Alert type="error" title={t('logs.exchange.failed')} />
            ) : receipt.kind === 'download' ? (
              <Alert
                type="info"
                title={t('logs.exchange.downloaded', {
                  filename: receipt.filename
                })}
              />
            ) : (
              <>
                <Alert type="info" title={t('logs.exchange.receipt')} />
                <Descriptions
                  column={1}
                  size="small"
                  items={[
                    {
                      key: 'accepted_events',
                      label: t('logs.exchange.accepted_events'),
                      children: (receipt.value as ApplicationLogImportReceipt)
                        .accepted_events
                    },
                    {
                      key: 'duplicate_events',
                      label: t('logs.exchange.duplicate_events'),
                      children: (receipt.value as ApplicationLogImportReceipt)
                        .duplicate_events
                    },
                    {
                      key: 'record_ids',
                      label: t('logs.exchange.record_ids'),
                      children: (
                        receipt.value as ApplicationLogImportReceipt
                      ).record_ids.join(', ')
                    }
                  ]}
                />
              </>
            )}
          </div>
        ))}
      </Space>
    </Modal>
  );
}
