import './navigation';
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
  ProviderTrajectoryStep,
  WorkflowTrajectoryEvent
} from '@1flowbase/api-client';
import { NativeTrajectoryWorkspace } from '../../../components/debug-console/trajectory/NativeTrajectoryWorkspace';
import { ProviderTrajectory } from '../../../components/debug-console/trajectory/ProviderTrajectory';
import type { ConversationLogTraceLoader } from '../../../components/debug-console/conversation-log-trace-model';
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import { workflowNative, workflowPage } from './workflow-fixture';

beforeEach(async () => {
  await appI18n.changeLanguage('zh_Hans');
});
const native: ProviderTrajectoryStep = {
  event_id: 'exact-native-locator',
  event_sequence: 7,
  event_type: 'provider_semantic_step',
  created_at: '2026-09-22T00:01:00Z',
  links: [],
  metadata: {
    source: 'ai_native',
    purpose: 'generate',
    kind: 'model_call',
    status: 'recorded',
    step_key: 'call',
    preview: 'first request',
    flow_run_id: 'run-current',
    node_id: 'llm-one',
    node_run_id: 'node-current',
    invocation_id: 'call-one',
    provider_attempt_index: 0
  }
};
const first = workflowNative(native, '规划节点');
const second = workflowNative(
  {
    ...native,
    event_id: 'second-native',
    metadata: {
      ...native.metadata,
      node_id: 'llm-two',
      node_run_id: 'node-two',
      invocation_id: 'call-two'
    }
  },
  '总结节点'
);
const node: WorkflowTrajectoryEvent = {
  ...first,
  event_id: 'node:started',
  native_step: null,
  event_type: 'node_started',
  category: 'nodes',
  preview: 'started',
  parent_task_run_id: 'actual-parent',
  task_run_id: 'child-task'
};
function fixture({
  floating = false,
  request_id
}: { floating?: boolean; request_id?: string } = {}) {
  const loadWorkflowTrajectory = vi
    .fn()
    .mockResolvedValue(workflowPage([first, second, node]));
  const loadWorkflowTrajectoryBody = vi.fn().mockResolvedValue({
    event_id: node.event_id,
    sections: [
      { kind: 'input', value: 'event input' },
      { kind: 'node', value: { status: 'running' } }
    ]
  });
  const loadTrajectoryBody = vi.fn().mockResolvedValue({
    event_id: native.event_id,
    source: 'ai_native',
    evidence_scope: 'step',
    sections: [
      { kind: 'system', value: 'Actual system input' },
      { kind: 'configuration', value: 'event configuration' },
      { kind: 'output', value: 'event output' }
    ],
    items: [],
    next_cursor: null
  });
  const loader: ConversationLogTraceLoader = {
    loadTree: vi.fn().mockResolvedValue({
      nodes: [
        {
          trace_node_id: 'trace-node',
          node_kind: 'node_run',
          node_run_id: 'node-current',
          node_id: 'llm-one',
          node_alias: '规划节点',
          node_type: 'llm',
          status: 'succeeded',
          started_at: node.created_at,
          has_children: true,
          has_content: true
        }
      ]
    }),
    loadChildren: vi.fn().mockResolvedValue({
      items: [],
      page_info: { has_more: false, page_size: 50 }
    }),
    loadContent: vi.fn().mockResolvedValue({
      trace_node_id: 'trace-node',
      node_kind: 'node_run',
      payload: {
        input_payload: { prompt: 'recorded input' },
        output_payload: { text: 'recorded output' }
      }
    }),
    loadWorkflowTrajectory,
    loadWorkflowTrajectoryBody,
    loadTrajectoryBody
  };
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      {floating ? (
        <ProviderTrajectory
          runId="run-current"
          nodeRunId="node-current"
          loader={loader}
        />
      ) : (
        <NativeTrajectoryWorkspace
          runId="run-current"
          loader={loader}
          options={request_id ? { request_id } : undefined}
        />
      )}
    </QueryClientProvider>
  );
  return {
    loader,
    loadWorkflowTrajectory,
    loadWorkflowTrajectoryBody,
    loadTrajectoryBody
  };
}
test('keeps every node name visible in rows, invocation groups and selected detail with native exact bodies', async () => {
  const { loadTrajectoryBody, loadWorkflowTrajectoryBody } = fixture();
  const rows = await screen.findAllByRole('button', {
    name: /^模型调用准备 ·/
  });
  expect(rows[0]).toHaveAccessibleName(/规划节点/);
  expect(rows[1]).toHaveAccessibleName(/总结节点/);
  expect(within(rows[0]).getByText('规划节点')).toBeInTheDocument();
  expect(within(rows[1]).getByText('总结节点')).toBeInTheDocument();
  expect(document.querySelector('.provider-trajectory__group')).toBeNull();
  fireEvent.click(screen.getByRole('button', { name: '调用分组' }));
  expect(
    screen.getByRole('button', { name: /总结节点.*生成.*call-two/ })
  ).toBeInTheDocument();
  expect(loadTrajectoryBody).not.toHaveBeenCalled();
  expect(loadWorkflowTrajectoryBody).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getAllByRole('button', { name: /^模型调用准备 ·/ })[1]
  );
  expect(
    within(screen.getByRole('complementary')).getByText('总结节点')
  ).toBeInTheDocument();
  await waitFor(() =>
    expect(loadTrajectoryBody).toHaveBeenCalledWith(
      'run-current',
      'node-two',
      'second-native',
      undefined,
      'semantic'
    )
  );
  expect(loadWorkflowTrajectoryBody).not.toHaveBeenCalled();
});
test('filters on the server, resets cursors, retains the same ledger and reuses category cache', async () => {
  const { loadWorkflowTrajectory, loadTrajectoryBody } = fixture({
    request_id: 'request-exact'
  });
  await screen.findAllByRole('button', { name: /^模型调用准备 ·/ });
  const ledger = screen.getByRole('region', { name: '顺序步骤' });
  const timeline = screen.getByTestId('trajectory-brush');
  loadWorkflowTrajectory.mockResolvedValue(workflowPage([node]));
  fireEvent.click(
    screen.getByText('节点执行', { selector: '.ant-segmented-item-label' })
  );
  await waitFor(() =>
    expect(loadWorkflowTrajectory).toHaveBeenLastCalledWith(
      'run-current',
      undefined,
      {
        category: 'nodes',
        node_run_id: undefined,
        request_id: 'request-exact',
        from: undefined,
        to: undefined
      }
    )
  );
  expect(screen.getByRole('region', { name: '顺序步骤' })).toBe(ledger);
  expect(screen.getByTestId('trajectory-brush')).toBe(timeline);
  expect(
    await screen.findByRole('button', { name: /^节点开始 ·/ })
  ).toBeInTheDocument();
  expect(
    screen.queryByRole('button', { name: /^模型调用准备 ·/ })
  ).not.toBeInTheDocument();
  fireEvent.click(
    screen.getByText('所有事件', { selector: '.ant-segmented-item-label' })
  );
  await screen.findAllByRole('button', { name: /^模型调用准备 ·/ });
  expect(loadWorkflowTrajectory).toHaveBeenCalledTimes(2);
  expect(loadTrajectoryBody).not.toHaveBeenCalled();
});
test('paginates summaries only and displays true parent relationships without inventing native bodies', async () => {
  const {
    loadWorkflowTrajectory,
    loadWorkflowTrajectoryBody,
    loadTrajectoryBody
  } = fixture();
  loadWorkflowTrajectory
    .mockResolvedValueOnce(
      workflowPage(
        [{ ...node, category: 'tools', event_type: 'tool_callback_completed' }],
        'opaque-next'
      )
    )
    .mockResolvedValue(workflowPage([second]));
  // The first request starts at render; change categories to exercise this response chain.
  await screen.findAllByRole('button', { name: /^模型调用准备 ·/ });
  fireEvent.click(
    screen.getByText('子任务', { selector: '.ant-segmented-item-label' })
  );
  await waitFor(() =>
    expect(loadWorkflowTrajectory).toHaveBeenCalledWith(
      'run-current',
      'opaque-next',
      expect.objectContaining({ category: 'agents' })
    )
  );
  expect(loadWorkflowTrajectoryBody).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: /^工具回调完成 ·/ }));
  fireEvent.click(
    within(screen.getByRole('complementary')).getByRole('tab', {
      name: '元数据'
    })
  );
  expect(
    within(screen.getByRole('complementary')).getByText('父任务: actual-parent')
  ).toBeInTheDocument();
  await waitFor(() =>
    expect(loadWorkflowTrajectoryBody).toHaveBeenCalledWith(
      'run-current',
      'node:started'
    )
  );
  expect(loadTrajectoryBody).not.toHaveBeenCalled();
});
test('opens lazily in the existing floating workspace and restores focus when detail closes', async () => {
  const { loadWorkflowTrajectory, loadTrajectoryBody } = fixture({
    floating: true
  });
  expect(loadWorkflowTrajectory).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '调用轨迹' }));
  expect(await screen.findByRole('dialog', { name: '调用轨迹' })).toHaveClass(
    'window-workspace-window'
  );
  const [row] = await screen.findAllByRole('button', {
    name: /^模型调用准备 ·/
  });
  fireEvent.click(row);
  await screen.findByText('Actual system input');
  fireEvent.click(screen.getByRole('button', { name: '关闭步骤检查器' }));
  expect(row).toHaveFocus();
  fireEvent.click(row);
  await screen.findByText('Actual system input');
  expect(loadTrajectoryBody).toHaveBeenCalledTimes(1);
  expect(loadWorkflowTrajectory).toHaveBeenCalledWith(
    'run-current',
    undefined,
    expect.objectContaining({ node_run_id: 'node-current' })
  );
});

