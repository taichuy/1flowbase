import '../../../../test/fixtures/rc-util-unique-ids';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { App } from 'antd';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, expect, test, vi } from 'vitest';
import { appI18n } from '../../../../shared/i18n/app-i18n';
import { useAuthStore } from '../../../../state/auth-store';
import { AgentLogsDeleteButton } from '../../components/logs/workspace/AgentLogsDeleteButton';
import { logDeletionJobKey } from '../../api/log-deletion';
import type { AgentLogsDeleteJob } from '@1flowbase/api-client';
const api = vi.hoisted(() => ({
  deleteConsoleApplicationLogs: vi.fn(),
  previewConsoleApplicationLogDeletion: vi.fn(),
  createConsoleApplicationLogDeletionJob: vi.fn(),
  getConsoleApplicationLogDeletionJob: vi.fn(),
  stopConsoleApplicationLogDeletionJob: vi.fn()
}));
vi.mock('@1flowbase/api-client', async (original) => ({
  ...(await original<typeof import('@1flowbase/api-client')>()),
  ...api
}));
let currentJob: AgentLogsDeleteJob | null;
function job(overrides: Partial<AgentLogsDeleteJob> = {}): AgentLogsDeleteJob {
  return {
    job_id: 'fixture-job',
    application_id: 'app-1',
    scope: { mode: 'all_time', batch_size: 100 },
    ingested_at_before: '2026-10-09T00:00:00Z',
    status: 'running',
    total_records: 205,
    deleted_records: 100,
    stop_requested: false,
    error_code: null,
    created_at: '2026-10-09T00:00:00Z',
    updated_at: '2026-10-09T00:00:01Z',
    ...overrides
  };
}
function renderButton() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } }
  });
  const onFinished = vi.fn().mockResolvedValue(undefined);
  const view = render(
    <QueryClientProvider client={client}>
      <App>
        <AgentLogsDeleteButton applicationId="app-1" onFinished={onFinished} />
      </App>
    </QueryClientProvider>
  );
  fireEvent.click(screen.getByRole('button', { name: '删除日志' }));
  return { ...view, onFinished, client };
}
async function choose(label: string) {
  fireEvent.mouseDown(screen.getByRole('combobox', { name: '选择日期' }));
  fireEvent.click(
    await screen.findByText(label, {
      selector: '.ant-select-item-option-content'
    })
  );
}
async function ready() {
  await waitFor(() =>
    expect(
      within(screen.getByRole('dialog')).getByRole('button', {
        name: '删除日志'
      })
    ).toBeEnabled()
  );
}
async function start() {
  await ready();
  fireEvent.click(
    within(screen.getByRole('dialog')).getByRole('button', { name: '删除日志' })
  );
}
beforeEach(async () => {
  vi.clearAllMocks();
  for (const item of Object.values(api)) item.mockReset();
  currentJob = null;
  await appI18n.changeLanguage('zh_Hans');
  useAuthStore.setState({ csrfToken: 'fixture-csrf' });
  api.getConsoleApplicationLogDeletionJob.mockImplementation(async () => ({
    job: currentJob
  }));
  api.previewConsoleApplicationLogDeletion.mockResolvedValue({
    total_records: 205
  });
  api.createConsoleApplicationLogDeletionJob.mockImplementation(
    async (_id, input) => {
      currentJob = job({
        job_id: input.job_id,
        scope: input.scope,
        status: 'queued',
        deleted_records: 0
      });
      return { job: currentJob };
    }
  );
  api.stopConsoleApplicationLogDeletionJob.mockImplementation(async () => {
    currentJob = { ...currentJob!, stop_requested: true };
    return { job: currentJob };
  });
});

