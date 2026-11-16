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
import { useTranslation } from 'react-i18next';
import type {
  ConsoleApplicationType,
  ConsoleWorkflowTriggerType
} from '@1flowbase/api-client';

import { WorkflowEventTriggerFields } from './WorkflowEventTriggerFields';
import { ScheduleCronEditor } from './schedule/ScheduleCronEditor';
import { ResizableDrawer } from '../../../shared/ui/resizable-drawer/ResizableDrawer';
import {
  useApplicationFormController,
  type ApplicationFormIntent,
  type ApplicationFormValues
} from './application-form/use-application-form-controller';

export type { ApplicationFormIntent } from './application-form/use-application-form-controller';

const APPLICATION_FORM_ID = 'application-form';

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
  const {
    form,
    isEdit,
    isEvent,
    eventQuery,
    catalogQuery,
    applicationTypeOption,
    workflowTriggerTypeOption,
    showWorkflow,
    triggerType,
    extension,
    extensionContract,
    timezoneOptions,
    mutation,
    loading,
    triggerLoadFailed
  } = useApplicationFormController({ open, csrfToken, intent, onClose });

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
