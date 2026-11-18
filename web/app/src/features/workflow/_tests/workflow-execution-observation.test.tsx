import { act, fireEvent, render, screen } from '@testing-library/react';
import { expect, test, vi } from 'vitest';
import { AppProviders } from '../../../app/AppProviders';
import { WorkflowRunTrace } from '../../agent-flow/components/debug-console/trajectory/workflow/WorkflowRunTrace';
import { NodeRunSummaryCard } from '../../agent-flow/components/detail/last-run/NodeRunSummaryCard';
import { NodeRunPayloadSections } from '../../agent-flow/components/detail/last-run/NodeRunPayloadSections';
import { WorkflowRunFeedback } from '../components/WorkflowRunFeedback';
import { emptyWorkflowRunState, workflowRunSnapshot } from '../lib/run-state';
import type {
  NodeLastRun,
  FlowDebugRunDetail
} from '../../agent-flow/api/runtime';

test('AC-002 shows tracking nodes first without waiting for the full trajectory index', () => {
  const loadWorkflowTrajectory = vi.fn();
  render(
    <AppProviders>
      <WorkflowRunTrace runId="run" loader={{ loadWorkflowTrajectory }}>
        <div>Node execution groups</div>
      </WorkflowRunTrace>
    </AppProviders>
  );
  expect(screen.getByText('Node execution groups')).toBeVisible();
  expect(screen.getByText('追踪节点').closest('label')).toHaveClass(
    'ant-segmented-item-selected'
  );
  const labels = [
    ...document.querySelectorAll('.ant-segmented-item-label')
  ].map((el) => el.textContent);
  expect(labels).toEqual(['追踪节点', '总轨迹']);
  expect(loadWorkflowTrajectory).not.toHaveBeenCalled();
});

test('AC-004 successful node keeps its own status and duration when the flow fails', () => {
  const lastRun = {
    flow_run: {
      status: 'failed',
      started_at: '2026-11-15T11:00:00Z',
      finished_at: '2026-11-15T11:20:00Z'
    },
    node_run: {
      status: 'succeeded',
      started_at: '2026-11-15T11:00:01Z',
      finished_at: '2026-11-15T11:00:02Z',
      metrics_payload: {}
    }
  } as NodeLastRun;
  render(
    <AppProviders>
      <NodeRunSummaryCard lastRun={lastRun} />
    </AppProviders>
  );
  expect(screen.getByText('succeeded')).toBeVisible();
  expect(screen.getByText('1000')).toBeVisible();
  expect(screen.getByText('整轮状态')).toBeVisible();
  expect(screen.getByText('failed')).toBeVisible();
});

test('AC-003 lazy node payloads expose the failure without opening output JSON', async () => {
  const load = vi.fn().mockResolvedValue(undefined);
  render(
    <AppProviders>
      <NodeRunPayloadSections
        inputPayload={{}}
        debugPayload={{}}
        outputPayload={{}}
        errorPayload={{
          kind: 'http_request_error',
          message:
            'POST /api/console/applications/codex/logs/deletion-jobs timed out after 30000ms'
        }}
        onLoadSection={load}
      />
    </AppProviders>
  );
  expect(screen.getByRole('alert')).toHaveTextContent(
    'codex/logs/deletion-jobs timed out after 30000ms'
  );
  expect(load).not.toHaveBeenCalled();
  await act(async () => {
    fireEvent.click(screen.getByText('输入', { exact: true }));
  });
  expect(load.mock.calls).toEqual([['input_payload']]);
  expect(screen.getByRole('alert')).toHaveTextContent(
    'codex/logs/deletion-jobs timed out after 30000ms'
  );
});

test('AC-004 inline trial feedback retains terminal status and failure reason', () => {
  render(
    <AppProviders>
      <WorkflowRunFeedback
        state={{
          ...emptyWorkflowRunState(),
          status: 'failed',
          failure: 'Cleanup request timed out'
        }}
      />
    </AppProviders>
  );
  expect(screen.getByRole('status')).toHaveTextContent('运行失败');
  expect(screen.getByRole('status')).toHaveTextContent(
    'Cleanup request timed out'
  );
});

test('AC-004 reconnect snapshot preserves the durable failure reason', () => {
  const snapshot = workflowRunSnapshot({
    flow_run: {
      id: 'run',
      status: 'failed',
      output_payload: {},
      error_payload: { message: 'Timeout at codex' }
    },
    node_runs: []
  } as unknown as FlowDebugRunDetail);
  expect(snapshot.failure).toBe('Timeout at codex');
});
