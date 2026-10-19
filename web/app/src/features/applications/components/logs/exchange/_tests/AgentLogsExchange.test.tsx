import '../../../../../../test/fixtures/rc-util-unique-ids';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import { appI18n } from '../../../../../../shared/i18n/app-i18n';
import { useAuthStore } from '../../../../../../state/auth-store';
import { ApiClientError } from '@1flowbase/api-client';
import { AgentLogsExchange } from '../AgentLogsExchange';
const api = vi.hoisted(() => ({
  exportApplicationLogsOtlp: vi.fn(),
  importApplicationLogsOtlp: vi.fn()
}));
const save = vi.hoisted(() => vi.fn());
vi.mock('../../../../api/log-exchange', () => api);
vi.mock('../../../../lib/run-export-download', () => ({
  saveApplicationRunExport: save
}));
beforeEach(async () => {
  for (const fn of Object.values(api)) fn.mockReset();
  save.mockReset();
  await appI18n.changeLanguage('zh_Hans');
  useAuthStore.setState({ csrfToken: 'fixture-csrf' });
});
function open(
  selectedRecords: Array<{
    id: string;
    application_id: string;
    log_conversation_id: string | null;
  }> = [
    { id: 'r1', application_id: 'app-a', log_conversation_id: 'c1' },
    { id: 'r2', application_id: 'app-b', log_conversation_id: 'c2' }
  ],
  applicationId = ''
) {
  const onImported = vi.fn();
  render(
    <AgentLogsExchange
      applicationId={applicationId}
      applications={[
        { id: 'app-a', name: 'App A' },
        { id: 'app-b', name: 'App B' }
      ]}
      selectedRecords={selectedRecords}
      onImported={onImported}
    />
  );
  fireEvent.click(screen.getByRole('button', { name: '日志交换' }));
  return onImported;
}
async function uploadOtlp() {
  const file = new File(['unused'], 'logs.json', { type: 'application/json' });
  Object.defineProperty(file, 'text', {
    value: async () => JSON.stringify({ resourceSpans: [] })
  });
  fireEvent.change(screen.getByLabelText('OTLP JSON 文件'), {
    target: { files: [file] }
  });
  await waitFor(() =>
    expect(screen.getByRole('button', { name: '执行交换' })).toBeEnabled()
  );
}
function execute() {
  fireEvent.click(
    within(screen.getByRole('dialog')).getByRole('button', {
      name: /执行交换$/
    })
  );
}

test('OTLP export groups the selection by owner and preserves a successful download when another app fails', async () => {
  api.exportApplicationLogsOtlp
    .mockResolvedValueOnce({
      blob: new Blob(['{}']),
      filename: 'server-otlp.json',
      contentType: 'application/json'
    })
    .mockRejectedValueOnce(new Error('connection'));
  open();
  execute();
  expect(
    await screen.findByText('已下载 server-otlp.json')
  ).toBeInTheDocument();
  expect(
    await screen.findByText('交换失败，请检查输入或连接后重试。')
  ).toBeInTheDocument();
  expect(api.exportApplicationLogsOtlp.mock.calls).toEqual([
    ['app-a', { log_conversation_ids: ['c1'] }, 'fixture-csrf'],
    ['app-b', { log_conversation_ids: ['c2'] }, 'fixture-csrf']
  ]);
  expect(save).toHaveBeenCalledOnce();
});

test('pending exchange blocks backdrop and Escape dismissal until the download finishes', async () => {
  let finishDownload!: (value: {
    blob: Blob;
    filename: string;
    contentType: string;
  }) => void;
  api.exportApplicationLogsOtlp.mockReturnValue(
    new Promise((resolve) => {
      finishDownload = resolve;
    })
  );
  open([{ id: 'r1', application_id: 'app-a', log_conversation_id: 'c1' }]);
  execute();

  const dialog = screen.getByRole('dialog');
  const backdrop = dialog.closest<HTMLElement>('.ant-modal-wrap');
  expect(backdrop).not.toBeNull();
  expect(within(dialog).getByRole('button', { name: /^关\s*闭$/ })).toBeDisabled();
  fireEvent.mouseDown(backdrop!);
  fireEvent.mouseUp(backdrop!);
  fireEvent.click(backdrop!);
  fireEvent.keyDown(backdrop!, { key: 'Escape', keyCode: 27 });
  expect(screen.getByRole('dialog')).toBeInTheDocument();
  expect(api.exportApplicationLogsOtlp).toHaveBeenCalledOnce();

  finishDownload({
    blob: new Blob(['{}']),
    filename: 'completed.json',
    contentType: 'application/json'
  });
  await screen.findByText('已下载 completed.json');
  await waitFor(() =>
    expect(within(dialog).getByRole('button', { name: /^关\s*闭$/ })).toBeEnabled()
  );
  fireEvent.mouseDown(backdrop!);
  fireEvent.mouseUp(backdrop!);
  fireEvent.click(backdrop!);
  await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
});