test('date selection counts server records and creates one durable job with default batch 100', async () => {
  const { client, onFinished } = renderButton();
  expect(screen.getByRole('spinbutton', { name: '删除批次' })).toHaveValue(
    '100'
  );
  fireEvent.mouseDown(screen.getByRole('combobox', { name: '选择日期' }));
  for (const label of [
    '过去 7 天',
    '过去 30 天',
    '过去 90 天',
    '过去一年',
    '全部',
    '自定义日期'
  ])
    expect(
      await screen.findByText(label, {
        selector: '.ant-select-item-option-content'
      })
    ).toBeInTheDocument();
  fireEvent.click(
    screen.getByText('全部', { selector: '.ant-select-item-option-content' })
  );
  await waitFor(() =>
    expect(
      api.previewConsoleApplicationLogDeletion.mock.calls.at(-1)?.[1]
    ).toEqual({ mode: 'all_time' })
  );
  await start();
  await waitFor(() =>
    expect(api.createConsoleApplicationLogDeletionJob).toHaveBeenCalledOnce()
  );
  expect(
    await screen.findByText('已删除 0 / 205 条日志', {}, { timeout: 4000 })
  ).toBeInTheDocument();
  const input = api.createConsoleApplicationLogDeletionJob.mock.calls[0][1];
  expect(input.scope).toEqual({ mode: 'all_time', batch_size: 100 });
  expect(input.job_id).toMatch(/^[0-9a-f-]{36}$/);
  expect(
    api.previewConsoleApplicationLogDeletion.mock.calls.at(-1)![1]
  ).toEqual({ mode: 'all_time' });
  await act(async () =>
    client.setQueryData(
      logDeletionJobKey('app-1', input.job_id),
      job({ job_id: input.job_id, status: 'succeeded', deleted_records: 205 })
    )
  );
  expect(
    await screen.findByText('已删除 205 / 205 条日志')
  ).toBeInTheDocument();
  expect(screen.getByText('所选范围的日志已删除。')).toBeInTheDocument();
  expect(onFinished).toHaveBeenCalledOnce();
  expect(api.deleteConsoleApplicationLogs).not.toHaveBeenCalled();
  expect(api.createConsoleApplicationLogDeletionJob).toHaveBeenCalledOnce();
});

test.each([
  ['过去 7 天', 7],
  ['过去 30 天', 30],
  ['过去 90 天', 90],
  ['过去一年', 365]
])(
  'preset %s previews and submits the same explicit range',
  async (label, days) => {
    renderButton();
    await choose(String(label));
    await start();
    await waitFor(() =>
      expect(api.createConsoleApplicationLogDeletionJob).toHaveBeenCalledOnce()
    );
    const scope =
      api.createConsoleApplicationLogDeletionJob.mock.calls[0][1].scope;
    expect(scope.mode).toBe('time_range');
    expect(
      new Date(scope.started_at_to).getTime() -
        new Date(scope.started_at_from).getTime()
    ).toBe(Number(days) * 86400000);
    expect(
      api.previewConsoleApplicationLogDeletion.mock.calls.at(-1)![1]
    ).toEqual({
      mode: scope.mode,
      started_at_from: scope.started_at_from,
      started_at_to: scope.started_at_to
    });
  }
);

test('custom date preview includes the entire end day and requires both dates', async () => {
  renderButton();
  await choose('自定义日期');
  expect(await screen.findByText('请选择开始和结束日期')).toBeInTheDocument();
  expect(
    within(screen.getByRole('dialog')).getByRole('button', { name: '删除日志' })
  ).toBeDisabled();
  const inputs = within(screen.getByRole('dialog')).getAllByRole('textbox');
  fireEvent.change(inputs[0], { target: { value: '2026-10-07' } });
  fireEvent.keyDown(inputs[0], { key: 'Enter', code: 'Enter' });
  fireEvent.change(inputs[1], { target: { value: '2026-10-08' } });
  fireEvent.keyDown(inputs[1], { key: 'Enter', code: 'Enter' });
  await start();
  await waitFor(() =>
    expect(api.createConsoleApplicationLogDeletionJob).toHaveBeenCalledOnce()
  );
  const scope =
    api.createConsoleApplicationLogDeletionJob.mock.calls[0][1].scope;
  expect(new Date(scope.started_at_from).getDate()).toBe(7);
  expect(new Date(scope.started_at_from).getHours()).toBe(0);
  expect(new Date(scope.started_at_to).getDate()).toBe(9);
  expect(new Date(scope.started_at_to).getHours()).toBe(0);
});

