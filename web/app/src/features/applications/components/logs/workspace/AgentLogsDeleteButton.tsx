import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import {
  Alert,
  Button,
  DatePicker,
  Form,
  Grid,
  InputNumber,
  Modal,
  Select,
  Space,
  Typography
} from 'antd';
import dayjs, { type Dayjs } from 'dayjs';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { AgentLogsDeleteScope } from '@1flowbase/api-client';
import { useAuthStore } from '../../../../../state/auth-store';
import { deleteApplicationLogsInBatches } from '../../../api/log-deletion';

type Values = {
  date: '7' | '30' | '90' | '365' | 'all' | 'custom';
  dates?: [Dayjs, Dayjs];
  batch_size: number;
};
type Status = 'ready' | 'running' | 'complete' | 'stopped' | 'failed';

type DateRangeDraft = [Dayjs | null, Dayjs | null];
function LogDeleteDateRangeInput({
  value,
  onChange
}: {
  value?: DateRangeDraft;
  onChange?: (value: DateRangeDraft | null) => void;
}) {
  const { sm } = Grid.useBreakpoint();
  const { t } = useTranslation('applications');
  if (sm) {
    return (
      <DatePicker.RangePicker
        value={value}
        onChange={onChange}
        style={{ width: '100%' }}
        allowClear
      />
    );
  }
  // A single native calendar fits narrow screens without hiding either month.
  return (
    <Space orientation="vertical" style={{ width: '100%' }}>
      <DatePicker
        placeholder={t('log_deletion.start_date')}
        aria-label={t('log_deletion.start_date')}
        value={value?.[0]}
        style={{ width: '100%' }}
        onChange={(date) => onChange?.([date, value?.[1] ?? null])}
      />
      <DatePicker
        placeholder={t('log_deletion.end_date')}
        aria-label={t('log_deletion.end_date')}
        value={value?.[1]}
        style={{ width: '100%' }}
        onChange={(date) => onChange?.([value?.[0] ?? null, date])}
      />
    </Space>
  );
}

export function AgentLogsDeleteButton({
  applicationId,
  onFinished
}: {
  applicationId: string;
  onFinished: () => Promise<void>;
}) {
  const { t } = useTranslation('applications');
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const [open, setOpen] = useState(false);
  const [form] = Form.useForm<Values>();
  const date = Form.useWatch('date', form);
  const [status, setStatus] = useState<Status>('ready');
  const [deleted, setDeleted] = useState(0);
  const [stopRequested, setStopRequested] = useState(false);
  const stop = useRef(false);
  const mounted = useRef(true);
  const running = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      stop.current = true;
    };
  }, []);

  async function submit(values: Values) {
    if (!csrfToken || running.current) return;
    running.current = true;
    stop.current = false;
    setStatus('running');
    const now = dayjs();
    const scope: AgentLogsDeleteScope =
      values.date === 'all'
        ? { mode: 'all_time', batch_size: values.batch_size }
        : {
            mode: 'time_range',
            started_at_from: (values.date === 'custom'
              ? values.dates![0].startOf('day')
              : now.subtract(Number(values.date), 'day')
            ).toISOString(),
            started_at_to: (values.date === 'custom'
              ? values.dates![1].startOf('day').add(1, 'day')
              : now
            ).toISOString(),
            batch_size: values.batch_size
          };
    try {
      const result = await deleteApplicationLogsInBatches(
        applicationId,
        scope,
        csrfToken,
        (receipt) => {
          if (mounted.current)
            setDeleted((count) => count + receipt.deleted_records);
        },
        () => stop.current
      );
      if (mounted.current) setStatus(result);
    } catch {
      if (mounted.current) setStatus('failed');
    } finally {
      // A failed response can follow a committed batch; always reload authoritative data.
      try {
        if (mounted.current) await onFinished();
      } finally {
        running.current = false;
      }
    }
  }
  function show() {
    form.resetFields();
    setDeleted(0);
    setStatus('ready');
    setStopRequested(false);
    setOpen(true);
  }
  const processing = status === 'running';
  return (
    <>
      <Button
        danger
        icon={<DeleteOutlined aria-hidden="true" />}
        onClick={show}
        disabled={!csrfToken}
      >
        {t('log_deletion.button')}
      </Button>
      <Modal
        title={t('log_deletion.button')}
        open={open}
        closable={!processing}
        mask={{ closable: !processing }}
        keyboard={!processing}
        onCancel={() => setOpen(false)}
        footer={
          <Space>
            {processing ? (
              <Button
                disabled={stopRequested}
                onClick={() => {
                  stop.current = true;
                  setStopRequested(true);
                }}
              >
                {t(
                  stopRequested ? 'log_deletion.stopping' : 'log_deletion.stop'
                )}
              </Button>
            ) : (
              <Button onClick={() => setOpen(false)}>
                {t(status === 'ready' ? 'auto.cancel' : 'auto.close')}
              </Button>
            )}
            {status === 'ready' && (
              <Button
                danger
                type="primary"
                disabled={!csrfToken}
                onClick={() => form.submit()}
              >
                {t('log_deletion.button')}
              </Button>
            )}
          </Space>
        }
      >
        <Form
          form={form}
          layout="vertical"
          initialValues={{ date: '7', batch_size: 100 }}
          onFinish={(values) => {
            void submit(values);
          }}
          disabled={status !== 'ready'}
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
                validator: (_, value: DateRangeDraft | undefined) =>
                  form.getFieldValue('date') !== 'custom' ||
                  (value?.length === 2 &&
                    value.every((item) => item?.isValid()) &&
                    value[0] !== null &&
                    value[1] !== null &&
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
                    : Promise.reject(new Error(t('log_deletion.batch_invalid')))
              }
            ]}
          >
            <InputNumber min={1} style={{ width: '100%' }} />
          </Form.Item>
        </Form>
        <Alert type="warning" showIcon title={t('log_deletion.warning')} />
        {status !== 'ready' && (
          <div role="status" style={{ marginTop: 16 }}>
            <Typography.Paragraph>
              {t('log_deletion.progress', { count: deleted })}
            </Typography.Paragraph>
            {processing ? (
              <Typography.Text>
                {t(
                  stopRequested
                    ? 'log_deletion.stopping_detail'
                    : 'log_deletion.running'
                )}
              </Typography.Text>
            ) : (
              <Alert
                type={
                  status === 'failed'
                    ? 'error'
                    : status === 'complete'
                      ? 'success'
                      : 'info'
                }
                showIcon
                title={t(
                  status === 'failed'
                    ? 'log_deletion.failed'
                    : status === 'complete'
                      ? 'log_deletion.complete'
                      : 'log_deletion.stopped'
                )}
              />
            )}
          </div>
        )}
      </Modal>
    </>
  );
}