test('time brush sends RFC3339 bounds and keeps the full scope domain after filtering', async () => {
  const { loadWorkflowTrajectory } = fixture();
  await screen.findAllByRole('button', { name: /^模型调用准备 ·/ });
  const track = screen.getByTestId('trajectory-brush');
  vi.spyOn(track, 'getBoundingClientRect').mockReturnValue({
    left: 0,
    width: 100
  } as DOMRect);
  track.setPointerCapture = vi.fn();
  vi.stubGlobal('PointerEvent', MouseEvent);
  try {
    fireEvent.pointerDown(track, { button: 0, clientX: 25 });
    fireEvent.pointerMove(track, { clientX: 75 });
    fireEvent.pointerUp(track, { clientX: 75 });
    await waitFor(() =>
      expect(loadWorkflowTrajectory).toHaveBeenLastCalledWith(
        'run-current',
        undefined,
        expect.objectContaining({
          from: '2026-09-22T00:15:00.000Z',
          to: '2026-09-22T00:45:00.000Z'
        })
      )
    );
    expect(screen.getAllByRole('slider')[0]).toHaveAttribute(
      'aria-valuemin',
      String(Date.parse('2026-09-22T00:00:00Z'))
    );
    expect(screen.getAllByRole('slider')[1]).toHaveAttribute(
      'aria-valuemax',
      String(Date.parse('2026-09-22T01:00:00Z'))
    );
  } finally {
    vi.unstubAllGlobals();
  }
});

