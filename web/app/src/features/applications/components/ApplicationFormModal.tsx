import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Form,
  Input,
  Radio,
  Select,
  Space,
  Switch,
  Typography
} from 'antd';
import { useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import type {
  ConsoleWorkflowExtensionHttpMethod,
  ConsoleWorkflowExtensionResponseMode,
  ConsoleWorkflowTriggerType
} from '@1flowbase/api-client';
import { SchemaModalPanel } from '../../../shared/schema-ui/overlay-shell/SchemaModalPanel';
import {
  fetchOrchestrationState,
  orchestrationQueryKey
} from '../../agent-flow/api/orchestration';
import { getStartInputFields } from '../../agent-flow/lib/variables/start-node-variables';
import {
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

const WORKFLOW_TRIGGER_TYPE_OPTIONS: ConsoleWorkflowTriggerType[] = [
  'extension',
  'schedule'
];
const DEFAULT_WORKFLOW_TRIGGER_TYPE: ConsoleWorkflowTriggerType = 'extension';

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
  application_type: 'agent_flow' | 'workflow';
  trigger_type: ConsoleWorkflowTriggerType;
  extension_subpath: string;
  extension_http_method: Extract<
    ConsoleWorkflowExtensionHttpMethod,
    'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE'
  >;
  extension_response_mode: ConsoleWorkflowExtensionResponseMode;
  schedule_enabled: boolean;
  schedule_cron: string;
  schedule_timezone: string;
  schedule_input_payload?: object;
  name: string;
  description: string;
  tag_ids: string[];
}

const applicationFormShell = {
  schemaVersion: '1.0.0',
  shellType: 'modal_panel',
  destroyOnHidden: true
} as const;

function workflowTriggerTypeLabelKey(type: ConsoleWorkflowTriggerType) {
  return type === 'extension'
    ? 'auto.workflow_trigger_type_extension'
    : 'auto.workflow_trigger_type_schedule';
}

export function ApplicationFormModal({
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
  const application = detailQuery.data ?? null;
  const isExtension =
    isEdit && application?.workflow_trigger_type === 'extension';
  const isSchedule =
    isEdit && application?.workflow_trigger_type === 'schedule';
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
  const orchestrationQuery = useQuery({
    queryKey: orchestrationQueryKey(applicationId),
    queryFn: () => fetchOrchestrationState(applicationId),
    enabled: open && isExtension,
    retry: false
  });

  useEffect(() => {
    if (!open) {
      form.resetFields();
      return;
    }
    if (!isEdit) {
      form.setFieldsValue({
        application_type: 'agent_flow',
        trigger_type: DEFAULT_WORKFLOW_TRIGGER_TYPE,
        extension_subpath: '',
        extension_http_method: 'POST',
        extension_response_mode: 'sync',
        schedule_enabled: false,
        schedule_cron: '',
        schedule_timezone: 'Asia/Shanghai',
        schedule_input_payload: {},
        name: '',
        description: '',
        tag_ids: []
      });
      return;
    }
    if (!application) return;
    form.setFieldsValue({
      application_type: application.application_type,
      trigger_type:
        application.workflow_trigger_type ?? DEFAULT_WORKFLOW_TRIGGER_TYPE,
      name: application.name,
      description: application.description,
      tag_ids: application.tags.map((tag) => tag.id)
    });
  }, [application, form, isEdit, open]);

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
      schedule_timezone: scheduleQuery.data.timezone,
      schedule_input_payload:
        typeof scheduleQuery.data.input_payload === 'object' &&
        scheduleQuery.data.input_payload !== null
          ? scheduleQuery.data.input_payload
          : {}
    });
  }, [form, scheduleQuery.data]);

  const watchedApplicationType =
    Form.useWatch('application_type', form) ?? 'agent_flow';
  const watchedTriggerType =
    Form.useWatch('trigger_type', form) ?? DEFAULT_WORKFLOW_TRIGGER_TYPE;
  const showWorkflow = isEdit
    ? application?.application_type === 'workflow'
    : watchedApplicationType === 'workflow';
  const triggerType = isEdit
    ? (application?.workflow_trigger_type ?? DEFAULT_WORKFLOW_TRIGGER_TYPE)
    : watchedTriggerType;
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
      if (intent.kind === 'create') {
        const workflow = values.application_type === 'workflow';
        const created = await createApplication(
          {
            application_type: values.application_type,
            workflow_trigger_type: workflow ? values.trigger_type : null,
            workflow_trigger_config: workflow
              ? values.trigger_type === 'schedule'
                ? {
                    cron: values.schedule_cron,
                    timezone: values.schedule_timezone,
                    input_payload: {}
                  }
                : {
                    subpath: values.extension_subpath,
                    http_method: values.extension_http_method,
                    response_mode: values.extension_response_mode
                  }
              : null,
            name: values.name,
            description: values.description,
            icon: null,
            icon_type: null,
            icon_background: null
          },
          csrfToken
        );
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
      if (application?.workflow_trigger_type === 'schedule') {
        await saveWorkflowScheduleTrigger(
          intent.applicationId,
          {
            enabled: values.schedule_enabled,
            cron: values.schedule_cron.trim(),
            timezone: values.schedule_timezone.trim(),
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
        })
      ]);
      form.resetFields();
      onClose();
      if (intent.kind === 'create') intent.onCreated(savedApplicationId);
      else intent.onSaved?.();
    }
  });

  const loading = isEdit && detailQuery.isPending;
  const triggerLoadFailed =
    mappingQuery.isError || scheduleQuery.isError || orchestrationQuery.isError;

  return (
    <SchemaModalPanel
      open={open}
      schema={{
        ...applicationFormShell,
        title: isEdit
          ? t('auto.edit_application_information')
          : t('auto.new_application')
      }}
      onClose={onClose}
    >
      {loading ? (
        <Typography.Text type="secondary">{t('auto.loading')}</Typography.Text>
      ) : (
        <Form<ApplicationFormValues>
          form={form}
          layout="vertical"
          onFinish={(values) => mutation.mutate(values)}
        >
          {mutation.isError ? (
            <Alert
              type="error"
              showIcon
              message={
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
                value={
                  application?.application_type === 'workflow'
                    ? t('auto.application_type_workflow')
                    : t('auto.application_type_agent_flow')
                }
              />
            </Form.Item>
          ) : (
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
                  value={workflowT(workflowTriggerTypeLabelKey(triggerType))}
                />
              </Form.Item>
            ) : (
              <Form.Item
                label={workflowT('auto.workflow_trigger_type')}
                name="trigger_type"
              >
                <Select<ConsoleWorkflowTriggerType>
                  options={WORKFLOW_TRIGGER_TYPE_OPTIONS.map((type) => ({
                    value: type,
                    label: workflowT(workflowTriggerTypeLabelKey(type))
                  }))}
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
                  label={t('auto.extension_subpath')}
                  name="extension_subpath"
                  rules={[
                    {
                      required: true,
                      message: t('auto.extension_subpath_required')
                    }
                  ]}
                >
                  <Input addonBefore="/api/ex/" placeholder="orders/create" />
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
              ) : (
                <Alert
                  type="info"
                  showIcon
                  message={t('auto.schedule_disabled_hint')}
                />
              )}
              <Form.Item
                label={t('auto.schedule_cron')}
                name="schedule_cron"
                rules={[
                  { required: true, message: t('auto.schedule_cron_required') }
                ]}
              >
                <Input placeholder="0 9 * * 1-5" />
              </Form.Item>
              <Form.Item
                label={t('auto.schedule_timezone')}
                name="schedule_timezone"
                rules={[
                  {
                    required: true,
                    message: t('auto.schedule_timezone_required')
                  }
                ]}
              >
                <Input />
              </Form.Item>
            </>
          ) : null}

          {triggerLoadFailed ? (
            <Alert
              type="error"
              showIcon
              message={t('auto.trigger_load_failed')}
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
          <Button type="primary" htmlType="submit" loading={mutation.isPending}>
            {isEdit ? t('auto.save_changes') : t('auto.create_application')}
          </Button>
        </Form>
      )}
    </SchemaModalPanel>
  );
}

function formatMutationError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