test('OTLP file import uses the object unchanged and shows actual duplicate receipt', async () => {
  const otlp = { resourceSpans: [{ scopeSpans: [] }] };
  api.importApplicationLogsOtlp.mockResolvedValue({
    accepted_events: 0,
    duplicate_events: 7,
    record_ids: ['existing-record']
  });
  open([], 'app-a');
  const file = new File(['unused'], 'logs.json', { type: 'application/json' });
  Object.defineProperty(file, 'text', {
    value: async () => JSON.stringify(otlp)
  });
  fireEvent.change(screen.getByLabelText('OTLP JSON 文件'), {
    target: { files: [file] }
  });
  await waitFor(() =>
    expect(screen.getByRole('button', { name: '执行交换' })).toBeEnabled()
  );
  execute();
  expect(await screen.findByText('existing-record')).toBeInTheDocument();
  expect(api.importApplicationLogsOtlp).toHaveBeenCalledWith(
    'app-a',
    otlp,
    'fixture-csrf'
  );
  expect(within(screen.getByRole('dialog')).getByText('7')).toBeInTheDocument();
});

test('invalid OTLP file and missing CSRF cannot start exchange', async () => {
  useAuthStore.setState({ csrfToken: null });
  open([], 'app-a');
  const file = new File(['unused'], 'invalid.json');
  Object.defineProperty(file, 'text', { value: async () => '[]' });
  fireEvent.change(screen.getByLabelText('OTLP JSON 文件'), {
    target: { files: [file] }
  });
  expect(
    await screen.findByText('请选择有效的 JSON 对象文件。')
  ).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '执行交换' })).toBeDisabled();
  expect(api.importApplicationLogsOtlp).not.toHaveBeenCalled();
});

test('real partial-import transport error shows committed counts, refreshes rows and permits a safe retry', async () => {
  const message =
    'Earlier source batches persisted. Retry unchanged source facts to finish the import safely.';
  const error = await ApiClientError.fromResponse(
    new Response(
      JSON.stringify({
        status: 409,
        code: 'agent_logs_partial_import',
        message,
        inventory: null,
        details: {
          accepted_events: 2,
          duplicate_events: 3,
          committed_records: 1
        }
      }),
      { status: 409, headers: { 'content-type': 'application/json' } }
    )
  );
  api.importApplicationLogsOtlp
    .mockRejectedValueOnce(error)
    .mockResolvedValueOnce({
      accepted_events: 1,
      duplicate_events: 5,
      record_ids: ['committed-record', 'remaining-record']
    });
  const onImported = open([], 'app-a');
  await uploadOtlp();
  execute();
  const dialog = screen.getByRole('dialog');
  expect(
    await within(dialog).findByText('导入未完成，部分来源批次已提交。')
  ).toBeInTheDocument();
  expect(
    within(dialog).getByText(
      '已提交的日志会保留。检查问题后可重试同一导入，已提交事件不会重复创建。'
    )
  ).toBeInTheDocument();
  for (const [label, count] of [
    ['接收事件数', '2'],
    ['重复事件数', '3'],
    ['已提交日志数', '1']
  ]) {
    const row = within(dialog).getByText(label, { exact: true }).closest('tr');
    expect(row).not.toBeNull();
    expect(within(row!).getByText(count, { exact: true })).toBeInTheDocument();
  }
  expect(onImported).toHaveBeenCalledOnce();
  expect(within(dialog).queryByText('服务端交换回执')).not.toBeInTheDocument();
  expect(within(dialog).queryByText('日志记录 ID')).not.toBeInTheDocument();
  const failure = within(dialog).getByRole('alert');
  expect(failure).toHaveClass('ant-alert-error');
  expect(failure).not.toHaveTextContent('public-fixture');
  expect(failure).not.toHaveTextContent('secret-fixture');
  execute();
  expect(
    await within(dialog).findByText('committed-record, remaining-record')
  ).toBeInTheDocument();
  expect(onImported).toHaveBeenCalledTimes(2);
  expect(api.importApplicationLogsOtlp).toHaveBeenCalledTimes(2);
  expect(api.importApplicationLogsOtlp.mock.calls[1]).toEqual(
    api.importApplicationLogsOtlp.mock.calls[0]
  );
});

test('upstream error diagnostics containing credentials remain hidden and do not imply a committed import', async () => {
  const message = 'upstream diagnostic public-fixture secret-fixture';
  const error = await ApiClientError.fromResponse(
    new Response(
      JSON.stringify({
        status: 502,
        code: 'upstream',
        message,
        inventory: null,
        details: null
      }),
      { status: 502, headers: { 'content-type': 'application/json' } }
    )
  );
  api.importApplicationLogsOtlp.mockRejectedValue(error);
  const onImported = open([], 'app-a');
  await uploadOtlp();
  execute();
  expect(
    await screen.findByText('交换失败，请检查输入或连接后重试。')
  ).toBeInTheDocument();
  expect(screen.getByRole('alert')).not.toHaveTextContent('public-fixture');
  expect(screen.getByRole('alert')).not.toHaveTextContent('secret-fixture');
  expect(onImported).not.toHaveBeenCalled();
});

