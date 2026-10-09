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
import { beforeEach, expect, test, vi } from 'vitest';
import { appI18n } from '../../../../shared/i18n/app-i18n';
import { useAuthStore } from '../../../../state/auth-store';
import { AgentLogsDeleteButton } from '../../components/logs/workspace/AgentLogsDeleteButton';
import { deleteApplicationLogsInBatches } from '../../api/log-deletion';
import type { AgentLogsDeleteReceipt } from '@1flowbase/api-client';
const api = vi.hoisted(() => ({ deleteConsoleApplicationLogs: vi.fn() }));
vi.mock('@1flowbase/api-client', async (original) => ({
  ...(await original<typeof import('@1flowbase/api-client')>()),
  ...api
}));
const boundary = '2026-10-09T00:00:00Z';
const receipt = (count: number, more = false): AgentLogsDeleteReceipt => ({
  deleted_records: count,
  has_more: more,
  ingested_at_before: boundary
});
function renderButton() {
  const onFinished = vi.fn().mockResolvedValue(undefined);
  const view = render(
    <App>
      <AgentLogsDeleteButton applicationId="app-1" onFinished={onFinished} />
    </App>
  );
  fireEvent.click(screen.getByRole('button', { name: '删除日志' }));
  return { ...view, onFinished };
}
async function choose(label: string) {
  fireEvent.mouseDown(screen.getByRole('combobox', { name: '选择日期' }));
  fireEvent.click(
    await screen.findByText(label, {
      selector: '.ant-select-item-option-content'
    })
  );
}
beforeEach(async () => {
  vi.clearAllMocks();
  api.deleteConsoleApplicationLogs.mockReset();
  await appI18n.changeLanguage('zh_Hans');
  useAuthStore.setState({ csrfToken: 'fixture-csrf' });
});

test('form offers six date scopes and defaults to batches of 100', async () => {
  const { onFinished } = renderButton();
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
  ]) {
    expect(
      await screen.findByText(label, {
        selector: '.ant-select-item-option-content'
      })
    ).toBeInTheDocument();
  }
  fireEvent.click(
    screen.getByText('全部', { selector: '.ant-select-item-option-content' })
  );
  api.deleteConsoleApplicationLogs
    .mockResolvedValueOnce(receipt(100, true))
    .mockResolvedValueOnce(receipt(3));
  fireEvent.click(
    within(screen.getByRole('dialog')).getByRole('button', { name: '删除日志' })
  );
  expect(await screen.findByText('已确认删除 103 条日志')).toBeInTheDocument();
  expect(screen.getByText('所选范围的日志已删除。')).toBeInTheDocument();
  expect(api.deleteConsoleApplicationLogs.mock.calls[0][1]).toEqual({
    mode: 'all_time',
    batch_size: 100,
    ingested_at_before: undefined
  });
  expect(api.deleteConsoleApplicationLogs.mock.calls[1][1]).toEqual({
    mode: 'all_time',
    batch_size: 100,
    ingested_at_before: boundary
  });
  expect(onFinished).toHaveBeenCalledTimes(1);
});

test.each([
  ['过去 7 天', 7],
  ['过去 30 天', 30],
  ['过去 90 天', 90],
  ['过去一年', 365]
])('preset %s freezes an explicit started_at range', async (label, days) => {
  renderButton();
  await choose(String(label));
  api.deleteConsoleApplicationLogs.mockResolvedValue(receipt(0));
  fireEvent.click(
    within(screen.getByRole('dialog')).getByRole('button', { name: '删除日志' })
  );
  await waitFor(() =>
    expect(api.deleteConsoleApplicationLogs).toHaveBeenCalledOnce()
  );
  const scope = api.deleteConsoleApplicationLogs.mock.calls[0][1];
  expect(scope.mode).toBe('time_range');
  // October fixtures avoid DST; the custom date test separately covers calendar inclusivity.
  expect(
    new Date(scope.started_at_to).getTime() -
      new Date(scope.started_at_from).getTime()
  ).toBe(Number(days) * 86400000);
});

