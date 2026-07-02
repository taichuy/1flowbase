import { useEffect, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Button, Drawer, Form, Space, Typography } from 'antd';
import { useTranslation } from 'react-i18next';

import { useAuthStore } from '../../../../state/auth-store';
import { applicationDetailQueryKey } from '../../api/applications';
import {
  applicationApiMappingQueryKey,
  applicationApiPublicationQueryKey,
  fetchApplicationApiMapping,
  fetchApplicationApiPublication,
  fetchWorkflowScheduleTrigger,
  publishApplicationApiVersion,
  saveApplicationApiMapping,
  saveWorkflowScheduleTrigger,
  workflowScheduleTriggerQueryKey,
  type ApplicationApiMapping,
  type WorkflowScheduleTrigger
} from '../../api/public-api';
import {
  DEFAULT_WORKFLOW_TRIGGER_VALUES,
  createDefaultWorkflowApiMapping,
  createWorkflowApiMappingWithoutExtension,
  createWorkflowScheduleTriggerInput,
  createWorkflowExtensionTargetOptions,
  createWorkflowTriggerValuesFromMapping,
  findInvalidWorkflowExtensionParameterTargets,
  type WorkflowTriggerFormValues
} from '../../lib/workflow-trigger-config';
import { useAgentFlowEditorStore } from '../../../agent-flow/store/editor/provider';
import { selectWorkingDocument } from '../../../agent-flow/store/editor/selectors';
import { WorkflowTriggerFormFields } from './WorkflowTriggerFormFields';
import './workflow-trigger-config-bar.css';