test('missing canonical conversation ID disables export without falling back to the task ID', () => {
  open([{ id: 'r1', application_id: 'app-a', log_conversation_id: null }]);
  expect(
    screen.getByText('部分所选日志缺少会话标识，请刷新列表后再导出。')
  ).toBeInTheDocument();
  expect(
    within(screen.getByRole('dialog')).getByRole('button', {
      name: /执行交换$/
    })
  ).toBeDisabled();
  execute();
  expect(api.exportApplicationLogsOtlp).not.toHaveBeenCalled();
});

test('native source exposes standard download and does not offer import into an executable application', async () => {
  render(
    <AgentLogsExchange
      applicationId="native"
      allowImport={false}
      applications={[{ id: 'native', name: 'Native' }]}
      selectedRecords={[
        {
          id: 'native-task',
          application_id: 'native',
          log_conversation_id: 'native-conversation'
        }
      ]}
      onImported={vi.fn()}
    />
  );
  fireEvent.click(screen.getByRole('button', { name: '日志交换' }));
  fireEvent.mouseDown(screen.getByRole('combobox', { name: '交换方式' }));
  expect(
    await screen.findByText('导出 OTLP JSON', {
      selector: '.ant-select-item-option-content'
    })
  ).toBeInTheDocument();
  expect(
    screen.queryByText('导入 OTLP JSON', {
      selector: '.ant-select-item-option-content'
    })
  ).not.toBeInTheDocument();
});

test('unified export defaults to native, cancels without download and confirms the chosen format', async () => {
  const onExportArchive = vi.fn().mockResolvedValue(undefined);
  api.exportApplicationLogsOtlp.mockResolvedValue({
    blob: new Blob(['{}']),
    filename: 'logs.json',
    contentType: 'application/json'
  });
  render(
    <AgentLogsExchange
      applicationId="app-a"
      allowImport={false}
      applications={[{ id: 'app-a', name: 'App A' }]}
      selectedRecords={[
        { id: 'r1', application_id: 'app-a', log_conversation_id: 'c1' }
      ]}
      onImported={vi.fn()}
      onExportArchive={onExportArchive}
    />
  );
  const trigger = screen.getByRole('button', { name: '导出已选日志' });
  expect(
    screen.queryByRole('button', { name: '日志交换' })
  ).not.toBeInTheDocument();
  fireEvent.click(trigger);
  expect(
    within(screen.getByRole('dialog')).getByText('1flowbase')
  ).toBeInTheDocument();
  expect(onExportArchive).not.toHaveBeenCalled();
  expect(api.exportApplicationLogsOtlp).not.toHaveBeenCalled();
  fireEvent.click(
    within(screen.getByRole('dialog')).getByRole('button', {
      name: /^关\s*闭$/
    })
  );
  expect(onExportArchive).not.toHaveBeenCalled();
  fireEvent.click(trigger);
  fireEvent.click(screen.getByRole('button', { name: '确认下载' }));
  await waitFor(() => expect(onExportArchive).toHaveBeenCalledOnce());
  await waitFor(() =>
    expect(screen.getByRole('button', { name: '确认下载' })).toBeEnabled()
  );
  fireEvent.mouseDown(screen.getByRole('combobox', { name: '导出格式' }));
  fireEvent.click(
    await screen.findByText('OTLP', {
      selector: '.ant-select-item-option-content'
    })
  );
  expect(api.exportApplicationLogsOtlp).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '确认下载' }));
  await waitFor(() =>
    expect(api.exportApplicationLogsOtlp).toHaveBeenCalledWith(
      'app-a',
      { log_conversation_ids: ['c1'] },
      'fixture-csrf'
    )
  );
  await waitFor(() => expect(save).toHaveBeenCalledOnce());
  fireEvent.click(
    within(screen.getByRole('dialog')).getByRole('button', {
      name: /^关\s*闭$/
    })
  );
  fireEvent.click(trigger);
  expect(
    within(screen.getByRole('dialog')).getByText('1flowbase')
  ).toBeInTheDocument();
});

test('native export allows runs without a conversation while OTLP explains why it cannot export them', async () => {
  render(
    <AgentLogsExchange
      applicationId="app-a"
      allowImport={false}
      applications={[]}
      selectedRecords={[
        { id: 'r1', application_id: 'app-a', log_conversation_id: null }
      ]}
      onImported={vi.fn()}
      onExportArchive={vi.fn()}
    />
  );
  fireEvent.click(screen.getByRole('button', { name: '导出已选日志' }));
  expect(screen.getByRole('button', { name: '确认下载' })).toBeEnabled();
  fireEvent.mouseDown(screen.getByRole('combobox', { name: '导出格式' }));
  fireEvent.click(
    await screen.findByText('OTLP', {
      selector: '.ant-select-item-option-content'
    })
  );
  expect(screen.getByRole('button', { name: '确认下载' })).toBeDisabled();
  expect(
    screen.getByText('部分所选日志缺少会话标识，请刷新列表后再导出。')
  ).toBeInTheDocument();
});
