import { DatePicker, Grid, Space } from 'antd';
import type { Dayjs } from 'dayjs';
import { useTranslation } from 'react-i18next';

type DateRangeDraft = [Dayjs | null, Dayjs | null];
export function LogDeleteDateRangeInput({
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
