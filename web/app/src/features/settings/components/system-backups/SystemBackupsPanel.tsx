import {
  createSystemBackup,
  createSystemRecoveryIntent,
  deleteSystemBackup,
  downloadSystemBackup,
  getSystemBackup,
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
import { useMemo, useState } from 'react';
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

function saveDownload(blob: Blob, filename: string | null) {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = filename ?? 'system.1fb-backup';
  anchor.click();
  URL.revokeObjectURL(url);
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
  const [exactName, setExactName] = useState('');
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
  const recoveryStatus = useQuery({
    queryKey: [...queryKey, 'recovery-status', recoveryJobId],
    queryFn: () => getSystemRecoveryStatus(recoveryJobId),
    enabled: Boolean(recoveryJobId),
    refetchInterval: 2000
  });
  const refresh = () => queryClient.invalidateQueries({ queryKey });
  const notifyError = () => message.error(t('operation_failed'));
  const createMutation = useMutation({
    mutationFn: () => createSystemBackup(csrfToken),
    onSuccess: async () => {
      await refresh();
      message.success(t('create_started'));
    },
    onError: notifyError
  });
  const importMutation = useMutation({
    mutationFn: (file: File) => importSystemBackup(file, csrfToken),
    onSuccess: async () => {
      await refresh();
      message.success(t('import_succeeded'));
    },
    onError: notifyError
  });
  const verifyMutation = useMutation({
    mutationFn: (id: string) => verifySystemBackup(id, csrfToken),
    onSuccess: async () => {
      await refresh();
      message.success(t('verify_succeeded'));
    },
    onError: notifyError
  });
  const downloadMutation = useMutation({
    mutationFn: (backupSetId: string) => downloadSystemBackup(backupSetId),
    onSuccess: (value) => saveDownload(value.blob, value.filename),
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
    mutationFn: (id: string) => preflightSystemRecovery(id, csrfToken),
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
          password
        },
        csrfToken
      );
      return createSystemRecoveryIntent(
        restoreTarget.backup_set_id,
        {
          challenge_token: challenge.challenge_token,
          exact_backup_name: exactName,
          plan_digest: preflight.plan_digest
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

  const closeRestore = () => {
    setRestoreTarget(undefined);
    setPreflight(undefined);
    setPassword('');
    setExactName('');
    setRecoveryJobId(undefined);
  };
  const openRestore = (item: BackupSetSummaryResponse) => {
    setRestoreTarget(item);
    setExactName('');
    setPreflight(undefined);
    setRecoveryJobId(undefined);
    preflightMutation.mutate(item.backup_set_id);
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
                importMutation.mutate(file);
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
              onClick={() => createMutation.mutate()}
            >
              {t('create')}
            </Button>
          </Space>
        </Flex>
      }
    >
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
                      onClick: () => verifyMutation.mutate(item.backup_set_id)
                    },
                    {
                      key: 'download',
                      icon: <DownloadOutlined />,
                      label: t('download'),
                      onClick: () => downloadMutation.mutate(item.backup_set_id)
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
                }
              ]}
            />
            <Typography.Title level={5}>{t('manifest')}</Typography.Title>
            <pre className="system-backups__manifest">
              {JSON.stringify(detail.data.sealed_manifest, null, 2)}
            </pre>
          </>
        ) : null}
      </Drawer>

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
