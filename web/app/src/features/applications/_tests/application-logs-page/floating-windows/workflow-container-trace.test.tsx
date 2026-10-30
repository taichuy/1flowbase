import '../../../../agent-flow/_tests/debug-console/trajectory/navigation';
import { runtimeApi, trajectoryApi } from './floating-windows.fixtures';
import { App as AntdApp } from 'antd';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { expect, test } from 'vitest';
import type {
  NodeExecutionIdentity,
  WorkflowTrajectoryEvent,
  WorkflowTrajectoryPage
} from '@1flowbase/api-client';
import { AppProviders } from '../../../../../app/AppProviders';
import { ApplicationLogsPage } from '../../../pages/ApplicationLogsPage';
import {
  applicationRunsPage,
  sampleTraceTree
} from './floating-windows.support';

function event(
  node_id: string,
  node_type: string,
  node_alias: string,
  execution_id: string,
  round_path: NodeExecutionIdentity['round_path'] = []
): WorkflowTrajectoryEvent {
  return {
    event_id: `event:${execution_id}`,
    event_sequence: 1,
    event_type: 'node_finished',
    created_at: '2026-04-17T09:00:01Z',
    category: 'nodes',
    flow_run_id: 'run-1',
    task_run_id: null,
    parent_task_run_id: null,
    node_run_id: `receipt:${execution_id}`,
    node_id,
    node_type,
    node_alias,
    status: 'succeeded',
    preview: '',
    native_step: null,
    execution_identity: {
      execution_id,
      node_id,
      parent_execution_id: round_path.at(-1)?.container_execution_id ?? null,
      round_path
    }
  };
}
const outer = {
  container_execution_id: 'logical-outer',
  container_node_id: 'outer',
  round_index: 1
};
const iteration = {
  container_execution_id: 'logical-iteration',
  container_node_id: 'iteration',
  round_index: 1
};
const inner = {
  container_execution_id: 'logical-inner',
  container_node_id: 'inner',
  round_index: 0
};
const leaf = event('leaf', 'template', 'Template leaf', 'logical-leaf-110', [
  outer,
  iteration,
  inner
]);
const sibling = event('leaf', 'template', 'Template leaf', 'logical-leaf-100', [
  outer,
  { ...iteration, round_index: 0 },
  { ...inner, container_execution_id: 'logical-inner-other' }
]);
const events = [
  event('code', 'code', 'Parse items', 'logical-code'),
  event('outer', 'loop', 'Outer loop', outer.container_execution_id),
  event(
    'iteration',
    'iteration',
    'Iteration',
    iteration.container_execution_id,
    [outer]
  ),
  event('inner', 'loop', 'Inner loop', inner.container_execution_id, [
    outer,
    iteration
  ]),
  event('inner', 'loop', 'Inner loop', 'logical-inner-other', [
    outer,
    { ...iteration, round_index: 0 }
  ]),
  leaf,
  sibling
];
function page(items = events): WorkflowTrajectoryPage {
  return {
    items,
    overview: {
      items,
      total_count: items.length,
      category_counts: { nodes: items.length },
      time_start: '2026-04-17T09:00:00Z',
      time_end: '2026-04-17T09:00:01Z'
    },
    nodes: items.map(
      ({
        flow_run_id,
        node_run_id,
        node_id,
        node_alias,
        node_type,
        execution_identity
      }) => ({
        execution_identity,
        flow_run_id,
        node_run_id: node_run_id!,
        node_id: node_id!,
        node_alias: node_alias!,
        node_type: node_type!
      })
    ),
    next_cursor: null,
    time_start: '2026-04-17T09:00:00Z',
    time_end: '2026-04-17T09:00:01Z'
  };
}
function openWorkflowTrace() {
  window.history.replaceState(
    {},
    '',
    '/applications/app-1/logs?run_id=run-1&view=trace'
  );
  runtimeApi.fetchApplicationRuns.mockResolvedValue(applicationRunsPage([]));
  render(
    <AppProviders>
      <AntdApp>
        <ApplicationLogsPage applicationId="app-1" applicationType="workflow" />
      </AntdApp>
    </AppProviders>
  );
}

