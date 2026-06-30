import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Alert, Button, Form, Input, Radio, Space } from 'antd';
import { useTranslation } from 'react-i18next';

import { SchemaModalPanel } from '../../../shared/schema-ui/overlay-shell/SchemaModalPanel';
import { applicationsQueryKey, createApplication } from '../api/applications';
import {
  saveApplicationApiMapping,
  saveWorkflowScheduleTrigger
} from '../api/public-api';
import { WorkflowTriggerFormFields } from './workflow/WorkflowTriggerFormFields';
import {
  DEFAULT_WORKFLOW_TRIGGER_VALUES,
  createDefaultWorkflowApiMapping,
  createWorkflowScheduleTriggerInput,
  type WorkflowTriggerFormValues
} from '../lib/workflow-trigger-config';

interface ApplicationCreateModalProps {
  open: boolean;
  csrfToken: string;
  onClose: () => void;
  onCreated: (applicationId: string) => void;
}

interface ApplicationCreateFormValues {
  application_type: 'agent_flow' | 'workflow';
  name: string;
  description: string;
  trigger_type: WorkflowTriggerFormValues['trigger_type'];
  extension_slug: WorkflowTriggerFormValues['extension_slug'];
  extension_method: WorkflowTriggerFormValues['extension_method'];
  extension_response_mode: WorkflowTriggerFormValues['extension_response_mode'];
  extension_parameters: WorkflowTriggerFormValues['extension_parameters'];
  schedule_enabled: WorkflowTriggerFormValues['schedule_enabled'];
  schedule_cron: WorkflowTriggerFormValues['schedule_cron'];
  schedule_timezone: WorkflowTriggerFormValues['schedule_timezone'];
  schedule_input_payload: WorkflowTriggerFormValues['schedule_input_payload'];
}

const applicationCreateShell = {
  schemaVersion: '1.0.0',
  shellType: 'modal_panel',
  destroyOnHidden: true
} as const;

export function ApplicationCreateModal({
  open,
  csrfToken,
  onClose,
  onCreated
}: ApplicationCreateModalProps) {
  const { t } = useTranslation('applications');
  const queryClient = useQueryClient();
  const [form] = Form.useForm<ApplicationCreateFormValues>();
  const mutation = useMutation({
    mutationFn: async (values: ApplicationCreateFormValues) => {
      const created = await createApplication(
        {
          application_type: values.application_type,
          name: values.name,
          description: values.description,
          icon: 'RobotOutlined',
          icon_type: 'iconfont',
          icon_background: '#E6F7F2'
        },
        csrfToken
      );

      if (values.application_type === 'workflow') {
        if (values.trigger_type === 'schedule') {
          await saveWorkflowScheduleTrigger(
            created.id,
            createWorkflowScheduleTriggerInput(values),
            csrfToken
          );
        } else {
          await saveApplicationApiMapping(
            created.id,
            createDefaultWorkflowApiMapping(values),
            csrfToken
          );
        }
      }

      return created;
    },
    onSuccess: async (created) => {
      await queryClient.invalidateQueries({ queryKey: applicationsQueryKey });
      form.resetFields();
      onClose();
      onCreated(created.id);
    }
  });
  const applicationType =
    Form.useWatch('application_type', form) ??
    form.getFieldValue('application_type') ??
    'agent_flow';
  const triggerType =
    Form.useWatch('trigger_type', form) ??
    form.getFieldValue('trigger_type') ??
    'extension';
  const isWorkflow = applicationType === 'workflow';
  const isExtensionTrigger = isWorkflow && triggerType === 'extension';
  const isScheduleTrigger = isWorkflow && triggerType === 'schedule';

  return (
    <SchemaModalPanel
      open={open}
      schema={{ ...applicationCreateShell, title: t('auto.new_application') }}
      onClose={onClose}
    >
      <Form<ApplicationCreateFormValues>
        form={form}
        layout="vertical"
        initialValues={{
          application_type: 'agent_flow',
          name: '',
          description: '',
          ...DEFAULT_WORKFLOW_TRIGGER_VALUES
        }}
        onFinish={(values) => mutation.mutate(values)}
      >
        {mutation.isError ? (
          <Alert
            type="error"
            showIcon
            message={t('auto.workflow_trigger_configuration_failed')}
            description={formatMutationError(mutation.error)}
          />
        ) : null}
        <Form.Item label={t('auto.type')} name="application_type">
          <Radio.Group>
            <Space direction="vertical" size="small">
              <Radio value="agent_flow">{t('auto.application_type_agent_flow')}</Radio>
              <Radio value="workflow">
                {t('auto.application_type_workflow')}
              </Radio>
            </Space>
          </Radio.Group>
        </Form.Item>

        {isWorkflow ? (
          <WorkflowTriggerFormFields
            isExtensionTrigger={isExtensionTrigger}
            isScheduleTrigger={isScheduleTrigger}
            t={t}
          />
        ) : null}

        <Form.Item
          label={t('auto.name')}
          name="name"
          rules={[{ required: true, message: t('auto.name_required') }]}
        >
          <Input />
        </Form.Item>

        <Form.Item label={t('auto.description')} name="description">
          <Input.TextArea rows={3} />
        </Form.Item>

        <Button type="primary" htmlType="submit" loading={mutation.isPending}>
          {t('auto.create_application')}</Button>
      </Form>
    </SchemaModalPanel>
  );
}

function formatMutationError(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}
