import { useQuery } from '@tanstack/react-query';
import { Alert, Button, Flex, Form, Typography } from 'antd';
import { useEffect, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';

import { ResizableDrawer } from '../../../shared/ui/resizable-drawer/ResizableDrawer';
import { ApplicationFormFields } from './application-form/ApplicationFormFields';
import {
  useApplicationFormMutation,
  type ApplicationFormIntent,
  type ApplicationFormValues
} from './application-form/use-application-form-mutation';
import { DEFAULT_SCHEDULE_CRON } from './schedule/schedule-cron';
import {
  fetchOrchestrationState,
  orchestrationQueryKey
} from '../../agent-flow/api/orchestration';
import { getStartInputFields } from '../../agent-flow/lib/variables/start-node-variables';
import {
  fetchWorkflowEventTrigger,
  workflowEventTriggerQueryKey,
  applicationCatalogQueryKey,
  applicationDetailQueryKey,
  createApplication,
  fetchApplicationCatalog,
  fetchApplicationDetail
} from '../api/applications';
import {
  applicationApiMappingQueryKey,
  fetchApplicationApiMapping,
  fetchWorkflowScheduleTrigger,
  workflowScheduleTriggerQueryKey
} from '../api/public-api';

export type { ApplicationFormIntent } from './application-form/use-application-form-mutation';

const APPLICATION_FORM_ID = 'application-form';
const SCHEDULE_TIMEZONES = ['UTC', ...Intl.supportedValuesOf('timeZone')];

interface ApplicationFormDrawerProps {
  open: boolean;
  csrfToken: string;
  intent: ApplicationFormIntent;
  onClose: () => void;
}

export function ApplicationFormDrawer({
  open,
  csrfToken,
  intent,
  onClose
}: ApplicationFormDrawerProps) {
  const { t } = useTranslation('applications');
  const [form] = Form.useForm<ApplicationFormValues>();
  const pendingCreated = useRef<Awaited<
    ReturnType<typeof createApplication>
  > | null>(null);
  const isEdit = intent.kind === 'edit';
  const applicationId = isEdit ? intent.applicationId : '';
  const detailQuery = useQuery({
    queryKey: applicationDetailQueryKey(applicationId),
    queryFn: () => fetchApplicationDetail(applicationId),
    enabled: open && isEdit,
    retry: false
  });
  const catalogQuery = useQuery({
    queryKey: applicationCatalogQueryKey,
    queryFn: fetchApplicationCatalog,
    enabled: open,
    retry: false
  });
  const defaultApplicationType = catalogQuery.data?.types[0]?.value;
  const defaultWorkflowTriggerType =
    catalogQuery.data?.workflow_triggers[0]?.value;
  const application = detailQuery.data ?? null;
  const isExtension =
    isEdit && application?.workflow_trigger_type === 'extension';
  const isSchedule =
    isEdit && application?.workflow_trigger_type === 'schedule';
  const isEvent = isEdit && application?.workflow_trigger_type === 'event';
  const eventQuery = useQuery({
    queryKey: workflowEventTriggerQueryKey(applicationId),
    queryFn: () => fetchWorkflowEventTrigger(applicationId),
    enabled: open && isEvent,
    retry: false
  });
  useEffect(() => {
    if (eventQuery.isSuccess)
      form.setFieldValue(
        'event',
        eventQuery.data ?? {
          contract_id: '',
          trigger_phase: 'published',
          input_mapping: {}
        }
      );
  }, [form, eventQuery.data, eventQuery.isSuccess]);
  const mappingQuery = useQuery({
    queryKey: applicationApiMappingQueryKey(applicationId),
    queryFn: () => fetchApplicationApiMapping(applicationId),
    enabled: open && isExtension,
    retry: false
  });
  const scheduleQuery = useQuery({
    queryKey: workflowScheduleTriggerQueryKey(applicationId),
    queryFn: () => fetchWorkflowScheduleTrigger(applicationId),
    enabled: open && isSchedule,
    retry: false
  });
  const timezoneOptions = useMemo(() => {
    const timezones = new Set(SCHEDULE_TIMEZONES);
    if (scheduleQuery.data?.timezone) {
      timezones.add(scheduleQuery.data.timezone);
    }
    return Array.from(timezones, (value) => ({ value, label: value }));
  }, [scheduleQuery.data?.timezone]);
  const orchestrationQuery = useQuery({
    queryKey: orchestrationQueryKey(applicationId),
    queryFn: () => fetchOrchestrationState(applicationId),
    enabled: open && (isExtension || isSchedule || isEvent),
    retry: false
  });

  useEffect(() => {
    if (!open) {
      form.resetFields();
      pendingCreated.current = null;
      return;
    }
    if (!isEdit) {
      if (!defaultApplicationType || !defaultWorkflowTriggerType) {
        return;
      }

      form.setFieldsValue({
        application_type: defaultApplicationType,
        trigger_type: defaultWorkflowTriggerType,
        extension_subpath: '',
        extension_http_method: 'POST',
        extension_response_mode: 'sync',
        event: {
          contract_id: '',
          trigger_phase: 'published',
          input_mapping: {}
        },
        schedule_enabled: false,
        schedule_cron: DEFAULT_SCHEDULE_CRON,
        schedule_timezone: undefined,
        schedule_input_payload: {},
        name: '',
        description: '',
        tag_ids: []
      });
      return;
    }
    if (!application || !defaultWorkflowTriggerType) return;
    form.setFieldsValue({
      application_type: application.application_type,
      trigger_type:
        application.workflow_trigger_type ?? defaultWorkflowTriggerType,
      name: application.name,
      description: application.description,
      tag_ids: application.tags.map((tag) => tag.id)
    });
  }, [
    application,
    defaultApplicationType,
    defaultWorkflowTriggerType,
    form,
    isEdit,
    open
  ]);

  useEffect(() => {
    const extension = mappingQuery.data?.extension;
    if (!extension) return;
    form.setFieldsValue({
      extension_subpath: extension.slug,
      extension_http_method:
        extension.method as ApplicationFormValues['extension_http_method'],
      extension_response_mode: extension.response_mode
    });
  }, [form, mappingQuery.data]);

  useEffect(() => {
    if (!scheduleQuery.data) return;
    form.setFieldsValue({
      schedule_enabled: scheduleQuery.data.enabled,
      schedule_cron: scheduleQuery.data.cron,
      schedule_timezone: scheduleQuery.data.timezone || undefined,
      schedule_input_payload:
        typeof scheduleQuery.data.input_payload === 'object' &&
        scheduleQuery.data.input_payload !== null
          ? scheduleQuery.data.input_payload
          : {}
    });
  }, [form, scheduleQuery.data]);

  const extension = mappingQuery.data?.extension ?? null;
  const extensionContract = useMemo(() => {
    const nodes = orchestrationQuery.data?.draft.document.graph.nodes ?? [];
    const startNode = nodes.find((node) => node.type === 'workflow_start');
    const endNode = nodes.find((node) => node.type === 'workflow_end');
    return {
      requestFields: getStartInputFields(startNode),
      responseFields: endNode?.outputs ?? []
    };
  }, [orchestrationQuery.data]);

  const mutation = useApplicationFormMutation({
    csrfToken,
    intent,
    isEvent,
    isSchedule,
    pendingCreated,
    onSaved: () => {
      form.resetFields();
      onClose();
    }
  });

  const loading = catalogQuery.isPending || (isEdit && detailQuery.isPending);
  const triggerLoadFailed =
    catalogQuery.isError ||
    mappingQuery.isError ||
    scheduleQuery.isError ||
    eventQuery.isError ||
    orchestrationQuery.isError;

  return (
    <ResizableDrawer
      open={open}
      title={
        isEdit
          ? t('auto.edit_application_information')
          : t('auto.new_application')
      }
      destroyOnClose
      resizeLabel={t('auto.resize_application_drawer')}
      footer={
        loading ? null : (
          <Flex justify="flex-end">
            <Button
              type="primary"
              htmlType="submit"
              form={APPLICATION_FORM_ID}
              disabled={triggerLoadFailed || (isEvent && eventQuery.isPending)}
              loading={mutation.isPending}
            >
              {isEdit ? t('auto.save_changes') : t('auto.create_application')}
            </Button>
          </Flex>
        )
      }
      onClose={onClose}
    >
      {loading ? (
        <Typography.Text type="secondary">{t('auto.loading')}</Typography.Text>
      ) : (
        <Form<ApplicationFormValues>
          id={APPLICATION_FORM_ID}
          form={form}
          layout="vertical"
          onFinish={(values) => mutation.mutate(values)}
        >
          {mutation.isError ? (
            <Alert
              type="error"
              showIcon
              title={
                isEdit
                  ? t('auto.save_failed')
                  : t('auto.create_application_failed')
              }
              description={formatMutationError(mutation.error)}
            />
          ) : null}

          <ApplicationFormFields
            isEdit={isEdit}
            catalogQuery={catalogQuery}
            application={application}
            extension={extension}
            extensionContract={extensionContract}
            timezoneOptions={timezoneOptions}
            triggerLoadFailed={triggerLoadFailed}
          />
        </Form>
      )}
    </ResizableDrawer>
  );
}

function formatMutationError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
