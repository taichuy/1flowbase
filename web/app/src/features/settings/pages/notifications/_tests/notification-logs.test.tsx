import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import type { NotificationSendLog } from '@1flowbase/api-client';
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import { NotificationLogs } from '../NotificationLogs';
import { resetAuthStore, useAuthStore } from '../../../../../state/auth-store';

let client: QueryClient;
let calls: string[];
const log: NotificationSendLog = {
  id: 'log-1',
  workspace_id: 'workspace-1',
  actor_user_id: 'user-1',
  instance_id: 'channel-1',
  display_name: 'Delivery channel',
  type_code: 'system',
  source: 'workflow',
  status: 'failure',
  failure_code: 'input_invalid',
  title: 'Plain title',
  content: '<script>raw text</script>',
  extra: {},
  recipient_count: 2,
  created_at: '2026-10-13T00:00:00Z',
  completed_at: null
};
let detail: NotificationSendLog;
beforeEach(() => {
  detail = { ...log };
  calls = [];
  useAuthStore.setState({
    sessionStatus: 'authenticated',
    csrfToken: 'csrf',
    actor: {
      id: 'user-1',
      current_workspace_id: 'workspace-1',
      effective_display_role: 'root'
    } as NonNullable<ReturnType<typeof useAuthStore.getState>['actor']>
  });
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string) => {
      calls.push(input);
      const url = new URL(input, 'http://localhost');
      const data = url.pathname.endsWith('/types')
        ? []
        : url.pathname.endsWith('/instances')
          ? []
          : url.pathname.endsWith('/logs/log-1')
            ? detail
            : {
                items: [
                  {
                    ...log,
                    id:
                      url.searchParams.get('page') === '2' ? 'log-2' : 'log-1',
                    display_name:
                      url.searchParams.get('page') === '2'
                        ? 'Second page channel'
                        : log.display_name
                  }
                ],
                page: Number(url.searchParams.get('page') ?? 1),
                page_size: 20,
                total: 21
              };
      return new Response(JSON.stringify({ data, meta: null }), {
        headers: { 'content-type': 'application/json' }
      });
    })
  );
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
});
// Await the asynchronous work triggered by each real DOM event.
async function click(...args: Parameters<typeof fireEvent.click>) {
  await act(async () => {
    fireEvent.click(...args);
  });
}
async function mouseDown(...args: Parameters<typeof fireEvent.mouseDown>) {
  await act(async () => {
    fireEvent.mouseDown(...args);
  });
}

async function renderLogs() {
  let view!: ReturnType<typeof render>;
  await act(async () => {
    view = render(
      <QueryClientProvider client={client}>
        <NotificationLogs />
      </QueryClientProvider>
    );
  });
  return view;
}
afterEach(async () => {
  try {
    await waitFor(() => {
      expect(client.isFetching()).toBe(0);
      expect(client.isMutating()).toBe(0);
    });
  } finally {
    cleanup();
    await act(async () => {
      await appI18n.changeLanguage('zh_Hans');
      client.clear();
      resetAuthStore();
    });
    vi.unstubAllGlobals();
  }
});
test('uses backend pages and exposes only safe, plain-text log detail', async () => {
  await renderLogs();
  await screen.findByText('Delivery channel');
  await click(screen.getByRole('button', { name: '详情' }));
  await screen.findByText('Plain title');
  expect(screen.getByRole('textbox', { name: '日志 ID' })).toHaveValue('log-1');
  expect(screen.getByRole('textbox', { name: '日志 ID' })).toHaveAttribute(
    'readonly'
  );
  expect(screen.getByText('channel-1')).toBeInTheDocument();
  expect(
    within(
      document.querySelector('.notification-log-detail') as HTMLElement
    ).getByText('系统通知')
  ).toBeInTheDocument();
  expect(screen.getByText('<script>raw text</script>')).toBeInTheDocument();
  expect(document.querySelector('.notification-log-detail script')).toBeNull();
  expect(screen.getAllByText('消息输入或收件人无效').length).toBeGreaterThan(0);
  expect(screen.queryByText('input_invalid')).not.toBeInTheDocument();
  expect(
    screen.queryByRole('button', { name: /重试|删除/ })
  ).not.toBeInTheDocument();
  await click(screen.getByRole('button', { name: 'Close' }));
  await waitFor(() =>
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  );
  await click(screen.getByTitle('2'));
  await screen.findByText('Second page channel');
  expect(
    calls.some(
      (url) => new URL(url, 'http://localhost').searchParams.get('page') === '2'
    )
  ).toBe(true);
});

