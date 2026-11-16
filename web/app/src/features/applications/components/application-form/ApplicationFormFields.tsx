import { Alert, Form, Input, Select } from 'antd';
import { useTranslation } from 'react-i18next';
import type {
  ConsoleApplicationType,
  ConsoleWorkflowTriggerType
} from '@1flowbase/api-client';
import { WorkflowEventTriggerFields } from '../WorkflowEventTriggerFields';
import { WorkflowExtensionFields } from './WorkflowExtensionFields';
import { WorkflowScheduleFields } from './WorkflowScheduleFields';
import type { useApplicationFormController } from './use-application-form-controller';

type ApplicationFormFieldsProps = Pick<
  ReturnType<typeof useApplicationFormController>,
  | 'form'
  | 'isEdit'
  | 'catalogQuery'
  | 'applicationTypeOption'
  | 'workflowTriggerTypeOption'
  | 'showWorkflow'
  | 'triggerType'
  | 'extension'
  | 'extensionContract'
  | 'timezoneOptions'
  | 'triggerLoadFailed'
>;

export function ApplicationFormFields({
  form,
  isEdit,
  catalogQuery,
  applicationTypeOption,
  workflowTriggerTypeOption,
  showWorkflow,
  triggerType,
  extension,
  extensionContract,
  timezoneOptions,
  triggerLoadFailed
}: ApplicationFormFieldsProps) {
  const { t } = useTranslation('applications');
  const { t: workflowT } = useTranslation('workflow');
  return (
    <>
      {isEdit ? (
        <Form.Item label={t('auto.type')} htmlFor="readonly_application_type">
          <Input
            id="readonly_application_type"
            readOnly
            value={applicationTypeOption?.label ?? ''}
          />
        </Form.Item>
      ) : (
        <Form.Item label={t('auto.type')} name="application_type">
          <Select<ConsoleApplicationType>
            options={(catalogQuery.data?.types ?? []).map((option) => ({
              value: option.value,
              label: option.label
            }))}
          />
        </Form.Item>
      )}

      {showWorkflow ? (
        isEdit ? (
          <Form.Item
            label={workflowT('auto.workflow_trigger_type')}
            htmlFor="readonly_trigger_type"
          >
            <Input
              id="readonly_trigger_type"
              readOnly
              value={workflowTriggerTypeOption?.label ?? ''}
            />
          </Form.Item>
        ) : (
          <Form.Item
            label={workflowT('auto.workflow_trigger_type')}
            name="trigger_type"
          >
            <Select<ConsoleWorkflowTriggerType>
              options={(catalogQuery.data?.workflow_triggers ?? []).map(
                (option) => ({
                  value: option.value,
                  label: option.label
                })
              )}
            />
          </Form.Item>
        )
      ) : null}

      {showWorkflow && triggerType === 'extension' ? (
        <WorkflowExtensionFields
          isEdit={isEdit}
          extension={extension}
          extensionContract={extensionContract}
        />
      ) : null}

      {showWorkflow && triggerType === 'schedule' ? (
        <WorkflowScheduleFields
          isEdit={isEdit}
          timezoneOptions={timezoneOptions}
          extensionContract={extensionContract}
        />
      ) : null}

      {showWorkflow && triggerType === 'event' ? (
        <WorkflowEventTriggerFields
          form={form}
          fields={extensionContract.requestFields}
        />
      ) : null}

      {triggerLoadFailed ? (
        <Alert type="error" showIcon title={t('auto.trigger_load_failed')} />
      ) : null}

      <Form.Item
        label={t('auto.name')}
        name="name"
        rules={[{ required: true, message: t('auto.name_required') }]}
      >
        <Input maxLength={64} />
      </Form.Item>
      <Form.Item label={t('auto.description')} name="description">
        <Input.TextArea rows={3} maxLength={240} />
      </Form.Item>
      <Form.Item label={t('auto.tags')} name="tag_ids">
        <Select
          mode="multiple"
          options={(catalogQuery.data?.tags ?? []).map((tag) => ({
            value: tag.id,
            label: tag.name
          }))}
        />
      </Form.Item>
    </>
  );
}
