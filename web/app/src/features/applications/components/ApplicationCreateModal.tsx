import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Alert, Button, Form, Input, Radio, Space } from 'antd';
import { useTranslation } from 'react-i18next';

import { SchemaModalPanel } from '../../../shared/schema-ui/overlay-shell/SchemaModalPanel';
import { applicationsQueryKey, createApplication } from '../api/applications';
import { WorkflowTriggerTypeField } from './workflow/WorkflowTriggerFormFields';
import {
  DEFAULT_WORKFLOW_TRIGGER_VALUES,
  WORKFLOW_TRIGGER_TYPES,
  type WorkflowTriggerType
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
  trigger_type: WorkflowTriggerType;
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
          workflow_trigger_type: isWorkflow ? values.trigger_type : null,
          name: values.name,
          description: values.description,
          icon: 'RobotOutlined',
          icon_type: 'iconfont',
          icon_background: '#E6F7F2'
        },
        csrfToken
      );

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
  const isWorkflow = applicationType === 'workflow';

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
          trigger_type: DEFAULT_WORKFLOW_TRIGGER_VALUES.trigger_type
        }}
        onFinish={(values) => mutation.mutate(values)}
      >
        {mutation.isError ? (
          <Alert
            type="error"
            showIcon
            message={t('auto.create_application_failed')}
            description={formatMutationError(mutation.error)}
          />
        ) : null}
        <Form.Item label={t('auto.type')} name="application_type">
          <Radio.Group>
            <Space direction="vertical" size="small">
              <Radio value="agent_flow">
                {t('auto.application_type_agent_flow')}
              </Radio>
              <Radio value="workflow">
                {t('auto.application_type_workflow')}
              </Radio>
            </Space>
          </Radio.Group>
        </Form.Item>

        {isWorkflow ? (
          <WorkflowTriggerTypeField
            triggerTypes={WORKFLOW_TRIGGER_TYPES}
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
          {t('auto.create_application')}
        </Button>
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
