import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Descriptions,
  Divider,
  Drawer,
  Flex,
  Form,
  Input,
  Select,
  Space,
  Switch,
  Tag,
  Typography
} from 'antd';
import { useEffect, useMemo } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import {
  applicationApiMappingQueryKey,
  fetchApplicationApiMapping,
  fetchWorkflowScheduleTrigger,
  workflowScheduleTriggerQueryKey
} from '../../../applications/api/public-api';
import type { ApplicationTagCatalogEntry } from '../../../applications/api/applications';
import type { SettingsApplicationManagementItem } from '../../api/application-management';
import {
  fetchOrchestrationState,
  orchestrationQueryKey
} from '../../../agent-flow/api/orchestration';
import { getStartInputFields } from '../../../agent-flow/lib/variables/start-node-variables';

export interface ApplicationDetailsValues {
  name: string;
  description: string;
  icon: string;
  icon_type: string;
  icon_background: string;
  tag_ids: string[];
  schedule_enabled?: boolean;
  schedule_cron?: string;
  schedule_timezone?: string;
  schedule_input_payload?: object;
}

export function ApplicationDetailsDrawer({
  application,
  catalogTags,
  saving,
  onClose,
  onSubmit
}: {
  application: SettingsApplicationManagementItem | null;
  catalogTags: ApplicationTagCatalogEntry[];
  saving: boolean;
  onClose: () => void;
  onSubmit: (values: ApplicationDetailsValues) => void;
}) {
  const [form] = Form.useForm<ApplicationDetailsValues>();
  const applicationId = application?.id ?? '';
  const isSchedule = application?.workflow_trigger_type === 'schedule';
  const isExtension = application?.workflow_trigger_type === 'extension';
  const mappingQuery = useQuery({
    queryKey: applicationApiMappingQueryKey(applicationId),
    queryFn: () => fetchApplicationApiMapping(applicationId),
    enabled: Boolean(applicationId && isExtension),
    retry: false
  });
  const scheduleQuery = useQuery({
    queryKey: workflowScheduleTriggerQueryKey(applicationId),
    queryFn: () => fetchWorkflowScheduleTrigger(applicationId),
    enabled: Boolean(applicationId && isSchedule),
    retry: false
  });
  const orchestrationQuery = useQuery({
    queryKey: orchestrationQueryKey(applicationId),
    queryFn: () => fetchOrchestrationState(applicationId),
    enabled: Boolean(applicationId && isExtension),
    retry: false
  });

  useEffect(() => {
    if (!application) {
      form.resetFields();
      return;
    }

    form.setFieldsValue({
      name: application.name,
      description: application.description,
      icon: application.icon ?? '',
      icon_type: application.icon_type ?? '',
      icon_background: application.icon_background ?? '',
      tag_ids: application.tags.map((tag) => tag.id)
    });
  }, [application, form]);

  useEffect(() => {
    if (!scheduleQuery.data) {
      return;
    }
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

  return (
    <Drawer
      open={Boolean(application)}
      width={640}
      title={
        application
          ? i18nText(
              'settingsApplicationManagement',
              'auto.application_management_details_named',
              { value1: application.name }
            )
          : ''
      }
      destroyOnHidden
      onClose={onClose}
      footer={
        <Flex justify="flex-end" gap={8}>
          <Button onClick={onClose}>
            {i18nText('applications', 'auto.cancel')}
          </Button>
          <Button type="primary" loading={saving} onClick={() => form.submit()}>
            {i18nText('applications', 'auto.save_changes')}
          </Button>
        </Flex>
      }
    >
      {application ? (
        <Form<ApplicationDetailsValues>
          form={form}
          layout="vertical"
          onFinish={onSubmit}
        >
          <Descriptions size="small" column={2}>
            <Descriptions.Item
              label={i18nText(
                'settingsApplicationManagement',
                'auto.application_management_type'
              )}
            >
              <Tag>
                {application.application_type === 'workflow'
                  ? i18nText('applications', 'auto.application_type_workflow')
                  : i18nText('applications', 'auto.application_type_agent_flow')}
              </Tag>
            </Descriptions.Item>
            <Descriptions.Item
              label={i18nText(
                'settingsApplicationManagement',
                'auto.application_management_publication_status'
              )}
            >
              <Tag
                color={
                  application.publication_status === 'published'
                    ? 'success'
                    : 'default'
                }
              >
                {application.publication_status === 'published'
                  ? i18nText('settings', 'auto.published')
                  : i18nText('settings', 'auto.unpublished')}
              </Tag>
            </Descriptions.Item>
            <Descriptions.Item
              label={i18nText(
                'settingsApplicationManagement',
                'auto.application_management_creator'
              )}
            >
              {application.created_by_display_name}
            </Descriptions.Item>
          </Descriptions>

          <Divider />
          <Typography.Title level={5}>
            {i18nText(
              'settingsApplicationManagement',
              'auto.application_management_basic_information'
            )}
          </Typography.Title>
          <Form.Item
            name="name"
            label={i18nText('applications', 'auto.application_name')}
            rules={[
              {
                required: true,
                message: i18nText('applications', 'auto.application_name_required')
              }
            ]}
          >
            <Input maxLength={64} />
          </Form.Item>
          <Form.Item
            name="description"
            label={i18nText('applications', 'auto.application_description')}
          >
            <Input.TextArea rows={3} maxLength={240} />
          </Form.Item>
          <Flex gap={12} wrap>
            <Form.Item
              name="icon"
              label={i18nText(
                'settingsApplicationManagement',
                'auto.application_management_icon'
              )}
              style={{ flex: 1, minWidth: 160 }}
            >
              <Input />
            </Form.Item>
            <Form.Item
              name="icon_type"
              label={i18nText(
                'settingsApplicationManagement',
                'auto.application_management_icon_type'
              )}
              style={{ flex: 1, minWidth: 160 }}
            >
              <Input />
            </Form.Item>
            <Form.Item
              name="icon_background"
              label={i18nText(
                'settingsApplicationManagement',
                'auto.application_management_icon_background'
              )}
              style={{ flex: 1, minWidth: 160 }}
            >
              <Input />
            </Form.Item>
          </Flex>
          <Form.Item
            name="tag_ids"
            label={i18nText(
              'settingsApplicationManagement',
              'auto.application_management_tags'
            )}
          >
            <Select
              mode="multiple"
              options={catalogTags.map((tag) => ({
                value: tag.id,
                label: tag.name
              }))}
            />
          </Form.Item>

          {isSchedule ? (
            <>
              <Divider />
              <Typography.Title level={5}>
                {i18nText(
                  'settingsApplicationManagement',
                  'auto.application_management_schedule_configuration'
                )}
              </Typography.Title>
              {scheduleQuery.isError ? (
                <Alert
                  type="error"
                  showIcon
                  message={i18nText(
                    'settingsApplicationManagement',
                    'auto.application_management_trigger_load_failed'
                  )}
                />
              ) : null}
              <Form.Item
                name="schedule_enabled"
                label={i18nText(
                  'settingsApplicationManagement',
                  'auto.application_management_schedule_enabled'
                )}
                valuePropName="checked"
              >
                <Switch />
              </Form.Item>
              <Form.Item
                name="schedule_cron"
                label={i18nText('applications', 'auto.schedule_cron')}
                rules={[{ required: true }]}
              >
                <Input />
              </Form.Item>
              <Form.Item
                name="schedule_timezone"
                label={i18nText('applications', 'auto.schedule_timezone')}
                rules={[{ required: true }]}
              >
                <Input />
              </Form.Item>
            </>
          ) : null}

          {isExtension ? (
            <>
              <Divider />
              <Typography.Title level={5}>
                {i18nText(
                  'settingsApplicationManagement',
                  'auto.application_management_extension_contract'
                )}
              </Typography.Title>
              <Typography.Paragraph type="secondary">
                {i18nText(
                  'settingsApplicationManagement',
                  'auto.application_management_extension_immutable_notice'
                )}
              </Typography.Paragraph>
              {mappingQuery.isError || orchestrationQuery.isError ? (
                <Alert
                  type="error"
                  showIcon
                  message={i18nText(
                    'settingsApplicationManagement',
                    'auto.application_management_trigger_load_failed'
                  )}
                />
              ) : extension ? (
                <Descriptions bordered size="small" column={1}>
                  <Descriptions.Item
                    label={i18nText('applications', 'auto.http_method')}
                  >
                    <Tag color="blue">{extension.method}</Tag>
                  </Descriptions.Item>
                  <Descriptions.Item
                    label={i18nText('applications', 'auto.extension_subpath')}
                  >
                    <Typography.Text code>
                      {`/api/ex/${extension.slug}`}
                    </Typography.Text>
                  </Descriptions.Item>
                  <Descriptions.Item
                    label={i18nText('applications', 'auto.response_mode')}
                  >
                    {extension.response_mode === 'sync'
                      ? i18nText('applications', 'auto.response_mode_sync')
                      : i18nText('applications', 'auto.response_mode_async')}
                  </Descriptions.Item>
                  <Descriptions.Item
                    label={i18nText(
                      'settingsApplicationManagement',
                      'auto.application_management_request_parameters'
                    )}
                  >
                    {extensionContract.requestFields.length > 0 ? (
                      <Space wrap>
                        {extensionContract.requestFields.map((field) => (
                          <Tag key={`${field.source}:${field.key}`}>
                            {field.source} · {field.key} · {field.valueType}
                          </Tag>
                        ))}
                      </Space>
                    ) : (
                      i18nText(
                        'settingsApplicationManagement',
                        'auto.application_management_no_request_parameters'
                      )
                    )}
                  </Descriptions.Item>
                  <Descriptions.Item
                    label={i18nText(
                      'settingsApplicationManagement',
                      'auto.application_management_response_fields'
                    )}
                  >
                    {extensionContract.responseFields.length > 0 ? (
                      <Space wrap>
                        {extensionContract.responseFields.map((field) => (
                          <Tag key={field.key}>
                            {field.key} · {field.valueType}
                          </Tag>
                        ))}
                      </Space>
                    ) : (
                      i18nText(
                        'settingsApplicationManagement',
                        'auto.application_management_no_response_fields'
                      )
                    )}
                  </Descriptions.Item>
                </Descriptions>
              ) : null}
            </>
          ) : null}
        </Form>
      ) : null}
    </Drawer>
  );
}
