import type React from 'react';
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import type { ConsoleApplicationRunDetail } from '@1flowbase/api-client';
import { createDefaultWorkflowDocument } from '@1flowbase/flow-schema';

import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { WorkflowTestRunPanel } from '../components/WorkflowTestRunPanel';
import type { WorkflowTriggerContext } from '../lib/trigger-context';

function createWorkflowDocument() {
  const document = createDefaultWorkflowDocument({ flowId: 'flow-1' });
  const startNode = document.graph.nodes.find(
    (node) => node.type === 'workflow_start'
  );

  if (!startNode) {
    throw new Error('expected workflow_start node');
  }

  startNode.config.input_fields = [
    {
      key: 'customer_id',
      label: 'Customer ID',
      inputType: 'text',
      valueType: 'string',
      required: true,
      source: 'path'
    },
    {
      key: 'force',
      label: 'Force',
      inputType: 'checkbox',
      valueType: 'boolean',
      required: false,
      defaultValue: false,
      source: 'query'
    }
  ];

  return document;
}

function createTriggerContext(
  overrides: Partial<WorkflowTriggerContext> = {}
): WorkflowTriggerContext {
  return {
    applicationId: 'app-1',
    triggerType: 'schedule',
    mapping: null,
    schedule: null,
    workflowStartFieldContract: undefined,
    ...overrides
  };
}

function createSucceededDetail(): ConsoleApplicationRunDetail {
  return {
    flow_run: {
      id: 'run-1',
      application_id: 'app-1',
      flow_id: 'flow-1',
      draft_id: 'draft-1',
      compiled_plan_id: 'plan-1',
      run_mode: 'debug_flow_run',
      status: 'succeeded',
      target_node_id: null,
      input_payload: {},
      output_payload: {
        ticket_id: 'ticket-C-42'
      },
      error_payload: null,
      created_by: 'user-1',
      started_at: '2026-07-10T00:00:00Z',
      finished_at: '2026-07-10T00:00:01Z',
      created_at: '2026-07-10T00:00:00Z'
    },
    node_runs: [],
    checkpoints: [],
    callback_tasks: [],
    events: []
  };
}

async function renderPanel(
  triggerContext = createTriggerContext(),
  onOpenTrace = vi.fn(),
  startRun: React.ComponentProps<
    typeof WorkflowTestRunPanel
  >['startRun'] = vi.fn()
) {
  const document = createWorkflowDocument();

  await renderSettled(
    <WorkflowTestRunPanel
      applicationId="app-1"
      document={document}
      triggerContext={triggerContext}
      onOpenTrace={onOpenTrace}
      startRun={startRun}
    />
  );

  await click(screen.getByRole('button', { name: '测试运行' }));
  return { document, onOpenTrace };
}

beforeEach(() => {
  resetAuthStore();
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'user-1',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'workspace-1'
    },
    me: null
  });
});

afterEach(async () => {
  cleanup();
  await act(async () => {
    resetAuthStore();
  });
  vi.restoreAllMocks();
});

describe('WorkflowTestRunPanel', () => {
  test('AC-103 uses schedule input_payload as editable JSON default', async () => {
    await renderPanel(
      createTriggerContext({
        triggerType: 'schedule',
        schedule: {
          id: 'schedule-1',
          workspace_id: 'workspace-1',
          application_id: 'app-1',
          enabled: true,
          cron: '0 9 * * *',
          timezone: 'UTC',
          input_payload: { customer_id: 'C-42', force: true },
          created_by: 'user-1',
          updated_by: 'user-1',
          created_at: '2026-07-10T00:00:00Z',
          updated_at: '2026-07-10T00:00:00Z'
        }
      })
    );

    expect(await screen.findByLabelText('输入 payload')).toHaveValue(
      JSON.stringify({ customer_id: 'C-42', force: true }, null, 2)
    );
  });

  test('AC-103 groups workflow start inputs by source', async () => {
    await renderPanel(
      createTriggerContext({
        triggerType: 'extension',
        mapping: null
      })
    );

    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByText('Path 参数')).toBeInTheDocument();
    expect(within(dialog).getByText('Query 参数')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('customer_id')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('force')).toBeInTheDocument();
  });

  test('AC-102/104 submits node-keyed input and displays Workflow Result with trace action', async () => {
    const detail = createSucceededDetail();
    const startRun = vi.fn().mockResolvedValue(detail);
    const { document, onOpenTrace } = await renderPanel(
      createTriggerContext(),
      vi.fn(),
      startRun
    );

    await change(await screen.findByLabelText('输入 payload'), {
      target: {
        value: JSON.stringify({ customer_id: 'C-42', force: true })
      }
    });
    await submit(
      screen.getByRole('form', { name: 'Workflow 测试运行表单' })
    );

    await waitFor(() => {
      expect(startRun).toHaveBeenCalledWith(
        'app-1',
        {
          document,
          input_payload: {
            'node-workflow-start': {
              customer_id: 'C-42',
              force: true
            }
          }
        },
        'csrf-123'
      );
    });

    expect(await screen.findByText('Workflow Result')).toBeInTheDocument();
    expect(screen.getByText('ticket-C-42')).toBeInTheDocument();
    expect(screen.getByText('已成功')).toBeInTheDocument();
    expect(screen.getByText('Trigger Delivery')).toBeInTheDocument();
    expect(
      screen.getByText('定时触发测试（不进入真实调度队列）')
    ).toBeInTheDocument();
    expect(screen.getByText('run-1')).toBeInTheDocument();

    await click(screen.getByRole('button', { name: '查看执行 Trace' }));
    expect(onOpenTrace).toHaveBeenCalledWith('run-1');
  });

  test('AC-106 shows a formal failure message without rendering the error object', async () => {
    const startRun = vi
      .fn()
      .mockRejectedValue(new Error('Workflow input validation failed'));
    await renderPanel(createTriggerContext(), vi.fn(), startRun);

    await change(await screen.findByLabelText('输入 payload'), {
      target: { value: JSON.stringify({ customer_id: 'C-42' }) }
    });
    await submit(
      screen.getByRole('form', { name: 'Workflow 测试运行表单' })
    );

    expect(
      await screen.findByText('Workflow input validation failed')
    ).toBeInTheDocument();
    expect(screen.queryByText('[object Object]')).not.toBeInTheDocument();
  });
});

async function renderSettled(...args: Parameters<typeof render>) {
  let view!: ReturnType<typeof render>;
  await act(async () => {
    view = render(...args);
  });
  return view;
}
async function click(...args: Parameters<typeof fireEvent.click>) {
  await act(async () => {
    fireEvent.click(...args);
  });
}
async function change(...args: Parameters<typeof fireEvent.change>) {
  await act(async () => {
    fireEvent.change(...args);
  });
}
async function submit(...args: Parameters<typeof fireEvent.submit>) {
  await act(async () => {
    fireEvent.submit(...args);
  });
}
