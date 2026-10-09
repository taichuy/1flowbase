import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import {
  Alert,
  Button,
  Descriptions,
  Form,
  InputNumber,
  Modal,
  Progress,
  Select,
  Space,
  Typography
} from 'antd';
import dayjs, { type Dayjs } from 'dayjs';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { ApiClientError } from '@1flowbase/api-client';
import type {
  AgentLogsDeleteJob,
  AgentLogsDeleteJobCreate,
  AgentLogsDeleteScope
} from '@1flowbase/api-client';
import { useAuthStore } from '../../../../../state/auth-store';
import {
  fetchApplicationLogDeletionJob,
  logDeletionJobKey,
  logDeletionPreviewKey,
  previewApplicationLogDeletion,
  startApplicationLogDeletion,
  stopApplicationLogDeletion
} from '../../../api/log-deletion';
import { LogDeleteDateRangeInput } from './LogDeleteDateRangeInput';

type Values = {
  date: '7' | '30' | '90' | '365' | 'all' | 'custom';
  dates?: [Dayjs | null, Dayjs | null];
  batch_size: number;
};
const active = (job: AgentLogsDeleteJob | null | undefined) =>
  job?.status === 'queued' || job?.status === 'running';
export function AgentLogsDeleteButton({
  applicationId,
  onFinished
}: {
  applicationId: string;
  onFinished: () => Promise<void>;
}) {
  const { t } = useTranslation('applications');
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const [form] = Form.useForm<Values>();
  const date = Form.useWatch('date', form);
  const dates = Form.useWatch('dates', form);
  const [anchor, setAnchor] = useState(() => dayjs());
  const [trackedId, setTrackedId] = useState<string>();
  const [pendingInput, setPendingInput] = useState<AgentLogsDeleteJobCreate>();
  const completed = useRef<string | undefined>(undefined);
  const observed = useQuery({
    queryKey: logDeletionJobKey(applicationId, trackedId),
    queryFn: ({ signal }) =>
      fetchApplicationLogDeletionJob(applicationId, trackedId, signal),
    refetchInterval: (query) => {
      if (
        query.state.error instanceof ApiClientError &&
        [401, 403].includes(query.state.error.status)
      )
        return false;
      return (trackedId && !query.state.data) ||
        active(query.state.data) ||
        (open && !!query.state.error)
        ? 1500
        : false;
    },
    retry: false,
    // Keep polling when the modal closes; the component does not execute deletion.
    refetchOnWindowFocus: true
  });
  const job = trackedId
    ? observed.data
    : active(observed.data)
      ? observed.data
      : null;
  useEffect(() => {
    if (!trackedId && active(observed.data))
      setTrackedId(observed.data!.job_id);
  }, [observed.data, trackedId]);
  useEffect(() => {
    if (job && !active(job) && completed.current !== job.job_id) {
      completed.current = job.job_id;
      setPendingInput(undefined);
      void onFinished();
    }
  }, [job, onFinished]);
  const scope = useMemo<AgentLogsDeleteScope | null>(() => {
    if (!date) return null;
    if (date === 'all') return { mode: 'all_time' };
    if (
      date === 'custom' &&
      (!dates?.[0]?.isValid() ||
        !dates?.[1]?.isValid() ||
        dates[0].isAfter(dates[1], 'day'))
    )
      return null;
    return {
      mode: 'time_range',
      started_at_from: (date === 'custom'
        ? dates![0]!.startOf('day')
        : anchor.subtract(Number(date), 'day')
      ).toISOString(),
      started_at_to: (date === 'custom'
        ? dates![1]!.startOf('day').add(1, 'day')
        : anchor
      ).toISOString()
    };
  }, [date, dates, anchor]);
  const preview = useQuery({
    queryKey: logDeletionPreviewKey(applicationId, scope),
    queryFn: ({ signal }) =>
      previewApplicationLogDeletion(applicationId, scope!, signal),
    enabled: open && !trackedId && !job && !!scope,
    retry: false,
    staleTime: 0
  });
  const start = useMutation({
    mutationFn: (input: AgentLogsDeleteJobCreate) =>
      startApplicationLogDeletion(applicationId, input, csrfToken!),
    onSuccess: async (value) => {
      await queryClient.cancelQueries({
        queryKey: logDeletionJobKey(applicationId, value.job_id)
      });
      queryClient.setQueryData(
        logDeletionJobKey(applicationId, value.job_id),
        value
      );
      setPendingInput(undefined);
    },
    onError: (error) => {
      if (
        error instanceof ApiClientError &&
        error.status >= 400 &&
        error.status < 500 &&
        error.status !== 408 &&
        error.status !== 429
      ) {
        setPendingInput(undefined);
        setTrackedId(undefined);
        void queryClient.invalidateQueries({
          queryKey: logDeletionJobKey(applicationId)
        });
      }
    }
  });
  const stop = useMutation({
    mutationFn: () =>
      stopApplicationLogDeletion(applicationId, job!.job_id, csrfToken!),
    onSuccess: async (value) => {
      await queryClient.cancelQueries({
        queryKey: logDeletionJobKey(applicationId, value.job_id)
      });
      queryClient.setQueryData(
        logDeletionJobKey(applicationId, value.job_id),
        value
      );
    }
  });
  function submit(values: Values) {
    if (
      !scope ||
      !csrfToken ||
      trackedId ||
      !preview.data?.total_records ||
      observed.isPending ||
      observed.isError
    )
      return;
    const input = {
      job_id: crypto.randomUUID(),
      scope: { ...scope, batch_size: values.batch_size }
    };
    setPendingInput(input);
    setTrackedId(input.job_id);
    start.mutate(input);
  }
  function reset() {
    // The inactive latest query may still contain an earlier running snapshot.
    queryClient.removeQueries({
      queryKey: logDeletionJobKey(applicationId),
      exact: true
    });
    setTrackedId(undefined);
    setPendingInput(undefined);
    start.reset();
    stop.reset();
    setAnchor(dayjs());
    form.resetFields();
    void queryClient.invalidateQueries({
      queryKey: logDeletionJobKey(applicationId)
    });
    void queryClient.invalidateQueries({
      queryKey: ['applications', applicationId, 'log-deletion-preview']
    });
  }
  const processing = active(job) || !!pendingInput;
  const percent = job
    ? job.total_records === 0
      ? job.status === 'succeeded'
        ? 100
        : 0
      : Math.floor((job.deleted_records / job.total_records) * 100)
    : 0;
  return (
    <>
      <Button
        danger
        icon={<DeleteOutlined aria-hidden="true" />}
        onClick={() => {
          setAnchor(dayjs());
          setOpen(true);
        }}
        disabled={!csrfToken}
      >
        {t('log_deletion.button')}
      </Button>
      <Modal
        title={t('log_deletion.button')}
        open={open}
        onCancel={() => setOpen(false)}
        footer={
          <Space wrap>
            <Button onClick={() => setOpen(false)}>{t('auto.close')}</Button>
            {active(job) && (
              <Button
                disabled={job!.stop_requested || stop.isPending || !csrfToken}
                onClick={() => stop.mutate()}
              >
                {t(
                  job!.stop_requested
                    ? 'log_deletion.stopping'
                    : 'log_deletion.stop'
                )}
              </Button>
            )}
            {job && !active(job) && (
              <Button onClick={reset}>{t('log_deletion.new_deletion')}</Button>
            )}
            {pendingInput && start.isError && (
              <Button
                onClick={() => start.mutate(pendingInput)}
                loading={start.isPending}
                disabled={!csrfToken}
              >
                {t('log_deletion.confirm_start')}
              </Button>
            )}
            {!trackedId && !job && (
              <Button
                danger
                type="primary"
                disabled={
                  !csrfToken ||
                  !scope ||
                  !preview.data?.total_records ||
                  preview.isFetching ||
                  preview.isError ||
                  observed.isPending ||
                  observed.isError
                }
                onClick={() => form.submit()}
              >
                {t('log_deletion.button')}
              </Button>
            )}
          </Space>
        }
      >
        {job ? (
          <Descriptions
            size="small"
            column={1}
            items={[
              {
                key: 'scope',
                label: t('log_deletion.date'),
                children:
                  job.scope.mode === 'all_time'
                    ? t('auto.all')
                    : t('log_deletion.fixed_range', {
                        started_at_from: dayjs(
                          job.scope.started_at_from
                        ).format('YYYY-MM-DD HH:mm:ss'),
                        started_at_to: dayjs(job.scope.started_at_to).format(
                          'YYYY-MM-DD HH:mm:ss'
                        )
                      })
              },
              {
                key: 'batch_size',
                label: t('log_deletion.batch_size'),
                children: job.scope.batch_size
              }
            ]}
          />
        ) : (
          <Form
            form={form}
            layout="vertical"
            initialValues={{ date: '7', batch_size: 100 }}
            onFinish={submit}
            disabled={processing || !!job}
          >
            <Form.Item
              label={t('log_deletion.date')}
              name="date"
              rules={[{ required: true }]}
            >
              <Select
                options={[
                  { value: '7', label: t('auto.past_seven_days') },
                  { value: '30', label: t('log_deletion.past_thirty_days') },
                  { value: '90', label: t('log_deletion.past_ninety_days') },
                  { value: '365', label: t('log_deletion.year') },
                  { value: 'all', label: t('auto.all') },
                  { value: 'custom', label: t('log_deletion.custom') }
                ]}
              />
            </Form.Item>
            <Form.Item
              hidden={date !== 'custom'}
              label={t('log_deletion.custom_range')}
              name="dates"
              rules={[
                {
                  validator: (_, value: Values['dates']) =>
                    form.getFieldValue('date') !== 'custom' ||
                    (value?.[0]?.isValid() &&
                      value?.[1]?.isValid() &&
                      !value[0].isAfter(value[1], 'day'))
                      ? Promise.resolve()
                      : Promise.reject(
                          new Error(t('log_deletion.range_required'))
                        )
                }
              ]}
            >
              <LogDeleteDateRangeInput />
            </Form.Item>
            {!trackedId && !job && (
              <Typography.Paragraph role="status">
                {!scope
                  ? t('log_deletion.range_required')
                  : preview.isFetching
                    ? t('log_deletion.counting')
                    : preview.isError
                      ? t('log_deletion.count_failed')
                      : preview.data
                        ? t('log_deletion.total', {
                            count: preview.data.total_records
                          })
                        : null}
                {preview.isError && (
                  <Button type="link" onClick={() => void preview.refetch()}>
                    {t('log_deletion.retry_count')}
                  </Button>
                )}
              </Typography.Paragraph>
            )}
            <Form.Item
              label={t('log_deletion.batch_size')}
              name="batch_size"
              rules={[
                {
                  validator: (_, value: unknown) =>
                    typeof value === 'number' &&
                    Number.isInteger(value) &&
                    value > 0 &&
                    value <= 4294967295
                      ? Promise.resolve()
                      : Promise.reject(
                          new Error(t('log_deletion.batch_invalid'))
                        )
                }
              ]}
            >
              <InputNumber min={1} style={{ width: '100%' }} />
            </Form.Item>
          </Form>
        )}
        <Alert type="warning" showIcon title={t('log_deletion.warning')} />
        {job && (
          <div role="status" style={{ marginTop: 16 }}>
            <Typography.Paragraph>
              {t('log_deletion.job_progress', {
                deleted_records: job.deleted_records,
                total_records: job.total_records
              })}
            </Typography.Paragraph>
            <Progress
              percent={percent}
              status={
                job.status === 'failed'
                  ? 'exception'
                  : job.status === 'succeeded'
                    ? 'success'
                    : active(job)
                      ? 'active'
                      : 'normal'
              }
            />
            {active(job) ? (
              <Typography.Text>
                {t(
                  job.stop_requested
                    ? 'log_deletion.stopping_detail'
                    : 'log_deletion.background_running'
                )}
              </Typography.Text>
            ) : (
              <Alert
                showIcon
                type={
                  job.status === 'succeeded'
                    ? 'success'
                    : job.status === 'failed'
                      ? 'error'
                      : 'info'
                }
                title={t(
                  job.status === 'succeeded'
                    ? 'log_deletion.complete'
                    : job.status === 'failed'
                      ? 'log_deletion.job_failed'
                      : 'log_deletion.stopped'
                )}
              />
            )}
          </div>
        )}
        {!job && pendingInput && (
          <Typography.Paragraph role="status" style={{ marginTop: 16 }}>
            {t('log_deletion.confirming_start')}
          </Typography.Paragraph>
        )}
        {(observed.isError || observed.fetchStatus === 'paused') && (
          <Alert
            style={{ marginTop: 16 }}
            type="info"
            showIcon
            title={t(
              observed.error instanceof ApiClientError &&
                [401, 403].includes(observed.error.status)
                ? 'log_deletion.access_failed'
                : 'log_deletion.reconnecting'
            )}
            action={
              <Button onClick={() => void observed.refetch()}>
                {t('log_deletion.retry_query')}
              </Button>
            }
          />
        )}
        {start.isError && !pendingInput && (
          <Alert
            style={{ marginTop: 16 }}
            type="error"
            showIcon
            title={t('log_deletion.start_rejected')}
          />
        )}
        {stop.isError && (
          <Alert
            style={{ marginTop: 16 }}
            type="warning"
            showIcon
            title={t('log_deletion.stop_unconfirmed')}
          />
        )}
      </Modal>
    </>
  );
}
