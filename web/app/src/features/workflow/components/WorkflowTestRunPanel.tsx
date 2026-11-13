import { useMemo, useState } from 'react';
import {
  Alert,
  Button,
  Checkbox,
  Drawer,
  Form,
  Input,
  InputNumber,
  Select,
  Typography
} from 'antd';

import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';

import { i18nText } from '../../../shared/i18n/text';
import { getStartInputFields } from '../../flow-editor/authoring/start-input-fields';
import type { WorkflowTriggerContext } from '../lib/trigger-context';

type WorkflowTestRunFormValues = {
  inputs: Record<string, unknown>;
};

export interface WorkflowTestRunPanelProps {
  document: FlowAuthoringDocument;
  triggerContext: WorkflowTriggerContext;
  running: boolean;
  canRun: boolean;
  onRun: (inputs: Record<string, unknown>) => Promise<void>;
}

export function WorkflowTestRunPanel({
  document,
  triggerContext,
  running,
  canRun,
  onRun
}: WorkflowTestRunPanelProps) {
  const [open, setOpen] = useState(false);
  const inputFields = useMemo(
    () =>
      getStartInputFields(
        document.graph.nodes.find((node) => node.type === 'workflow_start')
      ),
    [document]
  );
  const visibleInputFields = inputFields.filter((field) => !field.hidden);
  const hasUnsupportedInputs = visibleInputFields.some(
    (field) => field.inputType === 'file' || field.inputType === 'file_list'
  );
  const canExecute = Boolean(canRun && triggerContext.triggerType);
  const initialValues = useMemo<WorkflowTestRunFormValues>(
    () => ({
      inputs: Object.fromEntries(
        inputFields.flatMap((field) => {
          const value =
            (triggerContext.triggerType === 'schedule'
              ? triggerContext.schedule?.input_payload[field.key]
              : undefined) ??
            field.defaultValue ??
            (field.inputType === 'checkbox' ? false : undefined);
          return value === undefined ? [] : [[field.key, value]];
        })
      )
    }),
    [
      inputFields,
      triggerContext.triggerType,
      triggerContext.schedule?.input_payload
    ]
  );

  function runWorkflow(values: WorkflowTestRunFormValues) {
    if (!canExecute || running) return;
    setOpen(false);
    void onRun({ ...initialValues.inputs, ...values.inputs });
  }

  return (
    <>
      <Button
        disabled={!canRun || running}
        loading={running}
        onClick={() => {
          if (visibleInputFields.length === 0 && canExecute) {
            runWorkflow(initialValues);
          } else {
            setOpen(true);
          }
        }}
      >
        {i18nText('workflow', 'auto.workflow_test_run')}
      </Button>
      <Drawer
        open={open}
        size={520}
        styles={{ wrapper: { maxWidth: '100vw' } }}
        title={i18nText('workflow', 'auto.workflow_test_run')}
        onClose={() => setOpen(false)}
        destroyOnHidden
      >
        {visibleInputFields.length > 0 ? (
          <Form<WorkflowTestRunFormValues>
            aria-label={i18nText('workflow', 'auto.workflow_test_run_form')}
            layout="vertical"
            initialValues={initialValues}
            disabled={running}
            onFinish={runWorkflow}
          >
            <Typography.Title level={5} style={{ marginTop: 0 }}>
              {i18nText('workflow', 'auto.input_parameters')}
            </Typography.Title>
            {triggerContext.triggerType === 'event' ? (
              <Alert
                type="info"
                title={i18nText('workflow', 'event.debug_hint')}
                style={{ marginBottom: 16 }}
              />
            ) : null}
            {visibleInputFields.map((field) => (
              <Form.Item
                key={field.key}
                name={['inputs', field.key]}
                label={field.label}
                valuePropName={
                  field.inputType === 'checkbox' ? 'checked' : 'value'
                }
                rules={[
                  { required: field.required },
                  ...(field.inputType === 'url'
                    ? [{ type: 'url' as const }]
                    : [])
                ]}
              >
                {field.inputType === 'number' ? (
                  <InputNumber
                    placeholder={field.placeholder}
                    style={{ width: '100%' }}
                  />
                ) : field.inputType === 'checkbox' ? (
                  <Checkbox />
                ) : field.inputType === 'select' ? (
                  <Select
                    allowClear={!field.required}
                    placeholder={field.placeholder}
                    options={(field.options ?? []).map((value) => ({
                      value,
                      label: value
                    }))}
                  />
                ) : field.inputType === 'paragraph' ? (
                  <Input.TextArea
                    autoSize={{ minRows: 3, maxRows: 6 }}
                    placeholder={field.placeholder}
                    maxLength={field.maxLength}
                  />
                ) : (
                  <Input
                    type={field.inputType === 'url' ? 'url' : 'text'}
                    placeholder={field.placeholder}
                    maxLength={field.maxLength}
                    disabled={
                      field.inputType === 'file' ||
                      field.inputType === 'file_list'
                    }
                  />
                )}
              </Form.Item>
            ))}
            {hasUnsupportedInputs ? (
              <Alert
                showIcon
                type="warning"
                title={i18nText(
                  'workflow',
                  'auto.workflow_test_run_file_unavailable'
                )}
                style={{ marginBottom: 16 }}
              />
            ) : null}
            <Button
              type="primary"
              htmlType="submit"
              loading={running}
              disabled={!canExecute || hasUnsupportedInputs}
            >
              {i18nText('workflow', 'auto.workflow_test_run_execute')}
            </Button>
          </Form>
        ) : null}

        {!canExecute ? (
          <Alert
            showIcon
            type="warning"
            title={i18nText('workflow', 'auto.workflow_test_run_unavailable')}
          />
        ) : null}
      </Drawer>
    </>
  );
}
