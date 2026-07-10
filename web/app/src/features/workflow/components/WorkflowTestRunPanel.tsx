import { useMemo, useState } from 'react';
import {
  Alert,
  Button,
  Descriptions,
  Divider,
  Drawer,
  Form,
  Input,
  Space,
  Tag,
  Typography
} from 'antd';

import type {
  ConsoleApplicationRunDetail
} from '@1flowbase/api-client';
import type { FlowAuthoringDocument, FlowStartInputSource } from '@1flowbase/flow-schema';

import { useAuthStore } from '../../../state/auth-store';
import { i18nText } from '../../../shared/i18n/text';
import { startFlowDebugRun } from '../../agent-flow/api/runtime';
import {
  buildWorkflowTestRunInput,
  listWorkflowHttpInputFields,
  readWorkflowResult
} from '../lib/test-run';
import type { WorkflowTriggerContext } from '../lib/trigger-context';

const EXTENSION_SOURCES: FlowStartInputSource[] = [
  'path',
  'query',
  'form',
  'body'
];

const EXTENSION_SOURCE_LABEL_KEYS: Record<
  FlowStartInputSource,
  string
> = {
  path: 'auto.workflow_test_run_path_parameters',
  query: 'auto.workflow_test_run_query_parameters',
  form: 'auto.workflow_test_run_form_parameters',
  body: 'auto.workflow_test_run_body_parameters'
};

type WorkflowTestRunFormValues = {
  schedulePayload?: string;
  extensionInputs?: Record<string, Record<string, unknown>>;
};

export interface WorkflowTestRunPanelProps {
  applicationId: string;
  startRun?: typeof startFlowDebugRun;
  document: FlowAuthoringDocument;
  triggerContext: WorkflowTriggerContext;
  onOpenTrace: (runId: string) => void;
}

function errorMessage(error: unknown) {
  if (error instanceof Error && error.message.trim().length > 0) {
    return error.message;
  }

  return i18nText('workflow', 'auto.workflow_test_run_failed');
}

function ResultValue({ value }: { value: unknown }) {
  if (Array.isArray(value)) {
    return (
      <Space direction="vertical" size={4}>
        {value.map((entry, index) => (
          <ResultValue key={index} value={entry} />
        ))}
      </Space>
    );
  }

  if (typeof value === 'object' && value !== null) {
    return <WorkflowResult result={value as Record<string, unknown>} />;
  }

  if (typeof value === 'boolean') {
    return <Typography.Text>{String(value)}</Typography.Text>;
  }

  return (
    <Typography.Text>{value == null ? '—' : String(value)}</Typography.Text>
  );
}

function workflowRunStatusLabel(status: string) {
  if (status === 'waiting_human' || status === 'waiting_callback') {
    return i18nText('workflow', 'auto.workflow_test_run_status_waiting');
  }

  const key = {
    running: 'auto.workflow_test_run_status_running',
    succeeded: 'auto.workflow_test_run_status_succeeded',
    failed: 'auto.workflow_test_run_status_failed',
    cancelled: 'auto.workflow_test_run_status_cancelled'
  }[status];

  return key ? i18nText('workflow', key) : status;
}

function workflowRunStatusColor(status: string) {
  if (status === 'succeeded') return 'success';
  if (status === 'failed') return 'error';
  if (status === 'cancelled') return 'default';
  return 'processing';
}

function triggerDeliveryLabel(triggerContext: WorkflowTriggerContext) {
  if (triggerContext.triggerType === 'schedule') {
    return i18nText('workflow', 'auto.workflow_trigger_delivery_schedule');
  }
  if (triggerContext.triggerType === 'extension') {
    return i18nText('workflow', 'auto.workflow_trigger_delivery_extension');
  }
}

function WorkflowResult({ result }: { result: Record<string, unknown> }) {
  const entries = Object.entries(result);

  if (entries.length === 0) {
    return (
      <Typography.Text type="secondary">
        {i18nText('workflow', 'auto.workflow_test_run_empty_result')}
      </Typography.Text>
    );
  }

  return (
    <Descriptions column={1} size="small" bordered>
      {entries.map(([key, value]) => (
        <Descriptions.Item key={key} label={key}>
          <ResultValue value={value} />
        </Descriptions.Item>
      ))}
    </Descriptions>
  );
}