test('renders the complete backend body and title without guessing provider fields', async () => {
  const storedTitle = '后端保存的标题（与正文首行不同）';
  const content = `${'完整正文🙂'.repeat(180)}\n最后一行：<script>window.notificationLogExecuted = true</script>`;
  detail = {
    ...log,
    type_code: 'plugin-message',
    title: storedTitle,
    content,
    ...{
      text: 'UNMAPPED_PROVIDER_TEXT',
      provider_error: 'INTERNAL_EXCEPTION_WITH_SECRET_TOKEN'
    }
  };
  await renderLogs();
  await click(await screen.findByRole('button', { name: '详情' }));
  await screen.findByText(storedTitle);
  const drawer = within(screen.getByRole('dialog'));
  const title = drawer.getByRole('group', { name: '消息标题' });
  const body = drawer.getByRole('group', { name: '消息内容' });
  expect(within(title).getByText(storedTitle)).toBeInTheDocument();
  expect(body).toHaveTextContent(content, { normalizeWhitespace: false });
  expect(body.querySelector('script')).toBeNull();
  expect(drawer.queryByText('UNMAPPED_PROVIDER_TEXT')).not.toBeInTheDocument();
  expect(
    drawer.queryByText('INTERNAL_EXCEPTION_WITH_SECRET_TOKEN')
  ).not.toBeInTheDocument();
});

test.each([
  ['zh_Hans', '详情', '附属信息', '是', '否'],
  ['en_US', 'Details', 'Additional information', 'Yes', 'No']
])(
  'renders structured extra values and translated labels in %s',
  async (locale, action, extraLabel, trueLabel, falseLabel) => {
    await act(async () => {
      await appI18n.changeLanguage(locale);
    });
    detail = {
      ...log,
      extra: {
        delivery_id: 'receipt-42',
        accepted: true,
        retry_required: false,
        attempts: 0,
        recipients: ['contact-1', { name: 'Nested recipient' }],
        context: { provider: { receipt: 'nested-receipt-77' } },
        message_fragment: '<img src=x onerror="alert(1)">',
        empty_array: [],
        empty_object: {},
        unavailable: null
      }
    };
    await renderLogs();
    await click(await screen.findByRole('button', { name: action }));
    const extra = within(
      await screen.findByRole('group', { name: extraLabel })
    );
    expect(extra.getByText('delivery_id').tagName).toBe('DT');
    expect(extra.getByText('receipt-42')).toBeInTheDocument();
    expect(extra.getByText(trueLabel)).toBeInTheDocument();
    expect(extra.getByText(falseLabel)).toBeInTheDocument();
    expect(extra.getByText('0')).toBeInTheDocument();
    const recipients = within(extra.getByRole('list'));
    expect(recipients.getAllByRole('listitem')).toHaveLength(2);
    expect(recipients.getByText('contact-1')).toBeInTheDocument();
    expect(recipients.getByText('Nested recipient')).toBeInTheDocument();
    expect(extra.getByText('provider').tagName).toBe('DT');
    expect(extra.getByText('receipt').tagName).toBe('DT');
    expect(extra.getByText('nested-receipt-77')).toBeInTheDocument();
    expect(
      extra.getByText('<img src=x onerror="alert(1)">')
    ).toBeInTheDocument();
    expect(extra.queryByRole('img')).not.toBeInTheDocument();
    expect(extra.getAllByText('—')).toHaveLength(3);
    expect(extra.queryByText('[object Object]')).not.toBeInTheDocument();
    expect(
      extra.queryByText(JSON.stringify(detail.extra))
    ).not.toBeInTheDocument();
  }
);

test.each([null, 'A stored body does not supply a missing title'])(
  'keeps legacy empty titles and extra without inferring from content %s',
  async (content) => {
    detail = {
      ...log,
      title: null,
      content,
      extra: {},
      ...{ text: 'UNMAPPED_LEGACY_TEXT' }
    };
    await renderLogs();
    await click(await screen.findByRole('button', { name: '详情' }));
    const title = await screen.findByRole('group', { name: '消息标题' });
    expect(within(title).getByText('—')).toBeInTheDocument();
    const body = within(screen.getByRole('group', { name: '消息内容' }));
    expect(body.getByText(content ?? '—')).toBeInTheDocument();
    expect(screen.queryByText('附属信息')).not.toBeInTheDocument();
    expect(screen.queryByText('UNMAPPED_LEGACY_TEXT')).not.toBeInTheDocument();
  }
);

