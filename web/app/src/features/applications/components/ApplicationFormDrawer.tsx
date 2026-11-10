import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Flex,
  Form,
  Input,
  InputNumber,
  Select,
  Space,
  Switch,
  Typography
} from 'antd';
import { useEffect, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';

import { WorkflowEventTriggerFields } from './WorkflowEventTriggerFields';
import { ScheduleCronEditor } from './schedule/ScheduleCronEditor';
import { DEFAULT_SCHEDULE_CRON } from './schedule/schedule-cron';
import type {
  ReplaceConsoleWorkflowEventTriggerInput,
  ConsoleApplicationType,
  ConsoleWorkflowExtensionHttpMethod,
  ConsoleWorkflowExtensionResponseMode,
  ConsoleWorkflowTriggerType
} from '@1flowbase/api-client';
import { ResizableDrawer } from '../../../shared/ui/resizable-drawer/ResizableDrawer';
import {
  fetchOrchestrationState,
  orchestrationQueryKey
} from '../../agent-flow/api/orchestration';
import { getStartInputFields } from '../../agent-flow/lib/variables/start-node-variables';
import {
  fetchWorkflowEventTrigger,
  saveWorkflowEventTrigger,
  workflowEventTriggerQueryKey,
  applicationCatalogQueryKey,
  applicationDetailQueryKey,
  applicationsQueryKey,
  createApplication,
  fetchApplicationCatalog,
  fetchApplicationDetail,
  updateApplication
} from '../api/applications';
import {
  applicationApiMappingQueryKey,
  fetchApplicationApiMapping,
  fetchWorkflowScheduleTrigger,
  saveWorkflowScheduleTrigger,
  workflowScheduleTriggerQueryKey
} from '../api/public-api';

export type ApplicationFormIntent =
  | {
      kind: 'create';
      onCreated: (applicationId: string) => void;
    }
  | {
      kind: 'edit';
      applicationId: string;
      onSaved?: () => void;
    };

interface ApplicationFormValues {
  application_type: ConsoleApplicationType;
  trigger_type: ConsoleWorkflowTriggerType;
  extension_subpath: string;
  extension_http_method: Extract<
    ConsoleWorkflowExtensionHttpMethod,
    'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE'
  >;
  extension_response_mode: ConsoleWorkflowExtensionResponseMode;
  schedule_enabled: boolean;
  schedule_cron: string;
  schedule_timezone?: string;
  schedule_input_payload?: object;
  event: ReplaceConsoleWorkflowEventTriggerInput;
  name: string;
  description: string;
  tag_ids: string[];
}

const APPLICATION_FORM_ID = 'application-form';
const SCHEDULE_TIMEZONES = ['UTC', ...Intl.supportedValuesOf('timeZone')];

export function ApplicationFormDrawer({
  open,
  csrfToken,
  intent,
  onClose
}: {
  open: boolean;
  csrfToken: string;
  intent: ApplicationFormIntent;
  onClose: () => void;
}) {
  const { t } = useTranslation('applications');
  const { t: workflowT } = useTranslation('workflow');
  const queryClient = useQueryClient();
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

  const watchedApplicationType =
    Form.useWatch('application_type', form) ?? defaultApplicationType;
  const watchedTriggerType =
    Form.useWatch('trigger_type', form) ?? defaultWorkflowTriggerType;
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

  const mutation = useMutation({
    mutationFn: async (values: ApplicationFormValues) => {
      // Ant Form collects registered fields only. With no Start inputs there are no
      // mapping controls, but the event request still owns an explicit empty mapping.
      const event = {
        ...values.event,
        input_mapping: values.event?.input_mapping ?? {}
      };
      if (intent.kind === 'create') {
        const workflow = values.application_type === 'workflow';
        const created =
          pendingCreated.current ??
          (await createApplication(
            {
              application_type: values.application_type,
              workflow_trigger_type: workflow ? values.trigger_type : null,
              workflow_trigger_config: workflow
                ? values.trigger_type === 'schedule'
                  ? {
                      cron: values.schedule_cron,
                      timezone: values.schedule_timezone ?? '',
                      input_payload: {}
                    }
                  : values.trigger_type === 'extension'
                    ? {
                        subpath: values.extension_subpath,
                        http_method: values.extension_http_method,
                        response_mode: values.extension_response_mode
                      }
                    : null
                : null,
              name: values.name,
              description: values.description,
              icon: null,
              icon_type: null,
              icon_background: null
            },
            csrfToken
          ));
        pendingCreated.current = created;
        if (workflow && values.trigger_type === 'event')
          await saveWorkflowEventTrigger(created.id, event, csrfToken);
        if (values.tag_ids.length > 0) {
          await updateApplication(
            created.id,
            {
              name: created.name,
              description: created.description,
              tag_ids: values.tag_ids
            },
            csrfToken
          );
        }
        return created.id;
      }

      await updateApplication(
        intent.applicationId,
        {
          name: values.name,
          description: values.description,
          tag_ids: values.tag_ids
        },
        csrfToken
      );
      if (application?.workflow_trigger_type === 'event')
        await saveWorkflowEventTrigger(intent.applicationId, event, csrfToken);
      if (application?.workflow_trigger_type === 'schedule') {
        await saveWorkflowScheduleTrigger(
          intent.applicationId,
          {
            enabled: values.schedule_enabled,
            cron: values.schedule_cron.trim(),
            timezone: values.schedule_timezone?.trim() ?? '',
            input_payload: values.schedule_input_payload ?? {}
          },
          csrfToken
        );
      }
      return intent.applicationId;
    },
    onSuccess: async (savedApplicationId) => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: applicationsQueryKey }),
        queryClient.invalidateQueries({
          queryKey: applicationDetailQueryKey(savedApplicationId)
        }),
        queryClient.invalidateQueries({
          queryKey: workflowEventTriggerQueryKey(savedApplicationId)
        })
      ]);
      form.resetFields();
      onClose();
      if (intent.kind === 'create') intent.onCreated(savedApplicationId);
      else intent.onSaved?.();
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

          {isEdit ? (
            <Form.Item
              label={t('auto.type')}
              htmlFor="readonly_application_type"
            >
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
            isEdit ? (
              <>
                <Form.Item
                  label={t('auto.http_method')}
                  htmlFor="readonly_http_method"
                >
                  <Input
                    id="readonly_http_method"
                    readOnly
                    value={extension?.method ?? '—'}
                  />
                </Form.Item>
                <Form.Item
                  label={t('auto.extension_subpath')}
                  htmlFor="readonly_extension_subpath"
                >
                  <Input
                    id="readonly_extension_subpath"
                    readOnly
                    value={extension ? `/api/ex/${extension.slug}` : '—'}
                  />
                </Form.Item>
                <Form.Item
                  label={t('auto.response_mode')}
                  htmlFor="readonly_response_mode"
                >
                  <Input
                    id="readonly_response_mode"
                    readOnly
                    value={
                      extension?.response_mode === 'async'
                        ? t('auto.response_mode_async')
                        : t('auto.response_mode_sync')
                    }
                  />
                </Form.Item>
                <Form.Item
                  label={t('auto.request_parameters')}
                  htmlFor="readonly_request_parameters"
                >
                  <Input.TextArea
                    id="readonly_request_parameters"
                    readOnly
                    rows={Math.min(
                      4,
                      Math.max(1, extensionContract.requestFields.length)
                    )}
                    value={
                      extensionContract.requestFields.length > 0
                        ? extensionContract.requestFields
                            .map(
                              (field) =>
                                `${field.source} · ${field.key} · ${field.valueType}`
                            )
                            .join('\n')
                        : t('auto.no_request_parameters')
                    }
                  />
                </Form.Item>
                <Form.Item
                  label={t('auto.response_fields')}
                  htmlFor="readonly_response_fields"
                >
                  <Input.TextArea
                    id="readonly_response_fields"
                    readOnly
                    rows={Math.min(
                      4,
                      Math.max(1, extensionContract.responseFields.length)
                    )}
                    value={
                      extensionContract.responseFields.length > 0
                        ? extensionContract.responseFields
                            .map((field) => `${field.key} · ${field.valueType}`)
                            .join('\n')
                        : t('auto.no_response_fields')
                    }
                  />
                </Form.Item>
              </>
            ) : (
              <>
                <Form.Item
                  htmlFor="extension_subpath"
                  label={t('auto.extension_subpath')}
                  required
                >
                  <Space.Compact block>
                    <Input
                      aria-label="/api/ex/"
                      readOnly
                      value="/api/ex/"
                      style={{ width: 88 }}
                    />
                    <Form.Item
                      name="extension_subpath"
                      noStyle
                      rules={[
                        {
                          required: true,
                          message: t('auto.extension_subpath_required')
                        }
                      ]}
                    >
                      <Input
                        id="extension_subpath"
                        placeholder="orders/create"
                      />
                    </Form.Item>
                  </Space.Compact>
                </Form.Item>
                <Form.Item
                  label={t('auto.http_method')}
                  name="extension_http_method"
                >
                  <Select
                    options={['GET', 'POST', 'PUT', 'PATCH', 'DELETE'].map(
                      (value) => ({ value, label: value })
                    )}
                  />
                </Form.Item>
                <Form.Item
                  label={t('auto.response_mode')}
                  name="extension_response_mode"
                >
                  <Select
                    options={[
                      { value: 'sync', label: t('auto.response_mode_sync') },
                      { value: 'async', label: t('auto.response_mode_async') }
                    ]}
                  />
                </Form.Item>
              </>
            )
          ) : null}

          {showWorkflow && triggerType === 'schedule' ? (
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
                rules={[
                  { required: true, message: t('auto.schedule_cron_required') }
                ]}
              >
                <ScheduleCronEditor />
              </Form.Item>
              <Form.Item
                label={t('auto.schedule_timezone')}
                name="schedule_timezone"
              >
                <Select
                  allowClear
                  showSearch
                  placeholder={t('auto.schedule_timezone_system')}
                  optionFilterProp="label"
                  options={timezoneOptions}
                />
              </Form.Item>
              {extensionContract.requestFields
                .filter((field) =>
                  ['string', 'number', 'boolean'].includes(field.valueType)
                )
                .map((field) => (
                  <Form.Item
                    key={field.key}
                    label={`${field.label} · ${field.valueType}`}
                    name={['schedule_input_payload', field.key]}
                    rules={[
                      {
                        required:
                          field.required && field.defaultValue === undefined
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
                ))}
            </>
          ) : null}

          {showWorkflow && triggerType === 'event' ? (
            <WorkflowEventTriggerFields
              form={form}
              fields={extensionContract.requestFields}
            />
          ) : null}

          {triggerLoadFailed ? (
            <Alert
              type="error"
              showIcon
              title={t('auto.trigger_load_failed')}
            />
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
        </Form>
      )}
    </ResizableDrawer>
  );
}

function formatMutationError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
