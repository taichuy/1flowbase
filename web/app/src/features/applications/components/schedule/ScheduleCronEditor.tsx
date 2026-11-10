import { Flex, Form, Input, InputNumber, Select, TimePicker } from 'antd';
import dayjs from 'dayjs';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  DEFAULT_SCHEDULE_PATTERN,
  parseScheduleCron,
  serializeScheduleCron,
  type ScheduleFrequency,
  type SchedulePattern
} from './schedule-cron';

export function ScheduleCronEditor({
  id,
  value = '',
  onChange
}: {
  id?: string;
  value?: string;
  onChange?: (value: string) => void;
}) {
  const { t } = useTranslation('applications');
  const controlId = useId();
  const [customMode, setCustomMode] = useState(false);
  const parsed = parseScheduleCron(value);
  const pattern = parsed ?? DEFAULT_SCHEDULE_PATTERN;
  const frequency = customMode || !parsed ? 'custom' : pattern.frequency;
  const visual = frequency !== 'custom';
  const hasTime = ['daily', 'weekly', 'monthly'].includes(frequency);
  const update = (patch: Partial<SchedulePattern>) => {
    setCustomMode(false);
    onChange?.(serializeScheduleCron({ ...pattern, ...patch }));
  };
  const weekdays = [
    { value: 1, label: t('schedule.monday') },
    { value: 2, label: t('schedule.tuesday') },
    { value: 3, label: t('schedule.wednesday') },
    { value: 4, label: t('schedule.thursday') },
    { value: 5, label: t('schedule.friday') },
    { value: 6, label: t('schedule.saturday') },
    { value: 0, label: t('schedule.sunday') }
  ];

  return (
    <Flex vertical gap={12}>
      <Select<ScheduleFrequency | 'custom'>
        id={id}
        aria-label={t('schedule.frequency')}
        value={frequency}
        options={[
          { value: 'minutes', label: t('schedule.by_minutes') },
          { value: 'hours', label: t('schedule.by_hours') },
          { value: 'daily', label: t('schedule.daily') },
          { value: 'weekly', label: t('schedule.weekly') },
          { value: 'monthly', label: t('schedule.monthly') },
          { value: 'custom', label: t('schedule.custom') }
        ]}
        onChange={(next) => {
          if (next === 'custom') {
            setCustomMode(true);
          } else {
            update({ frequency: next, interval: 1 });
          }
        }}
      />
      {frequency === 'custom' && (
        <Input
          aria-label={t('auto.schedule_cron')}
          value={value}
          placeholder="0 9 * * 1-5"
          onChange={(event) => {
            setCustomMode(true);
            onChange?.(event.target.value);
          }}
        />
      )}
      {visual && (
        <Flex gap={12} wrap>
          {(frequency === 'minutes' || frequency === 'hours') && (
            <Form.Item
              label={frequency === 'minutes' ? t('schedule.minute_interval') : t('schedule.hour_interval')}
              tooltip={frequency === 'minutes' ? t('schedule.minute_interval_hint') : t('schedule.hour_interval_hint')}
              htmlFor={`${controlId}-interval`}
              style={{ flex: '1 1 160px', marginBottom: 0 }}
            >
              <InputNumber
                id={`${controlId}-interval`}
                aria-label={frequency === 'minutes' ? t('schedule.minute_interval') : t('schedule.hour_interval')}
                min={1}
                max={frequency === 'minutes' ? 59 : 23}
                precision={0}
                value={pattern.interval}
                style={{ width: '100%' }}
                onChange={(next) => {
                  if (next !== null && next >= 1 && next <= (frequency === 'minutes' ? 59 : 23)) {
                    update({ interval: next });
                  }
                }}
              />
            </Form.Item>
          )}
          {frequency === 'hours' && (
            <Form.Item
              label={t('schedule.minute_of_hour')}
              htmlFor={`${controlId}-minute`}
              style={{ flex: '1 1 160px', marginBottom: 0 }}
            >
              <InputNumber
                id={`${controlId}-minute`}
                min={0}
                max={59}
                precision={0}
                value={pattern.minute}
                style={{ width: '100%' }}
                onChange={(next) => {
                  if (next !== null && next >= 0 && next <= 59) update({ minute: next });
                }}
              />
            </Form.Item>
          )}
          {frequency === 'weekly' && (
            <Form.Item
              label={t('schedule.weekdays')}
              htmlFor={`${controlId}-weekdays`}
              style={{ flex: '1 1 100%', marginBottom: 0 }}
            >
              <Select
                id={`${controlId}-weekdays`}
                mode="multiple"
                value={pattern.weekdays}
                options={weekdays}
                onChange={(next: number[]) => {
                  if (next.length) update({ weekdays: next });
                }}
              />
            </Form.Item>
          )}
          {frequency === 'monthly' && (
            <Form.Item
              label={t('schedule.month_day')}
              tooltip={t('schedule.month_day_hint')}
              htmlFor={`${controlId}-month-day`}
              style={{ flex: '1 1 160px', marginBottom: 0 }}
            >
              <Select
                id={`${controlId}-month-day`}
                aria-label={t('schedule.month_day')}
                value={pattern.monthDay}
                options={Array.from({ length: 31 }, (_, index) => ({
                  value: index + 1,
                  label: t('schedule.day_of_month', { day: index + 1 })
                }))}
                onChange={(monthDay) => update({ monthDay })}
              />
            </Form.Item>
          )}
          {hasTime && (
            <Form.Item
              label={t('schedule.time')}
              htmlFor={`${controlId}-time`}
              style={{ flex: '1 1 160px', marginBottom: 0 }}
            >
              <TimePicker
                id={`${controlId}-time`}
                format="HH:mm"
                allowClear={false}
                needConfirm={false}
                showNow={false}
                value={dayjs('2000-01-01').hour(pattern.hour).minute(pattern.minute)}
                style={{ width: '100%' }}
                onChange={(time) => {
                  if (time) update({ hour: time.hour(), minute: time.minute() });
                }}
              />
            </Form.Item>
          )}
        </Flex>
      )}
    </Flex>
  );
}