export function WorkflowTriggerConfigBar({
  applicationId
}: {
  applicationId: string;
}) {
  const { t } = useTranslation('applications');
  const csrfToken = useAuthStore((state) => state.csrfToken) ?? '';
  const queryClient = useQueryClient();
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [form] = Form.useForm<WorkflowTriggerFormValues>();
  const workingDocument = useAgentFlowEditorStore(selectWorkingDocument);
  const extensionTargetOptions =
    createWorkflowExtensionTargetOptions(workingDocument);
  const mappingQuery = useQuery({
    queryKey: applicationApiMappingQueryKey(applicationId),
    queryFn: () => fetchApplicationApiMapping(applicationId)
  });
  const scheduleQuery = useQuery({
    queryKey: workflowScheduleTriggerQueryKey(applicationId),
    queryFn: () => fetchWorkflowScheduleTrigger(applicationId),
    retry: false
  });
  const publicationQuery = useQuery({
    queryKey: applicationApiPublicationQueryKey(applicationId),
    queryFn: () => fetchApplicationApiPublication(applicationId),
    retry: false
  });
  const triggerType =
    Form.useWatch('trigger_type', form) ??
    form.getFieldValue('trigger_type') ??
    DEFAULT_WORKFLOW_TRIGGER_VALUES.trigger_type;
  const saveMutation = useMutation({
    mutationFn: async (values: WorkflowTriggerFormValues) => {
      if (values.trigger_type === 'schedule') {
        await saveWorkflowScheduleTrigger(
          applicationId,
          createWorkflowScheduleTriggerInput(values),
          csrfToken
        );
        await saveApplicationApiMapping(
          applicationId,
          createWorkflowApiMappingWithoutExtension(),
          csrfToken
        );
        return;
      }

      await saveApplicationApiMapping(
        applicationId,
        createDefaultWorkflowApiMapping(values),
        csrfToken
      );
      if (scheduleQuery.data) {
        await saveWorkflowScheduleTrigger(
          applicationId,
          {
            enabled: false,
            cron: scheduleQuery.data.cron,
            timezone: scheduleQuery.data.timezone,
            input_payload: scheduleQuery.data.input_payload ?? {}
          },
          csrfToken
        );
      }
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: applicationApiMappingQueryKey(applicationId)
      });
      void queryClient.invalidateQueries({
        queryKey: applicationApiPublicationQueryKey(applicationId)
      });
      void queryClient.invalidateQueries({
        queryKey: workflowScheduleTriggerQueryKey(applicationId)
      });
      setDrawerOpen(false);
    }
  });
  const publishMutation = useMutation({
    mutationFn: async () => {
      const mapping =
        mappingQuery.data ?? (await fetchApplicationApiMapping(applicationId));
      const invalidTargets = findInvalidWorkflowExtensionParameterTargets(
        mapping,
        extensionTargetOptions
      );

      if (invalidTargets.length > 0) {
        throw new Error(t('auto.workflow_parameter_target_invalid'));
      }

      return publishApplicationApiVersion(applicationId, mapping, csrfToken);
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: applicationApiPublicationQueryKey(applicationId)
      });
      void queryClient.invalidateQueries({
        queryKey: applicationDetailQueryKey(applicationId)
      });
    }
  });

  useEffect(() => {
    if (!drawerOpen) {
      return;
    }

    form.setFieldsValue(
      createWorkflowTriggerValues(mappingQuery.data, scheduleQuery.data ?? null)
    );
  }, [drawerOpen, form, mappingQuery.data, scheduleQuery.data]);

  const triggerSummary = createWorkflowTriggerSummary(
    mappingQuery.data,
    scheduleQuery.data ?? null,
    t
  );
  const publicationSummary = publicationQuery.data?.active
    ? t('auto.workflow_publication_active')
    : t('auto.workflow_publication_not_published');

  return (
    <section className="workflow-trigger-config-bar">
      <div className="workflow-trigger-config-bar__summary">
        <Typography.Text strong>{t('auto.workflow_trigger')}</Typography.Text>
        <Typography.Text type="secondary">{triggerSummary}</Typography.Text>
        <Typography.Text type="secondary">{publicationSummary}</Typography.Text>
      </div>
      <Space wrap>
        <Button onClick={() => setDrawerOpen(true)}>
          {t('auto.workflow_trigger_configuration')}
        </Button>
        <Button
          type="primary"
          loading={publishMutation.isPending || mappingQuery.isLoading}
          onClick={() => publishMutation.mutate()}
        >
          {t('auto.publish_current_version')}
        </Button>
      </Space>
      {publishMutation.isError ? (
        <Alert
          type="error"
          showIcon
          message={t('auto.workflow_publish_failed')}
          description={formatMutationError(publishMutation.error)}
        />
      ) : null}
      <Drawer
        title={t('auto.workflow_trigger_configuration')}
        open={drawerOpen}
        width={640}
        destroyOnHidden
        onClose={() => setDrawerOpen(false)}
      >
        {saveMutation.isError ? (
          <Alert
            type="error"
            showIcon
            message={t('auto.workflow_trigger_configuration_failed')}
            description={formatMutationError(saveMutation.error)}
          />
        ) : null}
        <Form<WorkflowTriggerFormValues>
          form={form}
          layout="vertical"
          initialValues={DEFAULT_WORKFLOW_TRIGGER_VALUES}
          onFinish={(values) => saveMutation.mutate(values)}
        >
          <WorkflowTriggerFormFields
            isExtensionTrigger={triggerType === 'extension'}
            isScheduleTrigger={triggerType === 'schedule'}
            extensionTargetOptions={extensionTargetOptions}
            t={t}
          />
          <Button
            type="primary"
            htmlType="submit"
            loading={saveMutation.isPending}
          >
            {t('auto.save_changes')}
          </Button>
        </Form>
      </Drawer>
    </section>
  );
}

function createWorkflowTriggerValues(
  mapping: ApplicationApiMapping | null | undefined,
  schedule: WorkflowScheduleTrigger | null
): WorkflowTriggerFormValues {
  const values = createWorkflowTriggerValuesFromMapping(mapping);
  if (!schedule) {
    return values;
  }

  return {
    ...values,
    trigger_type: mapping?.extension ? values.trigger_type : 'schedule',
    schedule_enabled: schedule.enabled,
    schedule_cron: schedule.cron,
    schedule_timezone: schedule.timezone,
    schedule_input_payload: JSON.stringify(
      schedule.input_payload ?? {},
      null,
      2
    )
  };
}

function createWorkflowTriggerSummary(
  mapping: ApplicationApiMapping | null | undefined,
  schedule: WorkflowScheduleTrigger | null,
  t: (key: string, options?: Record<string, string>) => string
) {
  const extension = mapping?.extension;
  if (extension) {
    return `${extension.method} /api/ex/${extension.slug}`;
  }

  if (schedule) {
    const status = schedule.enabled
      ? t('auto.workflow_schedule_enabled')
      : t('auto.workflow_schedule_disabled');
    return `${status} · ${schedule.cron} · ${schedule.timezone}`;
  }

  return t('auto.workflow_trigger_not_configured');
}

function formatMutationError(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}