export function WorkflowTestRunPanel({
  applicationId,
  document,
  triggerContext,
  onOpenTrace,
  startRun = startFlowDebugRun
}: WorkflowTestRunPanelProps) {
  const [open, setOpen] = useState(false);
  const [running, setRunning] = useState(false);
  const [detail, setDetail] = useState<ConsoleApplicationRunDetail | null>(
    null
  );
  const [failure, setFailure] = useState<string | null>(null);
  const [form] = Form.useForm<WorkflowTestRunFormValues>();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const extensionInputFields = useMemo(
    () => listWorkflowHttpInputFields(document),
    [document]
  );

  const initialValues = useMemo<WorkflowTestRunFormValues>(
    () => ({
      schedulePayload: JSON.stringify(
        triggerContext.schedule?.input_payload ?? {},
        null,
        2
      ),
      extensionInputs: Object.fromEntries(
        EXTENSION_SOURCES.map((source) => [source, {}])
      )
    }),
    [triggerContext.schedule?.input_payload]
  );

  async function runWorkflow(values: WorkflowTestRunFormValues) {
    if (!csrfToken || !triggerContext.triggerType) {
      setFailure(i18nText('workflow', 'auto.workflow_test_run_unavailable'));
      return;
    }

    setRunning(true);
    setFailure(null);
    setDetail(null);
    try {
      const triggerType = triggerContext.triggerType;
      const runInput =
        triggerType === 'schedule'
            ? buildWorkflowTestRunInput({
                document,
                triggerType,
                schedulePayload: JSON.parse(values.schedulePayload ?? '{}')
              })
            : buildWorkflowTestRunInput({
                document,
                triggerType,
                extensionInputs: {
                  path: values.extensionInputs?.path ?? {},
                  query: values.extensionInputs?.query ?? {},
                  form: values.extensionInputs?.form ?? {},
                  body: values.extensionInputs?.body ?? {}
                }
              });
      const nextDetail = await startRun(
        applicationId,
        { ...runInput, document },
        csrfToken
      );
      setDetail(nextDetail);
    } catch (error) {
      setFailure(errorMessage(error));
    } finally {
      setRunning(false);
    }
  }

  return (
    <>
      <Button onClick={() => setOpen(true)}>
        {i18nText('workflow', 'auto.workflow_test_run')}
      </Button>
      <Drawer
        open={open}
        width={520}
        title={i18nText('workflow', 'auto.workflow_test_run')}
        onClose={() => setOpen(false)}
        destroyOnHidden
      >
        <Form
          aria-label={i18nText('workflow', 'auto.workflow_test_run_form')}
          form={form}
          layout="vertical"
          initialValues={initialValues}
          onFinish={runWorkflow}
        >
          {triggerContext.triggerType === 'schedule' ? (
            <Form.Item
              name="schedulePayload"
              label={i18nText(
                'workflow',
                'auto.workflow_schedule_input_payload'
              )}
              rules={[
                {
                  validator: (_, value: string) => {
                    try {
                      JSON.parse(value || '{}');
                      return Promise.resolve();
                    } catch {
                      return Promise.reject(
                        new Error(
                          i18nText(
                            'workflow',
                            'auto.workflow_schedule_input_payload_invalid'
                          )
                        )
                      );
                    }
                  }
                }
              ]}
            >
              <Input.TextArea rows={8} />
            </Form.Item>
          ) : null}

          {triggerContext.triggerType === 'extension'
            ? EXTENSION_SOURCES.map((source) => {
                const fields = extensionInputFields.filter(
                  (field) => field.source === source
                );

                if (fields.length === 0) {
                  return null;
                }

                return (
                  <section key={source}>
                    <Typography.Title level={5}>
                      {i18nText(
                        'workflow',
                        EXTENSION_SOURCE_LABEL_KEYS[source]
                      )}
                    </Typography.Title>
                    {fields.map((field) => (
                      <Form.Item
                        key={`${source}-${field.key}`}
                        name={['extensionInputs', source, field.key]}
                        label={field.key}
                      >
                        <Input />
                      </Form.Item>
                    ))}
                  </section>
                );
              })
            : null}

          {triggerContext.triggerType ? null : (
            <Alert
              showIcon
              type="warning"
              message={i18nText(
                'workflow',
                'auto.workflow_test_run_unavailable'
              )}
            />
          )}

          <Button
            type="primary"
            htmlType="submit"
            loading={running}
            disabled={!triggerContext.triggerType}
          >
            {i18nText('workflow', 'auto.workflow_test_run_execute')}
          </Button>
        </Form>

        {failure ? (
          <Alert
            showIcon
            type="error"
            message={i18nText('workflow', 'auto.workflow_test_run_failed')}
            description={failure}
            style={{ marginTop: 16 }}
          />
        ) : null}

        {running ? (
          <Alert
            showIcon
            type="info"
            message={i18nText(
              'workflow',
              'auto.workflow_test_run_status_running'
            )}
            style={{ marginTop: 16 }}
          />
        ) : null}

        {detail ? (
          <>
            <Divider />
            <Space direction="vertical" size="middle" style={{ width: '100%' }}>
              <Descriptions column={1} size="small">
                <Descriptions.Item
                  label={i18nText('workflow', 'auto.workflow_test_run_status')}
                >
                  <Tag color={workflowRunStatusColor(detail.flow_run.status)}>
                    {workflowRunStatusLabel(detail.flow_run.status)}
                  </Tag>
                </Descriptions.Item>
              </Descriptions>
              <Typography.Title level={5} style={{ margin: 0 }}>
                {i18nText('workflow', 'auto.workflow_result')}
              </Typography.Title>
              <WorkflowResult result={readWorkflowResult(detail)} />
              <Typography.Title level={5} style={{ margin: 0 }}>
                {i18nText('workflow', 'auto.workflow_trigger_delivery')}
              </Typography.Title>
              <Descriptions column={1} size="small" bordered>
                <Descriptions.Item
                  label={i18nText(
                    'workflow',
                    'auto.workflow_trigger_delivery_source'
                  )}
                >
                  {triggerDeliveryLabel(triggerContext)}
                </Descriptions.Item>
                <Descriptions.Item
                  label={i18nText(
                    'workflow',
                    'auto.workflow_trigger_delivery_run_id'
                  )}
                >
                  {detail.flow_run.id}
                </Descriptions.Item>
              </Descriptions>
              <Button onClick={() => onOpenTrace(detail.flow_run.id)}>
                {i18nText('workflow', 'auto.workflow_test_run_view_trace')}
              </Button>
            </Space>
          </>
        ) : null}
      </Drawer>
    </>
  );
}