test('zero or failed count prevents destructive start', async () => {
  api.previewConsoleApplicationLogDeletion.mockResolvedValue({
    total_records: 0
  });
  renderButton();
  expect(await screen.findByText('待删除 0 条日志')).toBeInTheDocument();
  expect(
    within(screen.getByRole('dialog')).getByRole('button', { name: '删除日志' })
  ).toBeDisabled();
  api.previewConsoleApplicationLogDeletion.mockRejectedValue(
    new Error('count transport')
  );
  await choose('全部');
  expect(
    await screen.findByText('统计失败，无法开始删除。')
  ).toBeInTheDocument();
  expect(api.createConsoleApplicationLogDeletionJob).not.toHaveBeenCalled();
});

test('changing range ignores an older delayed preview response', async () => {
  let resolve!: (value: { total_records: number }) => void;
  api.previewConsoleApplicationLogDeletion
    .mockImplementationOnce(
      () =>
        new Promise((done) => {
          resolve = done;
        })
    )
    .mockResolvedValue({ total_records: 3 });
  renderButton();
  await waitFor(() =>
    expect(api.previewConsoleApplicationLogDeletion).toHaveBeenCalled()
  );
  await choose('全部');
  expect(await screen.findByText('待删除 3 条日志')).toBeInTheDocument();
  await act(async () => resolve({ total_records: 999 }));
  expect(screen.getByText('待删除 3 条日志')).toBeInTheDocument();
  expect(screen.queryByText('待删除 999 条日志')).not.toBeInTheDocument();
});

test('restores a running job, polls absolute progress and keeps running when the modal closes', async () => {
  currentJob = job();
  const { onFinished } = renderButton();
  expect(
    await screen.findByText('已删除 100 / 205 条日志')
  ).toBeInTheDocument();
  expect(screen.getByRole('progressbar')).toHaveAttribute(
    'aria-valuenow',
    '48'
  );
  fireEvent.click(
    within(screen.getByRole('dialog')).getByRole('button', { name: /关\s*闭/ })
  );
  currentJob = job({ deleted_records: 150 });
  fireEvent.click(screen.getByRole('button', { name: '删除日志' }));
  expect(
    await screen.findByText('已删除 150 / 205 条日志', {}, { timeout: 4000 })
  ).toBeInTheDocument();
  expect(api.createConsoleApplicationLogDeletionJob).not.toHaveBeenCalled();
  expect(api.stopConsoleApplicationLogDeletionJob).not.toHaveBeenCalled();
  expect(onFinished).not.toHaveBeenCalled();
});

test('query interruption preserves committed progress and reconnects without retrying deletion', async () => {
  currentJob = job();
  renderButton();
  expect(
    await screen.findByText('已删除 100 / 205 条日志')
  ).toBeInTheDocument();
  api.getConsoleApplicationLogDeletionJob.mockRejectedValue(
    new Error('offline')
  );
  expect(
    await screen.findByText(
      '任务查询暂不可用，正在重新查询真实进度。',
      {},
      { timeout: 4000 }
    )
  ).toBeInTheDocument();
  expect(screen.getByText('已删除 100 / 205 条日志')).toBeInTheDocument();
  api.getConsoleApplicationLogDeletionJob.mockResolvedValue({
    job: job({ deleted_records: 150 })
  });
  expect(
    await screen.findByText('已删除 150 / 205 条日志', {}, { timeout: 4000 })
  ).toBeInTheDocument();
  expect(api.createConsoleApplicationLogDeletionJob).not.toHaveBeenCalled();
});

test('lost start response is reconciled by querying its identity without starting another task', async () => {
  api.createConsoleApplicationLogDeletionJob.mockImplementation(
    async (_id, input) => {
      currentJob = job({ job_id: input.job_id, scope: input.scope });
      throw new Error('lost acknowledgment');
    }
  );
  renderButton();
  await start();
  expect(
    await screen.findByText('已删除 100 / 205 条日志', {}, { timeout: 4000 })
  ).toBeInTheDocument();
  expect(api.createConsoleApplicationLogDeletionJob).toHaveBeenCalledOnce();
  expect(api.deleteConsoleApplicationLogs).not.toHaveBeenCalled();
});