test('sends source filter to the server and resets the page', async () => {
  await renderLogs();
  await screen.findByText('Delivery channel');
  await mouseDown(screen.getByRole('combobox', { name: '触发来源' }));
  await click(await screen.findByText('API 调用'));
  await waitFor(() =>
    expect(
      calls.some((input) => {
        const params = new URL(input, 'http://localhost').searchParams;
        return params.get('source') === 'api' && params.get('page') === '1';
      })
    ).toBe(true)
  );
});

test('hides failure reasons for successful delivery and retains a safe reason for an unknown result', async () => {
  await renderLogs();
  await screen.findByText('Delivery channel');
  detail = { ...log, status: 'success', failure_code: 'input_invalid' };
  await click(screen.getByRole('button', { name: '详情' }));
  await screen.findByText('Plain title');
  const success = within(screen.getByRole('dialog'));
  expect(success.getByText('发送成功')).toBeInTheDocument();
  expect(success.queryByText('失败原因')).not.toBeInTheDocument();
  expect(success.queryByText('消息输入或收件人无效')).not.toBeInTheDocument();
  await click(screen.getByRole('button', { name: 'Close' }));
  await waitFor(() =>
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  );
  detail = {
    ...log,
    status: 'unknown',
    failure_code: 'provider_result_unknown'
  };
  await act(async () => {
    await client.invalidateQueries({ queryKey: ['notifications'] });
  });
  await click(screen.getByRole('button', { name: '详情' }));
  await screen.findByText('供应商发送结果未确认');
  const unknown = within(screen.getByRole('dialog'));
  expect(unknown.getByText('结果待确认')).toBeInTheDocument();
  expect(unknown.getByText('失败原因')).toBeInTheDocument();
  expect(
    unknown.queryByText('provider_result_unknown')
  ).not.toBeInTheDocument();
});

test.each([
  [
    'authorization_required',
    '渠道尚未授权，请先完成扫码授权',
    'This channel needs authorization before sending'
  ],
  [
    'authorization_invalid',
    '渠道授权已失效，请重新扫码授权',
    'Channel authorization is invalid. Authorize again'
  ],
  [
    'recipient_not_ready',
    '收件人会话尚未就绪，请向机器人发送一条消息后重试',
    'The recipient conversation is not ready. Send a message to the bot and try again'
  ],
  [
    'subscription_required',
    '尚未收到可验证的用户消息，请先向机器人发送任意消息以开启通知订阅窗口',
    'No verifiable user message received. Send any message to the bot to open the notification subscription window'
  ],
  [
    'subscription_expired',
    '24小时通知订阅窗口已过，请向机器人发送任意消息续期后重试',
    'The 24-hour notification subscription window has expired. Send any message to the bot to renew it and try again'
  ],
  [
    'subscription_unavailable',
    '无法确认通知订阅状态，请稍后重试',
    'The notification subscription status cannot be confirmed. Try again later'
  ],
  [
    'provider_rejected',
    '供应商拒绝了本次消息发送',
    'The provider rejected the message'
  ],
  [
    'provider_result_unknown',
    '供应商发送结果未确认',
    'The provider delivery result is unconfirmed'
  ]
])(
  'renders safe translated reason for %s without provider diagnostics',
  async (failure_code, label, english) => {
    await renderLogs();
    await screen.findByText('Delivery channel');
    detail = {
      ...log,
      failure_code,
      status:
        failure_code === 'provider_result_unknown' ? 'unknown' : 'failure',
      // An unexpected extra field must never become a default UI diagnostic.
      ...{ provider_error: 'SECRET_TOKEN raw provider exception' }
    };
    await click(screen.getByRole('button', { name: '详情' }));
    await screen.findByText(label);
    const drawer = within(screen.getByRole('dialog'));
    expect(drawer.getByText(label)).toBeInTheDocument();
    expect(drawer.queryByText(failure_code)).not.toBeInTheDocument();
    expect(
      drawer.queryByText(/SECRET_TOKEN|raw provider exception/)
    ).not.toBeInTheDocument();
    await act(async () => {
      await appI18n.changeLanguage('en_US');
    });
    expect(drawer.getByText(english)).toBeInTheDocument();
  }
);
