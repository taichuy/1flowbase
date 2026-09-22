import './navigation';
import { workflowNative, workflowPage } from './workflow-fixture';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, expect, test, vi } from 'vitest';
import type {
  ClientTrajectoryStep,
  ProviderTrajectoryStep
} from '@1flowbase/api-client';
import { ProviderTrajectory } from '../../../components/debug-console/trajectory/ProviderTrajectory';
import type { ConversationLogTraceLoader } from '../../../components/debug-console/conversation-log-trace-model';
import { appI18n } from '../../../../../shared/i18n/app-i18n';

beforeEach(async () => {
  await appI18n.changeLanguage('zh_Hans');
});
const request: ClientTrajectoryStep = {
  id: 'request-current',
  request_id: 'request-current',
  flow_run_id: 'run-current',
  sequence: 900,
  created_at: '2026-09-22T00:00:00Z',
  category: 'request',
  name: 'request',
  namespace: null,
  preview: 'Current request',
  parameters_preview: null,
  result_preview: null,
  status: 'submitted',
  origin: 'submitted',
  protocol: 'responses',
  transport: 'websocket',
  node_run_id: null,
  parent_id: null,
  call_id: null,
  item_id: null,
  response_id: null,
  turn_id: null,
  related_step_id: null,
  available_sections: []
};
function invocation(
  id: string,
  attempt: number,
  purpose: ProviderTrajectoryStep['metadata']['purpose']
): ProviderTrajectoryStep {
  return {
    event_id: id,
    event_sequence: 950 + attempt,
    event_type: 'provider_semantic_step',
    created_at: request.created_at,
    metadata: {
      source: 'ai_native',
      kind: 'model_call',
      status: 'recorded',
      step_key: id,
      preview: id,
      flow_run_id: request.flow_run_id,
      node_id: 'llm',
      node_run_id: 'node-current',
      invocation_id: 'invocation-one',
      provider_attempt_index: attempt,
      purpose
    },
    links: [
      {
        relation: 'trigger',
        flow_run_id: request.flow_run_id,
        request_id: request.id
      },
      {
        relation: 'context',
        flow_run_id: 'run-previous',
        request_id: 'request-previous'
      }
    ]
  };
}
function fixture(nodeRunId?: string) {
  const loadClientTrajectory = vi
    .fn()
    .mockImplementation((run, _node, _cursor, options) =>
      Promise.resolve({
        items: [
          {
            ...request,
            id: options?.request_id ?? request.id,
            request_id: options?.request_id ?? request.id,
            flow_run_id: run,
            preview:
              run === 'run-previous' ? 'Previous request' : request.preview
          }
        ],
        next_cursor: null,
        integrity: 'complete'
      })
    );
  const loadWorkflowTrajectory = vi
    .fn()
    .mockResolvedValue(
      workflowPage([
        workflowNative(invocation('first-call', 0, 'prewarm')),
        workflowNative(invocation('retry-call', 1, 'generate'))
      ])
    );
  const loadTrajectoryBody = vi.fn().mockResolvedValue({
    source: 'ai_native',
    evidence_scope: 'step',
    event_id: 'first-call',
    sections: [
      { kind: 'system', value: 'Actual workflow system' },
      { kind: 'output', value: '' }
    ],
    items: [
      {
        event_id: 'body',
        sequence: 1,
        encoding: 'utf8',
        body: '{"raw":"retained"}'
      }
    ],
    next_cursor: null
  });
  const loader: ConversationLogTraceLoader = {
    loadTree: vi.fn(),
    loadChildren: vi.fn(),
    loadContent: vi.fn(),
    loadClientTrajectory,
    loadWorkflowTrajectory,
    loadTrajectoryBody
  };
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <ProviderTrajectory
        runId={request.flow_run_id}
        nodeRunId={nodeRunId}
        loader={loader}
      />
    </QueryClientProvider>
  );
  return { loadClientTrajectory, loadWorkflowTrajectory, loadTrajectoryBody };
}
test('opens request-linked invocation choices, resolves cross-run source by focus, and restores the original view', async () => {
  const { loadClientTrajectory, loadWorkflowTrajectory } = fixture();
  fireEvent.click(screen.getByRole('button', { name: '总轨迹' }));
  fireEvent.click(
    await screen.findByRole('button', { name: /Current request/ })
  );
  const search = screen.getByRole('textbox', { name: '搜索已加载步骤' });
  fireEvent.change(search, { target: { value: 'Current request' } });
  const ledger = search
    .closest('.provider-trajectory')!
    .querySelector('.provider-trajectory__ledger')!;
  ledger.scrollTop = 123;
  fireEvent.click(screen.getByRole('button', { name: '查看关联内部调用' }));
  await waitFor(() =>
    expect(loadWorkflowTrajectory).toHaveBeenCalledWith(
      'run-current',
      undefined,
      {
        category: 'all',
        node_run_id: undefined,
        from: undefined,
        to: undefined,
        request_id: 'request-current'
      }
    )
  );
  await screen.findAllByRole('button', {name: /^模型调用准备 ·/});
  fireEvent.click(screen.getByRole('button', {name: '调用分组'}));
  const calls = await screen.findAllByRole('button', {
    name: /^模型调用准备 ·/
  });
  expect(calls).toHaveLength(2);
  expect(screen.getByText('预热')).toBeInTheDocument();
  expect(screen.getByText('生成')).toBeInTheDocument();
  expect(screen.queryByRole('complementary')).not.toBeInTheDocument();
  fireEvent.click(calls[0]);
  await screen.findByRole('region', { name: '实际系统输入' });
  expect(screen.getByText('Actual workflow system')).toBeInTheDocument();
  expect(screen.queryByText(/retained/)).not.toBeInTheDocument();
  fireEvent.click(
    screen.getByRole('button', { name: '上下文来源请求 · request-previous' })
  );
  await waitFor(() =>
    expect(loadClientTrajectory).toHaveBeenLastCalledWith(
      'run-previous',
      undefined,
      undefined,
      { request_id: 'request-previous', focus_step_id: 'request-previous' }
    )
  );
  expect(
    await screen.findByRole('button', { name: /Previous request/ })
  ).toHaveAttribute('aria-pressed', 'true');
  expect(loadClientTrajectory).toHaveBeenCalledTimes(2);
  expect(loadWorkflowTrajectory).toHaveBeenCalledTimes(1);
  expect(
    screen.queryByRole('button', { name: '返回上一视图' })
  ).not.toBeInTheDocument();
  fireEvent.click(screen.getByText('工作流内部事件'));
  fireEvent.click(
    screen.getByText('客户端请求', { selector: '.ant-segmented-item-label' })
  );
  expect(screen.getByRole('textbox', { name: '搜索已加载步骤' })).toHaveValue(
    'Current request'
  );
  expect(
    screen.getByRole('button', { name: /Current request/ })
  ).toHaveAttribute('aria-pressed', 'true');
  expect(ledger.scrollTop).toBe(123);
});
test('node entry defaults to chronological internal events and historical missing metadata stays unknown', async () => {
  const { loadClientTrajectory, loadWorkflowTrajectory } =
    fixture('node-current');
  const historical = invocation('historical-call', 0, 'unknown');
  historical.links = [];
  loadWorkflowTrajectory.mockResolvedValue(
    workflowPage([workflowNative(historical)])
  );
  fireEvent.click(screen.getByRole('button', { name: '调用轨迹' }));
  fireEvent.click(
    await screen.findByRole('button', { name: /^模型调用准备 ·/ })
  );
  expect(loadClientTrajectory).not.toHaveBeenCalled();
  expect(screen.getByRole('button', { name: '调用分组' })).toHaveAttribute(
    'aria-pressed',
    'false'
  );
  const detail = screen.getByRole('complementary');
  expect(within(detail).getByText('用途未知')).toBeInTheDocument();
  expect(within(detail).getByText('请求来源未记录')).toBeInTheDocument();
  fireEvent.click(within(detail).getByRole('tab', { name: '内部事件原文' }));
  expect(await within(detail).findByText(/retained/)).toBeInTheDocument();
});
test('request without linked invocations shows an empty state without inventing calls', async () => {
  const { loadWorkflowTrajectory } = fixture();
  loadWorkflowTrajectory.mockResolvedValue(workflowPage([]));
  fireEvent.click(screen.getByRole('button', { name: '总轨迹' }));
  fireEvent.click(
    await screen.findByRole('button', { name: /Current request/ })
  );
  fireEvent.click(screen.getByRole('button', { name: '查看关联内部调用' }));
  expect(
    await screen.findByText('此请求没有关联的内部调用')
  ).toBeInTheDocument();
});

test.each([
  ['model_reply', 1250, '耗时: 1.25 s'],
  ['error', 0, '耗时: 0 ms'],
  ['model_reply', null, null],
  ['model_call', undefined, null]
] as const)(
  'shows recorded %s duration %s without inventing missing timing',
  async (kind, duration_ms, text) => {
    const { loadWorkflowTrajectory } = fixture('node-current');
    const event = invocation('timed-event', 0, 'generate');
    event.metadata.kind = kind;
    event.metadata.duration_ms = duration_ms;
    loadWorkflowTrajectory.mockResolvedValue(
      workflowPage([workflowNative(event)])
    );
    fireEvent.click(screen.getByRole('button', { name: '调用轨迹' }));
    const label =
      kind === 'model_reply'
        ? '模型回复'
        : kind === 'error'
          ? '调用错误'
          : '模型调用准备';
    fireEvent.click(
      await screen.findByRole('button', { name: new RegExp(`^${label} ·`) })
    );
    const detail = within(screen.getByRole('complementary'));
    if (text) expect(detail.getByText(text)).toBeInTheDocument();
    else expect(detail.queryByText(/^耗时:/)).not.toBeInTheDocument();
  }
);
