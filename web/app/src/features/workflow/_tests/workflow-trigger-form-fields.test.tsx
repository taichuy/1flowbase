import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { Button, Form } from 'antd';
import { describe, expect, test, vi } from 'vitest';

import { createDefaultWorkflowDocument } from '@1flowbase/flow-schema';

import { WorkflowExtensionTriggerFields } from '../components/WorkflowTriggerFormFields';
import {
  DEFAULT_WORKFLOW_TRIGGER_VALUES,
  createWorkflowExtensionTargetOptions,
  type WorkflowTriggerFormValues
} from '../lib/trigger-config';

const labels: Record<string, string> = {
  'auto.workflow_trigger_type': '触发器类型',
  'auto.workflow_trigger_type_extension': '扩展接口触发',
  'auto.workflow_trigger_type_schedule': '定时触发',
  'auto.workflow_extension_slug': '接口 slug',
  'auto.workflow_extension_slug_help': 'Slug help',
  'auto.workflow_extension_method': 'HTTP method',
  'auto.workflow_extension_response_mode': '响应模式',
  'auto.workflow_response_mode_sync': '同步响应',
  'auto.workflow_response_mode_async': '异步响应',
  'auto.workflow_parameter_source_path': 'path',
  'auto.workflow_parameter_source_query': 'query',
  'auto.workflow_parameter_source_form': 'form',
  'auto.workflow_parameter_source_body': 'body',
  'auto.parameter_source': '参数来源',
  'auto.parameter_name': '参数名',
  'auto.parameter_target': '目标 selector',
  'auto.remove_parameter': '移除参数',
  'auto.add_parameter': '添加参数',
  'auto.workflow_start_input_targets_empty': '先配置开始节点输入参数',
  'auto.workflow_parameter_target_invalid': '目标 selector 已失效',
  'auto.workflow_parameter_target_required': '请选择目标 selector',
  'auto.workflow_schedule_status': '定时状态',
  'auto.enable': '启用',
  'auto.disable': '停用',
  'auto.workflow_schedule_cron': '定时表达式',
  'auto.workflow_schedule_timezone': '时区',
  'auto.workflow_schedule_input_payload': '输入 payload',
  'auto.workflow_schedule_input_payload_invalid': '输入 payload 不是有效 JSON',
  'auto.field_required': '字段必填'
};

function t(key: string) {
  return labels[key] ?? key;
}

function renderExtensionFields({
  extensionParameters = [
    {
      source: 'query',
      name: 'customer_id',
      target: ''
    }
  ],
  targetOptions = [
    {
      value: 'node-workflow-start.customer_id',
      label: '客户 ID · node-workflow-start.customer_id'
    }
  ],
  onFinish = vi.fn()
}: {
  extensionParameters?: WorkflowTriggerFormValues['extension_parameters'];
  targetOptions?: Array<{ value: string; label: string }>;
  onFinish?: (values: WorkflowTriggerFormValues) => void;
} = {}) {
  render(
    <Form<WorkflowTriggerFormValues>
      layout="vertical"
      initialValues={{
        ...DEFAULT_WORKFLOW_TRIGGER_VALUES,
        trigger_type: 'extension',
        extension_slug: 'ticket_webhook',
        extension_parameters: extensionParameters
      }}
      onFinish={onFinish}
    >
      <WorkflowExtensionTriggerFields
        extensionTargetOptions={targetOptions}
        t={t}
      />
      <Button htmlType="submit">保存</Button>
    </Form>
  );

  return { onFinish };
}

describe('WorkflowTriggerFormFields', () => {
  test('builds extension target options from workflow start input fields', () => {
    const document = createDefaultWorkflowDocument({ flowId: 'flow-1' });
    const startNode = document.graph.nodes.find(
      (node) => node.id === 'node-workflow-start'
    );

    if (!startNode) {
      throw new Error('expected workflow start node');
    }

    startNode.config.input_fields = [
      {
        key: 'customer_id',
        label: '客户 ID',
        inputType: 'text',
        valueType: 'string',
        required: true,
        hidden: false
      },
      {
        key: 'priority',
        label: '优先级',
        inputType: 'select',
        valueType: 'string',
        required: false,
        hidden: false
      }
    ];

    expect(createWorkflowExtensionTargetOptions(document)).toEqual([
      {
        value: 'node-workflow-start.customer_id',
        label: '客户 ID · node-workflow-start.customer_id'
      },
      {
        value: 'node-workflow-start.priority',
        label: '优先级 · node-workflow-start.priority'
      }
    ]);
  });

  test('uses a selector control instead of a free text target input', () => {
    renderExtensionFields();

    expect(
      screen.queryByRole('textbox', { name: '目标 selector' })
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('combobox', { name: '目标 selector' })
    ).toBeInTheDocument();
  });

  test('shows an empty state when workflow start has no input fields', () => {
    renderExtensionFields({ targetOptions: [] });

    expect(screen.getByText('先配置开始节点输入参数')).toBeInTheDocument();
    expect(
      screen.queryByRole('textbox', { name: '目标 selector' })
    ).not.toBeInTheDocument();
  });

  test('blocks saving stale extension targets', async () => {
    const { onFinish } = renderExtensionFields({
      extensionParameters: [
        {
          source: 'query',
          name: 'customer_id',
          target: 'node-workflow-start.legacy_customer_id'
        }
      ]
    });

    fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));

    expect(await screen.findByText('目标 selector 已失效')).toBeInTheDocument();
    await waitFor(() => {
      expect(onFinish).not.toHaveBeenCalled();
    });
  });
});