test('custom dates require a range and include the full end day', async () => {
  renderButton();
  await choose('自定义日期');
  fireEvent.click(
    within(screen.getByRole('dialog')).getByRole('button', { name: '删除日志' })
  );
  expect(await screen.findByText('请选择开始和结束日期')).toBeInTheDocument();
  expect(api.deleteConsoleApplicationLogs).not.toHaveBeenCalled();
  const inputs = within(screen.getByRole('dialog')).getAllByRole('textbox');
  fireEvent.change(inputs[0], { target: { value: '2026-10-07' } });
  fireEvent.keyDown(inputs[0], { key: 'Enter', code: 'Enter' });
  fireEvent.change(inputs[1], { target: { value: '2026-10-08' } });
  fireEvent.keyDown(inputs[1], { key: 'Enter', code: 'Enter' });
  api.deleteConsoleApplicationLogs.mockResolvedValue(receipt(1));
  fireEvent.click(
    within(screen.getByRole('dialog')).getByRole('button', { name: '删除日志' })
  );
  await waitFor(() =>
    expect(api.deleteConsoleApplicationLogs).toHaveBeenCalledOnce()
  );
  const scope = api.deleteConsoleApplicationLogs.mock.calls[0][1];
  expect(new Date(scope.started_at_from).getDate()).toBe(7);
  expect(new Date(scope.started_at_from).getHours()).toBe(0);
  expect(new Date(scope.started_at_to).getDate()).toBe(9);
  expect(new Date(scope.started_at_to).getHours()).toBe(0);
});

test('stop waits for the active batch, and sends no later mutation', async () => {
  let resolve!: (value: AgentLogsDeleteReceipt) => void;
  api.deleteConsoleApplicationLogs.mockReturnValue(
    new Promise((done) => {
      resolve = done;
    })
  );
  const { onFinished } = renderButton();
  fireEvent.click(
    within(screen.getByRole('dialog')).getByRole('button', { name: '删除日志' })
  );
  fireEvent.click(await screen.findByRole('button', { name: '停止后续批次' }));
  expect(onFinished).not.toHaveBeenCalled();
  await act(async () => resolve(receipt(100, true)));
  expect(
    await screen.findByText('已停止后续批次，已提交的删除保留。')
  ).toBeInTheDocument();
  expect(api.deleteConsoleApplicationLogs).toHaveBeenCalledOnce();
  expect(onFinished).toHaveBeenCalledOnce();
});

test('failure preserves confirmed partial count and refreshes without automatic retry', async () => {
  api.deleteConsoleApplicationLogs
    .mockResolvedValueOnce(receipt(100, true))
    .mockRejectedValueOnce(new Error('fixture interruption'));
  const { onFinished } = renderButton();
  fireEvent.click(
    within(screen.getByRole('dialog')).getByRole('button', { name: '删除日志' })
  );
  expect(await screen.findByText('已确认删除 100 条日志')).toBeInTheDocument();
  expect(await screen.findByText(/删除中断。/)).toBeInTheDocument();
  expect(api.deleteConsoleApplicationLogs).toHaveBeenCalledTimes(2);
  expect(onFinished).toHaveBeenCalledOnce();
});

test('unmount stops the batch loop after the in-flight receipt', async () => {
  let resolve!: (value: AgentLogsDeleteReceipt) => void;
  api.deleteConsoleApplicationLogs.mockReturnValue(
    new Promise((done) => {
      resolve = done;
    })
  );
  const { unmount, onFinished } = renderButton();
  fireEvent.click(
    within(screen.getByRole('dialog')).getByRole('button', { name: '删除日志' })
  );
  await waitFor(() =>
    expect(api.deleteConsoleApplicationLogs).toHaveBeenCalledOnce()
  );
  unmount();
  await act(async () => resolve(receipt(100, true)));
  expect(api.deleteConsoleApplicationLogs).toHaveBeenCalledOnce();
  expect(onFinished).not.toHaveBeenCalled();
});

test('batch consumer repeats the first boundary even when multiple full batches remain', async () => {
  api.deleteConsoleApplicationLogs
    .mockResolvedValueOnce(receipt(100, true))
    .mockResolvedValueOnce(receipt(100, true))
    .mockResolvedValueOnce(receipt(5));
  const onReceipt = vi.fn();
  await expect(
    deleteApplicationLogsInBatches(
      'app-1',
      { mode: 'all_time', batch_size: 100 },
      'csrf',
      onReceipt,
      () => false
    )
  ).resolves.toBe('complete');
  expect(api.deleteConsoleApplicationLogs).toHaveBeenCalledTimes(3);
  expect(onReceipt.mock.calls.map(([item]) => item.deleted_records)).toEqual([
    100, 100, 5
  ]);
});

test('invalid batch size does not dispatch deletion', async () => {
  renderButton();
  fireEvent.change(screen.getByRole('spinbutton', { name: '删除批次' }), {
    target: { value: '1.5' }
  });
  fireEvent.click(
    within(screen.getByRole('dialog')).getByRole('button', { name: '删除日志' })
  );
  expect(
    await screen.findByText('请输入有效的正整数（不超过 4294967295）')
  ).toBeInTheDocument();
  expect(api.deleteConsoleApplicationLogs).not.toHaveBeenCalled();
});
