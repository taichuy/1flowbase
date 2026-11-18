import { Alert, Form, Input, Select } from 'antd';
import type { UseQueryResult } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import type {
  ConsoleApplicationType,
  ConsoleWorkflowTriggerType
} from '@1flowbase/api-client';
import { WorkflowEventTriggerFields } from '../WorkflowEventTriggerFields';
import { WorkflowExtensionFields } from './WorkflowExtensionFields';
import { WorkflowScheduleFields } from './WorkflowScheduleFields';
import type {
  fetchApplicationCatalog,
  fetchApplicationDetail
} from '../../api/applications';
import type { fetchApplicationApiMapping } from '../../api/public-api';
import type { getStartInputFields } from '../../../agent-flow/lib/variables/start-node-variables';
import type { fetchOrchestrationState } from '../../../agent-flow/api/orchestration';

type OrchestrationNode = Awaited<
  ReturnType<typeof fetchOrchestrationState>
>['draft']['document']['graph']['nodes'][number];

export interface ApplicationFormFieldsProps {
  isEdit: boolean;
  catalogQuery: UseQueryResult<
    Awaited<ReturnType<typeof fetchApplicationCatalog>>
  >;
  application: Awaited<ReturnType<typeof fetchApplicationDetail>> | null;
  extension:
    | Awaited<ReturnType<typeof fetchApplicationApiMapping>>['extension']
    | null;
  extensionContract: {
    requestFields: ReturnType<typeof getStartInputFields>;
    responseFields: NonNullable<OrchestrationNode['outputs']>;
  };
  timezoneOptions: Array<{ value: string; label: string }>;
  triggerLoadFailed: boolean;
}

export function ApplicationFormFields({
  isEdit,
  catalogQuery,
  application,
  extension,
  extensionContract,
  timezoneOptions,
  triggerLoadFailed
}: ApplicationFormFieldsProps) {
  const { t } = useTranslation('applications');
  const { t: workflowT } = useTranslation('workflow');
  const defaultApplicationType = catalogQuery.data?.types[0]?.value;
  const defaultWorkflowTriggerType =
    catalogQuery.data?.workflow_triggers[0]?.value;
  const watchedApplicationType =
    Form.useWatch('application_type') ?? defaultApplicationType;
  const watchedTriggerType =
    Form.useWatch('trigger_type') ?? defaultWorkflowTriggerType;
  const showWorkflow = isEdit
    ? application?.application_type === 'workflow'
    : watchedApplicationType === 'workflow';
  const triggerType = isEdit
    ? (application?.workflow_trigger_type ?? defaultWorkflowTriggerType)
    : watchedTriggerType;
  const applicationTypeOption = catalogQuery.data?.types.find(
    (option) =>
      option.value ===
      (isEdit ? application?.application_type : watchedApplicationType)
  );
  const workflowTriggerTypeOption = catalogQuery.data?.workflow_triggers.find(
    (option) => option.value === triggerType
  );
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
        <WorkflowEventTriggerFields fields={extensionContract.requestFields} />
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
