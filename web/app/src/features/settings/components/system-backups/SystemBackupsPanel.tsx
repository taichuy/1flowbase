import {
  createSystemBackup,
  createSystemRecoveryIntent,
  deleteSystemBackup,
  getSystemBackupDownloadUrl,
  getSystemBackup,
  getSystemBackupJobStatus,
  getSystemRecoveryStatus,
  importSystemBackup,
  listSystemBackups,
  preflightSystemRecovery,
  reauthenticateSystemRecovery,
  verifySystemBackup,
  type BackupSetSummaryResponse,
  type RecoveryPreflightResponse
} from '@1flowbase/api-client';
import {
  DeleteOutlined,
  DownloadOutlined,
  MoreOutlined,
  PlusOutlined,
  ReloadOutlined,
  SafetyCertificateOutlined,
  UploadOutlined,
  WarningOutlined
} from '@ant-design/icons';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  App,
  Button,
  Descriptions,
  Drawer,
  Dropdown,
  Flex,
  Input,
  Modal,
  Select,
  Space,
  Steps,
  Table,
  Tag,
  Typography,
  Upload
} from 'antd';
import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { useAuthStore } from '../../../../state/auth-store';
import { formatDateTime } from '../../../../shared/i18n/format';
import { SettingsSectionSurface } from '../SettingsSectionSurface';
import './system-backups-panel.css';

const queryKey = ['settings', 'system-backups'] as const;

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 ** 2) return `${(value / 1024).toFixed(1)} KB`;
  if (value < 1024 ** 3) return `${(value / 1024 ** 2).toFixed(1)} MB`;
  return `${(value / 1024 ** 3).toFixed(1)} GB`;
}

function startDirectDownload(backupSetId: string) {
  const anchor = document.createElement('a');
  anchor.href = getSystemBackupDownloadUrl(backupSetId);
  anchor.download = '';
  anchor.rel = 'noopener';
  anchor.click();
}