test('formal Code/Template-only Workflow trace selects the backend 1/1/0 receipt and retains node/full-run details', async () => {
  trajectoryApi.fetchWorkflowTrajectory.mockImplementation(
    async (_app, _run, _cursor, options) => ({
      ...page(),
      items: options?.index_only ? [] : events
    })
  );
  trajectoryApi.fetchWorkflowTrajectoryBody.mockResolvedValue({
    event_id: leaf.event_id,
    sections: [{ kind: 'output', value: { result: '1:2' } }]
  });
  runtimeApi.fetchApplicationRunTraceTree.mockResolvedValue({
    nodes: events.map((item) => ({
      trace_node_id: `node_run:${item.node_run_id}`,
      node_kind: 'node_run',
      flow_run_id: 'run-1',
      node_run_id: item.node_run_id,
      node_id: item.node_id,
      node_type: item.node_type,
      node_alias: item.node_alias,
      status: 'succeeded',
      started_at: item.created_at,
      has_content: true,
      has_children: false
    }))
  });
  openWorkflowTrace();
  const filter = await screen.findByRole('combobox', {
    name: '按循环轮次筛选'
  });
  expect(runtimeApi.fetchApplicationRunTraceTree).not.toHaveBeenCalled();
  expect(
    runtimeApi.fetchApplicationRunConversationMessages
  ).not.toHaveBeenCalled();
  expect(trajectoryApi.fetchWorkflowTrajectory).toHaveBeenCalledWith(
    'app-1',
    'run-1',
    undefined,
    expect.objectContaining({ index_only: true })
  );
  fireEvent.mouseDown(filter);
  for (const title of ['Outer loop · 第 2 轮', 'Iteration · 第 2 轮']) {
    const label = await screen.findByTitle(title);
    const popup = label.closest('.ant-select-dropdown');
    const window = screen.getByTestId('application-logs-floating-run-detail');
    // Real AntD portal and explicit slot: do not clip the menu inside the window
    // or let its default global layer fall behind the active window's timeline.
    expect(window).not.toContainElement(popup as HTMLElement);
    expect(popup).toHaveStyle({ zIndex: Number(window.style.zIndex) + 1 });
    const switcher = label
      .closest('.ant-select-tree-treenode')
      ?.querySelector('.ant-select-tree-switcher');
    expect(switcher).not.toBeNull();
    fireEvent.click(switcher!);
  }
  fireEvent.click(await screen.findByTitle('Inner loop · 第 1 轮'));
  const selected = await screen.findByRole('button', {
    name: `节点结束 · Template leaf · ${leaf.node_run_id}`
  });
  await waitFor(() =>
    expect(
      screen.queryByRole('button', {
        name: `节点结束 · Template leaf · ${sibling.node_run_id}`
      })
    ).not.toBeInTheDocument()
  );
  expect(selected).toHaveTextContent(
    'Outer loop · 第 2 轮 / Iteration · 第 2 轮 / Inner loop · 第 1 轮'
  );
  fireEvent.click(selected);
  expect(selected).toHaveAttribute('aria-pressed', 'true');
  await waitFor(() =>
    expect(trajectoryApi.fetchWorkflowTrajectoryBody).toHaveBeenCalledWith(
      'app-1',
      'run-1',
      leaf.event_id
    )
  );
  const inspector = screen.getByRole('complementary', { name: '步骤检查器' });
  fireEvent.click(within(inspector).getByRole('tab', { name: '原文' }));
  expect(
    await within(inspector).findByText('event:logical-leaf-110', {
      exact: false
    })
  ).toBeInTheDocument();
  fireEvent.click(
    screen.getByText('追踪节点', { selector: '.ant-segmented-item-label' })
  );
  expect(
    await screen.findByRole('button', { name: /Parse items.*code/ })
  ).toBeInTheDocument();
  expect(runtimeApi.fetchApplicationRunTraceTree).toHaveBeenCalledWith(
    'app-1',
    'run-1'
  );
  const detail = screen.getByTestId('application-logs-floating-run-detail');
  expect(
    within(detail).getByRole('button', { name: '导出当前运行 JSON' })
  ).toBeInTheDocument();
  fireEvent.click(within(detail).getByRole('tab', { name: '详情' }));
  await waitFor(() =>
    expect(runtimeApi.fetchApplicationRunOverview).toHaveBeenCalledWith(
      'app-1',
      'run-1'
    )
  );
});

test.each(['code', 'llm'])(
  'no-container %s workflow keeps its existing node tree and detail entry',
  async (node_type) => {
    const node_alias = node_type === 'llm' ? 'LLM' : 'Parse items';
    trajectoryApi.fetchWorkflowTrajectory.mockResolvedValue(
      page([event('ordinary', node_type, node_alias, 'ordinary')])
    );
    const tree = sampleTraceTree();
    tree.nodes[0] = { ...tree.nodes[0], node_type, node_alias };
    runtimeApi.fetchApplicationRunTraceTree.mockResolvedValue(tree);
    openWorkflowTrace();
    fireEvent.click(
      await screen.findByRole('button', {
        name: new RegExp(`${node_alias}.*${node_type}`)
      })
    );
    expect(
      await screen.findByRole('region', { name: `${node_alias} 节点详情` })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('combobox', { name: '按循环轮次筛选' })
    ).not.toBeInTheDocument();
    if (node_type === 'llm')
      expect(
        await screen.findByRole('button', { name: '调用轨迹' })
      ).toBeInTheDocument();
    expect(
      runtimeApi.fetchApplicationRunConversationMessages
    ).not.toHaveBeenCalled();
  }
);

test('a failed workflow index is visible and retryable without removing legacy trace access', async () => {
  trajectoryApi.fetchWorkflowTrajectory.mockRejectedValue(
    new Error('index unavailable')
  );
  openWorkflowTrace();
  expect(await screen.findByRole('alert')).toHaveTextContent('加载失败');
  expect(
    await screen.findByRole('button', { name: /LLM.*llm/ })
  ).toBeInTheDocument();
  trajectoryApi.fetchWorkflowTrajectory.mockResolvedValue(page());
  fireEvent.click(screen.getByRole('button', { name: /^重\s*试$/ }));
  expect(
    await screen.findByRole('combobox', { name: '按循环轮次筛选' })
  ).toBeInTheDocument();
});