test('stop is explicit and terminal partial progress does not show 100 percent', async () => {
  currentJob = job();
  const { client, onFinished } = renderButton();
  fireEvent.click(await screen.findByRole('button', { name: '停止后续批次' }));
  expect(
    await screen.findByRole('button', { name: '正在停止' })
  ).toBeDisabled();
  expect(onFinished).not.toHaveBeenCalled();
  await act(async () =>
    client.setQueryData(
      logDeletionJobKey('app-1', 'fixture-job'),
      job({ status: 'stopped', stop_requested: true })
    )
  );
  expect(
    await screen.findByText('已停止后续批次，已提交的删除保留。')
  ).toBeInTheDocument();
  expect(screen.getByRole('progressbar')).toHaveAttribute(
    'aria-valuenow',
    '48'
  );
  expect(onFinished).toHaveBeenCalledOnce();
});

test('invalid batch size cannot create a job', async () => {
  renderButton();
  await ready();
  fireEvent.change(screen.getByRole('spinbutton', { name: '删除批次' }), {
    target: { value: '1.5' }
  });
  fireEvent.click(
    within(screen.getByRole('dialog')).getByRole('button', { name: '删除日志' })
  );
  expect(
    await screen.findByText('请输入有效的正整数（不超过 4294967295）')
  ).toBeInTheDocument();
  expect(api.createConsoleApplicationLogDeletionJob).not.toHaveBeenCalled();
});

test('initial lookup interruption reconnects before allowing a new deletion', async () => {
  api.getConsoleApplicationLogDeletionJob
    .mockRejectedValueOnce(new Error('lookup interruption'))
    .mockResolvedValue({ job: null });
  renderButton();
  expect(
    await screen.findByText('任务查询暂不可用，正在重新查询真实进度。')
  ).toBeInTheDocument();
  expect(
    within(screen.getByRole('dialog')).getByRole('button', { name: '删除日志' })
  ).toBeDisabled();
  await waitFor(
    () =>
      expect(
        within(screen.getByRole('dialog')).getByRole('button', {
          name: '删除日志'
        })
      ).toBeEnabled(),
    { timeout: 4000 }
  );
  expect(api.createConsoleApplicationLogDeletionJob).not.toHaveBeenCalled();
});

test('a failed server task retains its partial count without guessing completion', async () => {
  currentJob = job();
  const { client, onFinished } = renderButton();
  expect(
    await screen.findByText('已删除 100 / 205 条日志')
  ).toBeInTheDocument();
  await act(async () =>
    client.setQueryData(
      logDeletionJobKey('app-1', 'fixture-job'),
      job({
        status: 'failed',
        deleted_records: 100,
        error_code: 'agent_logs_delete_batch_failed'
      })
    )
  );
  expect(
    await screen.findByText(
      '删除任务失败，已提交的删除保留。请检查服务端任务日志后重新创建删除。'
    )
  ).toBeInTheDocument();
  expect(screen.getByRole('progressbar')).toHaveAttribute(
    'aria-valuenow',
    '48'
  );
  expect(onFinished).toHaveBeenCalledOnce();
});

test('a restored task can finish and start a new deletion without reviving its stale latest snapshot', async () => {
  currentJob = job();
  const { client } = renderButton();
  expect(
    await screen.findByText('已删除 100 / 205 条日志')
  ).toBeInTheDocument();
  currentJob = job({ status: 'succeeded', deleted_records: 205 });
  await act(async () =>
    client.setQueryData(logDeletionJobKey('app-1', 'fixture-job'), currentJob)
  );
  fireEvent.click(await screen.findByRole('button', { name: '新建删除' }));
  expect(
    await screen.findByRole('spinbutton', { name: '删除批次' })
  ).toHaveValue('100');
  await start();
  await waitFor(() =>
    expect(api.createConsoleApplicationLogDeletionJob).toHaveBeenCalledOnce()
  );
  expect(
    api.createConsoleApplicationLogDeletionJob.mock.calls[0][1].job_id
  ).not.toBe('fixture-job');
});
