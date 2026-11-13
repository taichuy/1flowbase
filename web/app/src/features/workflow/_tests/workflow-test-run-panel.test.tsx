import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { afterEach, describe, expect, test, vi } from 'vitest';
import { createDefaultWorkflowDocument } from '@1flowbase/flow-schema';

import {
  WorkflowTestRunPanel,
  type WorkflowTestRunPanelProps
} from '../components/WorkflowTestRunPanel';
import type { WorkflowTriggerContext } from '../lib/trigger-context';

function documentWithInputs() {
  const document = createDefaultWorkflowDocument({ flowId: 'flow-1' });
  document.graph.nodes.find(
    (node) => node.type === 'workflow_start'
  )!.config.input_fields = [
    {
      key: 'customer_id',
      label: 'Customer ID',
      inputType: 'text',
      valueType: 'string',
      required: true
    },
    {
      key: 'force',
      label: 'Force',
      inputType: 'checkbox',
      valueType: 'boolean',
      required: true,
      defaultValue: false
    },
    {
      key: 'limit',
      label: 'Limit',
      inputType: 'number',
      valueType: 'number',
      required: false,
      defaultValue: 10
    }
  ];
  return document;
}
function context(
  triggerType: WorkflowTriggerContext['triggerType'] = 'schedule'
): WorkflowTriggerContext {
  return {
    applicationId: 'app-1',
    triggerType,
    mapping: null,
    schedule: null,
    workflowStartFieldContract: undefined
  };
}
async function renderPanel(props: Partial<WorkflowTestRunPanelProps> = {}) {
  const onRun = vi.fn().mockResolvedValue(undefined);
  await act(async () => {
    render(
      <WorkflowTestRunPanel
        document={documentWithInputs()}
        triggerContext={context()}
        running={false}
        canRun
        onRun={onRun}
        {...props}
      />
    );
  });
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: '测试运行' }));
  });
  return onRun;
}
afterEach(cleanup);

describe('Workflow test-run parameters', () => {
  test.each(['schedule', 'extension', 'event'] as const)(
    '%s immediately runs empty inputs without a drawer',
    async (triggerType) => {
      const onRun = await renderPanel({
        triggerContext: context(triggerType),
        document: createDefaultWorkflowDocument({ flowId: 'flow-1' })
      });
      expect(onRun).toHaveBeenCalledExactlyOnceWith({});
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    }
  );

  test('required input prevents starting and keeps the form open', async () => {
    const onRun = await renderPanel();
    await act(async () => {
      fireEvent.submit(screen.getByRole('form'));
    });
    await waitFor(() =>
      expect(screen.getByLabelText('Customer ID')).toHaveAttribute(
        'aria-invalid',
        'true'
      )
    );
    expect(onRun).not.toHaveBeenCalled();
    expect(screen.getByRole('dialog')).toBeInTheDocument();
  });

  test('starts with typed values and closes the parameter drawer', async () => {
    const onRun = await renderPanel();
    expect(screen.getByLabelText('Limit')).toHaveValue('10');
    expect(screen.getByLabelText('Force')).not.toBeChecked();
    await act(async () => {
      fireEvent.change(screen.getByLabelText('Customer ID'), {
        target: { value: 'C-42' }
      });
    });
    await act(async () => {
      fireEvent.submit(screen.getByRole('form'));
    });
    await waitFor(() =>
      expect(onRun).toHaveBeenCalledWith({
        customer_id: 'C-42',
        force: false,
        limit: 10
      })
    );
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
  });

  test('prefills schedule values instead of exposing raw JSON', async () => {
    await renderPanel({
      triggerContext: {
        ...context(),
        schedule: {
          id: 'schedule-1',
          workspace_id: 'workspace-1',
          application_id: 'app-1',
          enabled: false,
          cron: '0 3 * * *',
          timezone: 'Asia/Shanghai',
          input_payload: { customer_id: 'C-42', force: true, limit: 0 },
          created_by: 'user-1',
          updated_by: 'user-1',
          created_at: '',
          updated_at: ''
        }
      }
    });
    expect(screen.getByLabelText('Customer ID')).toHaveValue('C-42');
    expect(screen.getByLabelText('Force')).toBeChecked();
    expect(screen.getByLabelText('Limit')).toHaveValue('0');
    expect(screen.queryByLabelText('输入 payload')).not.toBeInTheDocument();
  });

  test('an unavailable trigger never starts an empty run', async () => {
    const onRun = await renderPanel({
      triggerContext: context(null),
      document: createDefaultWorkflowDocument({ flowId: 'flow-1' })
    });
    expect(onRun).not.toHaveBeenCalled();
    expect(
      screen.getByText('当前工作流尚未配置可用触发器或登录凭证。')
    ).toBeInTheDocument();
  });

  test('the run entry is disabled while a run is being observed', async () => {
    const onRun = await renderPanel({ running: true });
    expect(screen.getByRole('button', { name: /测试运行/ })).toBeDisabled();
    expect(onRun).not.toHaveBeenCalled();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });
});
