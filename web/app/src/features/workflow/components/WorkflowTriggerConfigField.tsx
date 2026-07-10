import type {
  FlowAuthoringDocument,
  FlowNodeDocument
} from '@1flowbase/flow-schema';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Alert, Button, Form } from 'antd';
import { useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import type { SchemaFieldRendererProps } from '../../../shared/schema-ui/registry/create-renderer-registry';
import { useAuthStore } from '../../../state/auth-store';
import { applicationDetailQueryKey } from '../../applications/api/applications';
import {
  applicationApiMappingQueryKey,
  applicationApiPublicationQueryKey,
  saveApplicationApiMapping,
  saveWorkflowScheduleTrigger,
  workflowScheduleTriggerQueryKey
} from '../../applications/api/public-api';
import {
  DEFAULT_WORKFLOW_TRIGGER_VALUES,
  createDefaultWorkflowApiMapping,
  createWorkflowExtensionTargetOptions,
  createWorkflowScheduleTriggerInput,
  createWorkflowTriggerValuesFromContext,
  type WorkflowTriggerFormValues
} from '../lib/trigger-config';
import {
  WorkflowExtensionTriggerFields,
  WorkflowScheduleTriggerFields
} from './WorkflowTriggerFormFields';
import { asWorkflowTriggerContext } from '../lib/trigger-context';

function formatMutationError(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

export function WorkflowTriggerConfigField({
  adapter
}: SchemaFieldRendererProps) {
  const { t } = useTranslation('workflow');
  const csrfToken = useAuthStore((state) => state.csrfToken) ?? '';
  const queryClient = useQueryClient();
  const [form] = Form.useForm<WorkflowTriggerFormValues>();
  const node = adapter.getDerived('node') as FlowNodeDocument | null;
  const document = adapter.getDerived(
    'document'
  ) as FlowAuthoringDocument | null;
  const context = asWorkflowTriggerContext(
    adapter.getDerived('workflowTriggerContext')
  );
  const values = useMemo(() => {
    if (!context?.triggerType) {
      return DEFAULT_WORKFLOW_TRIGGER_VALUES;
    }

    return createWorkflowTriggerValuesFromContext({
      triggerType: context.triggerType,
      mapping: context.mapping,
      schedule: context.schedule
    });
  }, [context?.mapping, context?.schedule, context?.triggerType]);
  const extensionTargetOptions = useMemo(
    () => (document ? createWorkflowExtensionTargetOptions(document) : []),
    [document]
  );
  const saveMutation = useMutation({
    mutationFn: async (nextValues: WorkflowTriggerFormValues) => {
      if (!context?.triggerType) {
        return;
      }

      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      if (context.triggerType === 'schedule') {
        await saveWorkflowScheduleTrigger(
          context.applicationId,
          createWorkflowScheduleTriggerInput(nextValues),
          csrfToken
        );
        return;
      }

      if (context.triggerType === 'extension') {
        await saveApplicationApiMapping(
          context.applicationId,
          createDefaultWorkflowApiMapping(nextValues),
          csrfToken
        );
      }
    },
    onSuccess: () => {
      if (!context) {
        return;
      }

      void queryClient.invalidateQueries({
        queryKey: applicationApiMappingQueryKey(context.applicationId)
      });
      void queryClient.invalidateQueries({
        queryKey: workflowScheduleTriggerQueryKey(context.applicationId)
      });
      void queryClient.invalidateQueries({
        queryKey: applicationApiPublicationQueryKey(context.applicationId)
      });
      void queryClient.invalidateQueries({
        queryKey: applicationDetailQueryKey(context.applicationId)
      });
    }
  });

  useEffect(() => {
    form.setFieldsValue(values);
  }, [form, values]);

  if (!context?.triggerType || node?.type !== 'workflow_start') {
    return null;
  }

  return (
    <Form<WorkflowTriggerFormValues>
      form={form}
      layout="vertical"
      initialValues={values}
      onFinish={(nextValues) => saveMutation.mutate(nextValues)}
    >
      {saveMutation.isError ? (
        <Alert
          type="error"
          showIcon
          message={t('auto.workflow_trigger_configuration_failed')}
          description={formatMutationError(saveMutation.error)}
        />
      ) : null}
      {context.triggerType === 'extension' ? (
        <WorkflowExtensionTriggerFields
          extensionTargetOptions={extensionTargetOptions}
          t={t}
        />
      ) : (
        <WorkflowScheduleTriggerFields t={t} />
      )}
      <Button type="primary" htmlType="submit" loading={saveMutation.isPending}>
        {t('auto.save_changes')}
      </Button>
    </Form>
  );
}
