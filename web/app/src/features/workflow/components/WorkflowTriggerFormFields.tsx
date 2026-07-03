import { Alert, Button, Form, Input, Radio, Select, Space } from 'antd';

import {
  WORKFLOW_EXTENSION_HTTP_METHODS,
  WORKFLOW_EXTENSION_PARAMETER_SOURCES,
  parseWorkflowScheduleInputPayload,
  type WorkflowExtensionParameterSource,
  type WorkflowExtensionTargetOption
} from '../lib/trigger-config';

export function WorkflowExtensionTriggerFields({
  extensionTargetOptions = [],
  t
}: {
  extensionTargetOptions?: WorkflowExtensionTargetOption[];
  t: (key: string, options?: Record<string, string>) => string;
}) {
  const form = Form.useFormInstance();
  const allowedTargets = new Set(
    extensionTargetOptions.map((option) => option.value)
  );
  const hasExtensionTargets = extensionTargetOptions.length > 0;

  return (
    <>
      <Form.Item
        label={t('auto.workflow_extension_slug')}
        name="extension_slug"
        rules={[
          {
            required: true,
            message: t('auto.field_required', {
              value1: t('auto.workflow_extension_slug')
            })
          }
        ]}
        extra={t('auto.workflow_extension_slug_help')}
      >
        <Input addonBefore="/api/ex/" />
      </Form.Item>

      <Form.Item
        label={t('auto.workflow_extension_method')}
        name="extension_method"
      >
        <Radio.Group>
          <Space wrap>
            {WORKFLOW_EXTENSION_HTTP_METHODS.map((method) => (
              <Radio key={method} value={method}>
                {method}
              </Radio>
            ))}
          </Space>
        </Radio.Group>
      </Form.Item>

      <Form.Item
        label={t('auto.workflow_extension_response_mode')}
        name="extension_response_mode"
      >
        <Radio.Group>
          <Space wrap>
            <Radio value="sync">{t('auto.workflow_response_mode_sync')}</Radio>
            <Radio value="async">
              {t('auto.workflow_response_mode_async')}
            </Radio>
          </Space>
        </Radio.Group>
      </Form.Item>

      {!hasExtensionTargets ? (
        <Alert
          type="info"
          showIcon
          message={t('auto.workflow_start_input_targets_empty')}
        />
      ) : null}

      <Form.List name="extension_parameters">
        {(fields, { add, remove }) => (
          <Space direction="vertical" size={8} style={{ width: '100%' }}>
            {fields.map((field) => (
              <Space
                key={field.key}
                align="start"
                size={8}
                wrap
                style={{ width: '100%' }}
              >
                <Form.Item
                  label={t('auto.parameter_source')}
                  name={[field.name, 'source']}
                >
                  <Select<WorkflowExtensionParameterSource>
                    style={{ width: 120 }}
                    options={WORKFLOW_EXTENSION_PARAMETER_SOURCES.map(
                      (source) => ({
                        value: source,
                        label: t(workflowParameterSourceLabelKey(source))
                      })
                    )}
                  />
                </Form.Item>
                <Form.Item
                  label={t('auto.parameter_name')}
                  name={[field.name, 'name']}
                >
                  <Input style={{ width: 180 }} />
                </Form.Item>
                <Form.Item
                  label={t('auto.parameter_target')}
                  name={[field.name, 'target']}
                  rules={[
                    {
                      validator: (_, value: string | undefined) => {
                        const target = value?.trim() ?? '';
                        const parameterName =
                          (
                            form.getFieldValue([
                              'extension_parameters',
                              field.name,
                              'name'
                            ]) as string | undefined
                          )?.trim() ?? '';

                        if (target.length === 0) {
                          return parameterName.length > 0
                            ? Promise.reject(
                                new Error(
                                  t('auto.workflow_parameter_target_required')
                                )
                              )
                            : Promise.resolve();
                        }

                        return allowedTargets.has(target)
                          ? Promise.resolve()
                          : Promise.reject(
                              new Error(
                                t('auto.workflow_parameter_target_invalid')
                              )
                            );
                      }
                    }
                  ]}
                >
                  <Select
                    disabled={!hasExtensionTargets}
                    style={{ width: 260 }}
                    options={extensionTargetOptions}
                  />
                </Form.Item>
                {fields.length > 1 ? (
                  <Button onClick={() => remove(field.name)}>
                    {t('auto.remove_parameter')}
                  </Button>
                ) : null}
              </Space>
            ))}
            <Button
              type="dashed"
              disabled={!hasExtensionTargets}
              onClick={() => add({ source: 'query', name: '', target: '' })}
            >
              {t('auto.add_parameter')}
            </Button>
          </Space>
        )}
      </Form.List>
    </>
  );
}

export function WorkflowScheduleTriggerFields({
  t
}: {
  t: (key: string, options?: Record<string, string>) => string;
}) {
  return (
    <>
      <Form.Item
        label={t('auto.workflow_schedule_status')}
        name="schedule_enabled"
      >
        <Radio.Group>
          <Space wrap>
            <Radio value>{t('auto.enable')}</Radio>
            <Radio value={false}>{t('auto.disable')}</Radio>
          </Space>
        </Radio.Group>
      </Form.Item>
      <Form.Item
        label={t('auto.workflow_schedule_cron')}
        name="schedule_cron"
        rules={[
          {
            required: true,
            message: t('auto.field_required', {
              value1: t('auto.workflow_schedule_cron')
            })
          }
        ]}
      >
        <Input />
      </Form.Item>
      <Form.Item
        label={t('auto.workflow_schedule_timezone')}
        name="schedule_timezone"
        rules={[
          {
            required: true,
            message: t('auto.field_required', {
              value1: t('auto.workflow_schedule_timezone')
            })
          }
        ]}
      >
        <Input />
      </Form.Item>
      <Form.Item
        label={t('auto.workflow_schedule_input_payload')}
        name="schedule_input_payload"
        rules={[
          {
            validator: (_, value: string) => {
              try {
                parseWorkflowScheduleInputPayload(value);
                return Promise.resolve();
              } catch {
                return Promise.reject(
                  new Error(t('auto.workflow_schedule_input_payload_invalid'))
                );
              }
            }
          }
        ]}
      >
        <Input.TextArea rows={3} />
      </Form.Item>
    </>
  );
}

function workflowParameterSourceLabelKey(
  source: WorkflowExtensionParameterSource
) {
  switch (source) {
    case 'path':
      return 'auto.workflow_parameter_source_path';
    case 'query':
      return 'auto.workflow_parameter_source_query';
    case 'form':
      return 'auto.workflow_parameter_source_form';
    case 'body':
      return 'auto.workflow_parameter_source_body';
  }
}