test('node filtering sends the exact execution identity instead of matching a display name', async () => {
  const { loadWorkflowTrajectory } = fixture();
  await screen.findAllByRole('button', { name: /^模型调用准备 ·/ });
  fireEvent.mouseDown(screen.getByRole('combobox', { name: '筛选节点执行' }));
  fireEvent.click(await screen.findByText('执行时节点名称 · llm'));
  await waitFor(() =>
    expect(loadWorkflowTrajectory).toHaveBeenLastCalledWith(
      'run-current',
      undefined,
      expect.objectContaining({ category: 'all', node_run_id: 'node-current' })
    )
  );
});

test('shows only selected event sections without loading the node tree or borrowing final output', async () => {
  const { loader, loadWorkflowTrajectoryBody } = fixture();
  fireEvent.click(await screen.findByRole('button', { name: /^节点开始 ·/ }));
  const inspector = within(screen.getByRole('complementary'));
  expect(await inspector.findByText(/event input/)).toBeInTheDocument();
  expect(inspector.getAllByRole('tab').map((tab) => tab.textContent)).toEqual([
    '输入',
    '数据处理',
    '输出',
    '元数据',
    '原文'
  ]);
  fireEvent.click(inspector.getByRole('tab', { name: '输出' }));
  expect(await inspector.findByText('未记录此项内容')).toBeInTheDocument();
  expect(inspector.queryByText(/recorded output/)).not.toBeInTheDocument();
  expect(loader.loadTree).not.toHaveBeenCalled();
  expect(loader.loadChildren).not.toHaveBeenCalled();
  expect(loader.loadContent).not.toHaveBeenCalled();
  expect(loadWorkflowTrajectoryBody).toHaveBeenCalledExactlyOnceWith(
    'run-current',
    'node:started'
  );
});

test('switches native input process and output sections without mixing data or refetching the event', async () => {
  const { loadTrajectoryBody } = fixture();
  fireEvent.click(
    (await screen.findAllByRole('button', { name: /^模型调用准备 ·/ }))[0]
  );
  const inspector = within(screen.getByRole('complementary'));
  expect(await inspector.findByText('Actual system input')).toBeInTheDocument();
  expect(inspector.queryByText('event configuration')).not.toBeInTheDocument();
  fireEvent.click(inspector.getByRole('tab', { name: '数据处理' }));
  expect(await inspector.findByText('event configuration')).toBeInTheDocument();
  expect(inspector.queryByText('Actual system input')).not.toBeInTheDocument();
  fireEvent.click(inspector.getByRole('tab', { name: '输出' }));
  expect(await inspector.findByText('event output')).toBeInTheDocument();
  expect(inspector.queryByText('event configuration')).not.toBeInTheDocument();
  expect(loadTrajectoryBody).toHaveBeenCalledTimes(1);
});
