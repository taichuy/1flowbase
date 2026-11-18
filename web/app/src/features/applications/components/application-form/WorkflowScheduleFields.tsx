import { Form, Input, InputNumber, Select, Switch } from 'antd';
import { useTranslation } from 'react-i18next';
import { ScheduleCronEditor } from '../schedule/ScheduleCronEditor';
import type { ApplicationFormFieldsProps } from './ApplicationFormFields';

type WorkflowScheduleFieldsProps = Pick<
  ApplicationFormFieldsProps,
  'isEdit' | 'timezoneOptions' | 'extensionContract'
>;

export function WorkflowScheduleFields({
  isEdit,
  timezoneOptions,
  extensionContract
}: WorkflowScheduleFieldsProps) {
  const { t } = useTranslation('applications');
  return (
    <>
      {isEdit ? (
        <Form.Item
          name="schedule_enabled"
          label={t('auto.schedule_enabled')}
          valuePropName="checked"
        >
          <Switch />
        </Form.Item>
      ) : null}
      <Form.Item
        label={t('schedule.plan')}
        tooltip={!isEdit ? t('auto.schedule_disabled_hint') : undefined}
        name="schedule_cron"
        rules={[{ required: true, message: t('auto.schedule_cron_required') }]}
      >
        <ScheduleCronEditor />
      </Form.Item>
      <Form.Item label={t('auto.schedule_timezone')} name="schedule_timezone">
        <Select
          allowClear
          showSearch
          placeholder={t('auto.schedule_timezone_system')}
          optionFilterProp="label"
          options={timezoneOptions}
        />
      </Form.Item>
      {extensionContract.requestFields.flatMap((field) => {
        if (!['string', 'number', 'boolean'].includes(field.valueType))
          return [];
        return [
          <Form.Item
            key={field.key}
            label={`${field.label} · ${field.valueType}`}
            name={['schedule_input_payload', field.key]}
            rules={[
              {
                required: field.required && field.defaultValue === undefined
              }
            ]}
          >
            {field.valueType === 'number' ? (
              <InputNumber style={{ width: '100%' }} />
            ) : field.valueType === 'boolean' ? (
              <Select
                allowClear
                options={[
                  { value: true, label: 'true' },
                  { value: false, label: 'false' }
                ]}
              />
            ) : (
              <Input placeholder={field.placeholder} />
            )}
          </Form.Item>
        ];
      })}
    </>
  );
}