export function SystemBackupsPanel() {
  const { t } = useTranslation('settingsSystemBackups');
  const csrfToken = useAuthStore((state) => state.csrfToken) ?? '';
  const queryClient = useQueryClient();
  const { message, modal } = App.useApp();
  const [keyword, setKeyword] = useState('');
  const [availability, setAvailability] = useState<string>();
  const [detailId, setDetailId] = useState<string>();
  const [restoreTarget, setRestoreTarget] =
    useState<BackupSetSummaryResponse>();
  const [preflight, setPreflight] = useState<RecoveryPreflightResponse>();
  const [password, setPassword] = useState('');
  const [createOpen, setCreateOpen] = useState(false);
  const [backupPassword, setBackupPassword] = useState('');
  const [pendingImport, setPendingImport] = useState<File>();
  const [importPassword, setImportPassword] = useState('');
  const [verifyTarget, setVerifyTarget] = useState<BackupSetSummaryResponse>();
  const [verifyPassword, setVerifyPassword] = useState('');
  const [recoveryBackupPassword, setRecoveryBackupPassword] = useState('');
  const [exactName, setExactName] = useState('');
  const [backupJobId, setBackupJobId] = useState<string>();
  const [recoveryJobId, setRecoveryJobId] = useState<string>();
  const availabilityLabel = (value: string) => {
    if (value === 'ready') return t('availability_ready');
    if (value === 'corrupt') return t('availability_corrupt');
    return t('availability_incompatible');
  };

  const backups = useQuery({ queryKey, queryFn: () => listSystemBackups() });
  const detail = useQuery({
    queryKey: [...queryKey, detailId],
    queryFn: () => getSystemBackup(detailId!),
    enabled: Boolean(detailId)
  });
  const backupJobStatus = useQuery({
    queryKey: [...queryKey, 'job-status', backupJobId],
    queryFn: () => getSystemBackupJobStatus(backupJobId!),
    enabled: Boolean(backupJobId),
    refetchInterval: (query) =>
      query.state.data?.status === 'succeeded' ||
      query.state.data?.status === 'failed'
        ? false
        : 2000
  });
  const recoveryStatus = useQuery({
    queryKey: [...queryKey, 'recovery-status', recoveryJobId],
    queryFn: () => getSystemRecoveryStatus(recoveryJobId),
    enabled: Boolean(recoveryJobId),
    refetchInterval: 2000
  });
  const refresh = () => queryClient.invalidateQueries({ queryKey });
  useEffect(() => {
    if (backupJobStatus.data?.status === 'succeeded') {
      void queryClient.invalidateQueries({ queryKey });
    }
  }, [backupJobStatus.data?.status, queryClient]);
  const notifyError = () => message.error(t('operation_failed'));
  const createMutation = useMutation({
    mutationFn: (backupPassword?: string) =>
      createSystemBackup(
        csrfToken,
        undefined,
        backupPassword ? { backup_password: backupPassword } : undefined
      ),
    onSuccess: (queued) => {
      setBackupJobId(queued.backup_job_id);
      setCreateOpen(false);
      setBackupPassword('');
      message.success(t('backup_started'));
    },
    onError: notifyError
  });
  const importMutation = useMutation({
    mutationFn: ({ file, password }: { file: File; password?: string }) =>
      importSystemBackup(file, csrfToken, undefined, password),
    onSuccess: async () => {
      await refresh();
      setPendingImport(undefined);
      setImportPassword('');
      message.success(t('import_succeeded'));
    },
    onError: notifyError
  });
  const verifyMutation = useMutation({
    mutationFn: ({ id, password }: { id: string; password?: string }) =>
      verifySystemBackup(id, csrfToken, undefined, password),
    onSuccess: async () => {
      await refresh();
      setVerifyTarget(undefined);
      setVerifyPassword('');
      message.success(t('verify_succeeded'));
    },
    onError: notifyError
  });
  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteSystemBackup(id, csrfToken),
    onSuccess: async () => {
      await refresh();
      message.success(t('delete_succeeded'));
    },
    onError: notifyError
  });
  const preflightMutation = useMutation({
    mutationFn: ({ id, password }: { id: string; password?: string }) =>
      preflightSystemRecovery(id, csrfToken, undefined, password),
    onSuccess: setPreflight,
    onError: notifyError
  });
  const intentMutation = useMutation({
    mutationFn: async () => {
      if (!restoreTarget || !preflight)
        throw new Error('missing recovery contract');
      const challenge = await reauthenticateSystemRecovery(
        {
          backup_set_id: restoreTarget.backup_set_id,
          exact_backup_name: exactName,
          plan_digest: preflight.plan_digest,
          password,
          backup_password: recoveryBackupPassword || undefined
        },
        csrfToken
      );
      return createSystemRecoveryIntent(
        restoreTarget.backup_set_id,
        {
          challenge_token: challenge.challenge_token,
          exact_backup_name: exactName,
          plan_digest: preflight.plan_digest,
          backup_password: recoveryBackupPassword || undefined
        },
        csrfToken
      );
    },
    onSuccess: (intent) => setRecoveryJobId(intent.recovery_job_id),
    onError: notifyError
  });

  const filtered = useMemo(
    () =>
      (backups.data?.items ?? []).filter((item) => {
        const matchesKeyword =
          !keyword ||
          item.exact_backup_name
            .toLowerCase()
            .includes(keyword.toLowerCase()) ||
          item.backup_set_id.includes(keyword);
        return (
          matchesKeyword &&
          (!availability || item.availability === availability)
        );
      }),
    [availability, backups.data?.items, keyword]
  );
  const detailSummary = backups.data?.items.find(
    (item) => item.backup_set_id === detailId
  );

  const closeRestore = () => {
    setRestoreTarget(undefined);
    setPreflight(undefined);
    setPassword('');
    setExactName('');
    setRecoveryBackupPassword('');
    setRecoveryJobId(undefined);
  };
  const openRestore = (item: BackupSetSummaryResponse) => {
    setRestoreTarget(item);
    setExactName('');
    setPreflight(undefined);
    setRecoveryJobId(undefined);
    preflightMutation.mutate({ id: item.backup_set_id });
  };

  return (
    <SettingsSectionSurface
      toolbar={
        <Flex gap={8} justify="space-between" wrap>
          <Space wrap>
            <Input.Search
              aria-label={t('search')}
              allowClear
              placeholder={t('search')}
              value={keyword}
              onChange={(event) => setKeyword(event.target.value)}
            />
            <Select
              allowClear
              aria-label={t('availability')}
              options={['ready', 'corrupt', 'incompatible'].map((value) => ({
                value,
                label: availabilityLabel(value)
              }))}
              placeholder={t('availability')}
              value={availability}
              onChange={setAvailability}
            />
          </Space>
          <Space wrap>
            <Upload
              accept=".1fb-backup,application/octet-stream"
              beforeUpload={(file) => {
                setPendingImport(file);
                return false;
              }}
              maxCount={1}
              showUploadList={false}
            >
              <Button icon={<UploadOutlined />}>{t('import')}</Button>
            </Upload>
            <Button
              icon={<ReloadOutlined />}
              onClick={() => void backups.refetch()}
            >
              {t('refresh')}
            </Button>
            <Button
              type="primary"
              icon={<PlusOutlined />}
              loading={createMutation.isPending}
              onClick={() => setCreateOpen(true)}
            >
              {t('create')}
            </Button>
          </Space>
        </Flex>
      }
    >
      {backupJobId ? (
        <Alert
          showIcon
          type={
            backupJobStatus.data?.status === 'failed'
              ? 'error'
              : backupJobStatus.data?.status === 'succeeded'
                ? 'success'
                : 'info'
          }
          title={
            backupJobStatus.data?.status === 'failed'
              ? t('backup_failed')
              : backupJobStatus.data?.status === 'succeeded'
                ? t('backup_succeeded')
                : t('backup_started')
          }
          description={
            <Descriptions
              column={{ xs: 1, sm: 2 }}
              size="small"
              items={[
                {
                  key: 'backup_job_id',
                  label: t('backup_job_id'),
                  children: backupJobId
                },
                {
                  key: 'backup_job_status',
                  label: t('backup_job_status'),
                  children: backupJobStatus.data?.status ?? '—'
                },
                {
                  key: 'sealed_components',
                  label: t('sealed_components'),
                  children: backupJobStatus.data?.sealed_components ?? '—'
                },
                ...(backupJobStatus.data?.status === 'failed'
                  ? [
                      {
                        key: 'failure_code',
                        label: t('failure_code'),
                        children: backupJobStatus.data.failure_code ?? '—'
                      }
                    ]
                  : [])
              ]}
            />
          }
        />
      ) : null}
      <Table<BackupSetSummaryResponse>
        className="system-backups__table"
        dataSource={filtered}
        loading={backups.isLoading}
        pagination={{ pageSize: 20 }}
        rowKey="backup_set_id"
        scroll={{ x: 980 }}
        onRow={(record) => ({
          onClick: () => setDetailId(record.backup_set_id)
        })}
        columns={[
          {
            title: t('name'),
            dataIndex: 'exact_backup_name',
            width: 260,
            ellipsis: true
          },
          {
            title: t('status'),
            dataIndex: 'availability',
            width: 120,
            render: (value: string) => (
              <Tag color={value === 'ready' ? 'green' : 'red'}>
                {availabilityLabel(value)}
              </Tag>
            )
          },
          {
            title: t('size'),
            dataIndex: 'total_size_bytes',
            width: 120,
            render: formatBytes
          },
          {
            title: t('created_at'),
            dataIndex: 'created_at',
            width: 190,
            render: (value: string) => formatDateTime(value)
          },
          {
            title: t('digest'),
            dataIndex: 'envelope_digest',
            width: 220,
            ellipsis: true,
            render: (value: string | null) => value ?? '—'
          },
          {
            title: t('actions'),
            key: 'actions',
            fixed: 'right',
            width: 72,
            render: (_, item) => (
              <Dropdown
                menu={{
                  items: [
                    {
                      key: 'verify',
                      icon: <SafetyCertificateOutlined />,
                      label: t('verify'),
                      onClick: () => setVerifyTarget(item)
                    },
                    {
                      key: 'download',
                      icon: <DownloadOutlined />,
                      label: t('download'),
                      onClick: () => startDirectDownload(item.backup_set_id)
                    },
                    {
                      key: 'restore',
                      danger: true,
                      icon: <WarningOutlined />,
                      label: t('restore'),
                      onClick: () => openRestore(item)
                    },
                    { type: 'divider' },
                    {
                      key: 'delete',
                      danger: true,
                      icon: <DeleteOutlined />,
                      label: t('delete'),
                      onClick: () =>
                        modal.confirm({
                          title: t('delete_confirm'),
                          okButtonProps: { danger: true },
                          onOk: () =>
                            deleteMutation.mutateAsync(item.backup_set_id)
                        })
                    }
                  ]
                }}
                trigger={['click']}
              >
                <Button
                  aria-label={t('actions')}
                  icon={<MoreOutlined />}
                  type="text"
                  onClick={(event) => event.stopPropagation()}
                />
              </Dropdown>
            )
          }
        ]}
      />

      <Drawer
        open={Boolean(detailId)}
        title={t('details')}
        width={640}
        onClose={() => setDetailId(undefined)}
      >
        {detail.data ? (
          <>
            <Descriptions
              column={1}
              bordered
              size="small"
              items={[
                {
                  key: 'id',
                  label: t('name'),
                  children: detail.data.exact_backup_name
                },
                {
                  key: 'backup_set_id',
                  label: 'ID',
                  children: detail.data.backup_set_id
                },
                {
                  key: 'availability',
                  label: t('status'),
                  children: detailSummary
                    ? availabilityLabel(detailSummary.availability)
                    : '—'
                },
                {
                  key: 'created_at',
                  label: t('created_at'),
                  children: detailSummary
                    ? formatDateTime(detailSummary.created_at)
                    : '—'
                },
                {
                  key: 'size',
                  label: t('size'),
                  children: detailSummary
                    ? formatBytes(detailSummary.total_size_bytes)
                    : '—'
                },
                {
                  key: 'digest',
                  label: t('digest'),
                  children: detailSummary?.envelope_digest ?? '—'
                }
              ]}
            />
            <Typography.Title level={5}>{t('content_scope')}</Typography.Title>
            <Descriptions
              column={{ xs: 1, sm: 2 }}
              size="small"
              items={[
                {
                  key: 'components',
                  label: t('component_count'),
                  children: detail.data.content.component_count
                },
                {
                  key: 'postgresql',
                  label: t('postgresql_count'),
                  children: detail.data.content.postgresql_count
                },
                {
                  key: 'objects',
                  label: t('business_objects'),
                  children: detail.data.content.business_object_count
                },
                {
                  key: 'extensions',
                  label: t('extension_artifacts'),
                  children: detail.data.content.extension_artifact_count
                },
                {
                  key: 'mcp',
                  label: t('mcp_artifacts'),
                  children: detail.data.content.mcp_artifact_count
                },
                {
                  key: 'excluded',
                  label: t('excluded_domains'),
                  children:
                    detail.data.content.excluded_domains.join(', ') || '—'
                }
              ]}
            />
            <Alert
              showIcon
              type={detail.data.compatibility.compatible ? 'success' : 'error'}
              title={
                detail.data.compatibility.compatible
                  ? t('detail_compatible')
                  : t('detail_incompatible')
              }
              description={
                detail.data.compatibility.failures.join(', ') ||
                t('compatibility_passed')
              }
            />
            <Descriptions
              column={1}
              size="small"
              items={[
                {
                  key: 'build',
                  label: t('application_build'),
                  children: detail.data.compatibility.application_build
                },
                {
                  key: 'migration',
                  label: t('migration_head'),
                  children: detail.data.compatibility.migration_head
                },
                {
                  key: 'verification',
                  label: t('verification_result'),
                  children:
                    detail.data.verification.verified === null
                      ? t('not_verified')
                      : detail.data.verification.verified
                        ? t('verified')
                        : t('verification_failed')
                },
                {
                  key: 'checked',
                  label: t('verification_time'),
                  children: detail.data.verification.checked_at
                    ? formatDateTime(detail.data.verification.checked_at)
                    : '—'
                }
              ]}
            />
            <Typography.Title level={5}>{t('components')}</Typography.Title>
            <Table
              dataSource={detail.data.components}
              pagination={false}
              rowKey="component_id"
              scroll={{ x: 640 }}
              size="small"
              columns={[
                {
                  title: t('component'),
                  dataIndex: 'component_id',
                  ellipsis: true
                },
                { title: t('kind'), dataIndex: 'kind', width: 140 },
                {
                  title: t('size'),
                  dataIndex: 'size_bytes',
                  width: 100,
                  render: formatBytes
                },
                {
                  title: t('digest'),
                  dataIndex: 'content_digest',
                  ellipsis: true
                }
              ]}
            />
            <Typography.Title level={5}>{t('creation_log')}</Typography.Title>
            <Table
              dataSource={detail.data.creation_journal}
              pagination={false}
              rowKey="sequence"
              size="small"
              columns={[
                { title: '#', dataIndex: 'sequence', width: 56 },
                {
                  title: t('created_at'),
                  dataIndex: 'occurred_at',
                  render: (value: string) => formatDateTime(value)
                },
                {
                  title: t('journal_state'),
                  dataIndex: 'state',
                  render: (value: string | null) => value ?? '—'
                },
                {
                  title: t('failure_code'),
                  dataIndex: 'failure_code',
                  render: (value: string | null) => value ?? '—'
                }
              ]}
            />
            <Typography.Title level={5}>
              {t('recovery_history')}
            </Typography.Title>
            <Table
              dataSource={detail.data.recovery_history}
              pagination={false}
              rowKey="recovery_job_id"
              size="small"
              columns={[
                { title: 'ID', dataIndex: 'recovery_job_id', ellipsis: true },
                {
                  title: t('journal_state'),
                  dataIndex: 'status',
                  render: (value: string | null) => value ?? '—'
                },
                {
                  title: t('created_at'),
                  dataIndex: 'started_at',
                  render: (value: string) => formatDateTime(value)
                }
              ]}
            />
          </>
        ) : null}
      </Drawer>

      <Modal
        destroyOnHidden
        open={createOpen}
        title={t('create')}
        okText={t('create')}
        confirmLoading={createMutation.isPending}
        onCancel={() => {
          setCreateOpen(false);
          setBackupPassword('');
        }}
        onOk={() => createMutation.mutate(backupPassword || undefined)}
      >
        <Typography.Paragraph>{t('backup_password_help')}</Typography.Paragraph>
        <Input.Password
          autoComplete="new-password"
          placeholder={t('backup_password')}
          value={backupPassword}
          onChange={(event) => setBackupPassword(event.target.value)}
        />
      </Modal>

      <Modal
        destroyOnHidden
        open={Boolean(pendingImport)}
        title={t('import')}
        okText={t('import')}
        confirmLoading={importMutation.isPending}
        onCancel={() => {
          setPendingImport(undefined);
          setImportPassword('');
        }}
        onOk={() => {
          if (pendingImport) {
            importMutation.mutate({
              file: pendingImport,
              password: importPassword || undefined
            });
          }
        }}
      >
        <Typography.Paragraph>{t('import_password_help')}</Typography.Paragraph>
        <Input.Password
          autoComplete="current-password"
          placeholder={t('backup_password')}
          value={importPassword}
          onChange={(event) => setImportPassword(event.target.value)}
        />
      </Modal>

      <Modal
        destroyOnHidden
        open={Boolean(verifyTarget)}
        title={t('verify')}
        okText={t('verify')}
        confirmLoading={verifyMutation.isPending}
        onCancel={() => {
          setVerifyTarget(undefined);
          setVerifyPassword('');
        }}
        onOk={() => {
          if (verifyTarget) {
            verifyMutation.mutate({
              id: verifyTarget.backup_set_id,
              password: verifyPassword || undefined
            });
          }
        }}
      >
        <Typography.Paragraph>{t('verify_password_help')}</Typography.Paragraph>
        <Input.Password
          autoComplete="current-password"
          placeholder={t('backup_password')}
          value={verifyPassword}
          onChange={(event) => setVerifyPassword(event.target.value)}
        />
      </Modal>

      <Modal
        className="system-backups__restore"
        destroyOnHidden
        footer={null}
        open={Boolean(restoreTarget)}
        title={t('restore_title')}
        width={760}
        onCancel={closeRestore}
      >
        <Steps
          current={recoveryJobId ? 2 : preflight ? 1 : 0}
          items={[
            { title: t('step_preflight') },
            { title: t('step_confirm') },
            { title: t('step_status') }
          ]}
        />
        <div className="system-backups__restore-body">
          {preflightMutation.isPending ? (
            <Typography.Text>{t('preflighting')}</Typography.Text>
          ) : null}
          {preflight && !recoveryJobId ? (
            <>
              <Alert
                showIcon
                type={preflight.compatible ? 'success' : 'error'}
                title={
                  preflight.compatible ? t('compatible') : t('incompatible')
                }
                description={
                  preflight.failures.join(', ') || t('preflight_passed')
                }
              />
              <Input.Password
                autoComplete="current-password"
                placeholder={t('backup_password')}
                value={recoveryBackupPassword}
                onChange={(event) =>
                  setRecoveryBackupPassword(event.target.value)
                }
              />
              <Button
                onClick={() =>
                  restoreTarget &&
                  preflightMutation.mutate({
                    id: restoreTarget.backup_set_id,
                    password: recoveryBackupPassword || undefined
                  })
                }
              >
                {t('step_preflight')}
              </Button>
              <Descriptions
                className="system-backups__impact"
                column={{ xs: 1, sm: 2 }}
                size="small"
                items={[
                  {
                    key: 'db',
                    label: t('database_replaced'),
                    children: preflight.impact.database_replaced
                      ? t('yes')
                      : t('no')
                  },
                  {
                    key: 'objects',
                    label: t('business_objects'),
                    children: preflight.impact.business_object_count
                  },
                  {
                    key: 'extensions',
                    label: t('extension_artifacts'),
                    children: preflight.impact.extension_artifact_count
                  },
                  {
                    key: 'mcp',
                    label: t('mcp_artifacts'),
                    children: preflight.impact.mcp_artifact_count
                  },
                  {
                    key: 'required',
                    label: t('required_space'),
                    children: formatBytes(preflight.required_space_bytes)
                  },
                  {
                    key: 'available',
                    label: t('available_space'),
                    children: formatBytes(preflight.available_space_bytes)
                  }
                ]}
              />
              <Alert showIcon type="warning" title={t('danger_notice')} />
              <Input.Password
                autoComplete="current-password"
                placeholder={t('password')}
                value={password}
                onChange={(event) => setPassword(event.target.value)}
              />
              <Input
                placeholder={restoreTarget?.exact_backup_name}
                value={exactName}
                onChange={(event) => setExactName(event.target.value)}
              />
              <Flex justify="end">
                <Button
                  danger
                  type="primary"
                  disabled={
                    !preflight.compatible ||
                    !password ||
                    exactName !== restoreTarget?.exact_backup_name
                  }
                  loading={intentMutation.isPending}
                  onClick={() => intentMutation.mutate()}
                >
                  {t('confirm_restore')}
                </Button>
              </Flex>
            </>
          ) : null}
          {recoveryJobId ? (
            <>
              <Alert
                showIcon
                type="info"
                title={t('recovery_started')}
                description={recoveryJobId}
              />
              <Descriptions
                column={1}
                bordered
                size="small"
                items={[
                  {
                    key: 'phase',
                    label: t('phase'),
                    children: recoveryStatus.data?.phase ?? '—'
                  },
                  {
                    key: 'journal_state',
                    label: t('journal_state'),
                    children: recoveryStatus.data?.journal_state ?? '—'
                  },
                  {
                    key: 'writes',
                    label: t('active_writes'),
                    children: recoveryStatus.data?.active_write_count ?? '—'
                  },
                  {
                    key: 'safety',
                    label: t('safety_backup'),
                    children: recoveryStatus.data?.safety_backup_set_id ?? '—'
                  }
                ]}
              />
            </>
          ) : null}
        </div>
      </Modal>
    </SettingsSectionSurface>
  );
}
