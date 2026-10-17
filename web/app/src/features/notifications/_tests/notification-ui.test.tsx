import type { ReactNode } from 'react';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import type {
  NotificationInstance,
  NotificationType
} from '@1flowbase/api-client';
import { appI18n } from '../../../shared/i18n/app-i18n';
import { useAuthStore, resetAuthStore } from '../../../state/auth-store';
import { NotificationsPage } from '../../settings/pages/notifications/NotificationsPage';
import { NotificationBell } from '../components/NotificationBell';

class SignalStream {
  static instances: SignalStream[] = [];
  onopen: (() => void) | null = null;
  onerror: (() => void) | null = null;
  refresh: (() => void) | null = null;
  close = vi.fn();
  constructor(
    readonly url: string,
    readonly options: EventSourceInit
  ) {
    SignalStream.instances.push(this);
  }
  addEventListener(_name: string, refresh: () => void) {
    this.refresh = refresh;
  }
}
const type: NotificationType = {
  type_code: 'system',
  display_name: '系统通知',
  description: null,
  config_schema: {},
  config_form: {
    schema_version: '1.0.0',
    fields: [
      {
        key: 'token',
        label: '访问令牌',
        type: 'string',
        control: 'password',
        required: true,
        options: [],
        visible_when: [],
        disabled_when: []
      }
    ]
  },
  input_schema: {},
  input_form: { schema_version: '1.0.0', fields: [] },
  output_schema: {},
  contribution_id: 'system',
  installation_id: null,
  available: true,
  unavailable_reason: null
};
const initialInstance: NotificationInstance = {
  id: 'instance-1',
  workspace_id: 'workspace-1',
  type_code: 'system',
  installation_id: null,
  display_name: '系统站内通知',
  status: 'enabled',
  authorization: null,
  config_json: {},
  configured_secret_fields: ['token'],
  available: true,
  unavailable_reason: null,
  created_at: '2026-10-13T00:00:00Z',
  updated_at: '2026-10-13T00:00:00Z'
};

test('switches plugin metadata while an edit drawer is open and preserves user names, draft values and canonical writes', async () => {
  const plugin: NotificationType = {
    ...type,
    type_code: 'vendor/notice/private',
    installation_id: 'plugin-installation',
    display_name: 'Raw plugin name',
    config_schema: {
      type: 'object',
      properties: { endpoint: { type: 'string' } }
    },
    config_form: {
      schema_version: '1.0.0',
      fields: [
        {
          key: 'endpoint',
          label: 'Raw endpoint',
          type: 'string',
          options: [],
          visible_when: [],
          disabled_when: []
        }
      ]
    },
    authorization: {
      actions: [{ code: 'qr', kind: 'qr_code', label: 'Raw scan' }],
      managed_secret_keys: ['bot_token']
    },
    i18n: {
      zh_Hans: {
        display_name: '插件通知',
        config_form: { fields: { endpoint: { label: '插件服务地址' } } },
        authorization: { actions: { qr: { label: '插件扫码' } } }
      },
      en_US: {
        display_name: 'Plugin notification',
        config_form: { fields: { endpoint: { label: 'Plugin endpoint' } } },
        authorization: { actions: { qr: { label: 'Plugin scan' } } }
      }
    }
  };
  catalog = [type, plugin];
  instance = {
    ...initialInstance,
    type_code: plugin.type_code,
    installation_id: plugin.installation_id,
    display_name: 'User supplied 渠道',
    config_json: { endpoint: 'https://old.example' },
    configured_secret_fields: [],
    authorization: { status: 'authorized' }
  };
  renderUI(<NotificationsPage />);
  fireEvent.click(
    await screen.findByRole('button', { name: 'User supplied 渠道' })
  );
  expect(screen.getByRole('textbox', { name: '通知类型' })).toHaveValue(
    '插件通知'
  );
  fireEvent.change(screen.getByRole('textbox', { name: '插件服务地址' }), {
    target: { value: 'https://draft.example' }
  });
  fireEvent.change(screen.getByRole('textbox', { name: '渠道名称' }), {
    target: { value: 'User supplied 新名称' }
  });
  const reads = calls.filter((call) => call.url.endsWith('/types')).length;
  await act(async () => {
    await appI18n.changeLanguage('en_US');
  });
  expect(
    screen.getByRole('textbox', { name: 'Notification type' })
  ).toHaveValue('Plugin notification');
  expect(screen.getByRole('textbox', { name: 'Plugin endpoint' })).toHaveValue(
    'https://draft.example'
  );
  expect(screen.getByRole('textbox', { name: 'Channel name' })).toHaveValue(
    'User supplied 新名称'
  );
  expect(
    screen.getByRole('button', { name: /Plugin scan/ })
  ).toBeInTheDocument();
  expect(calls.filter((call) => call.url.endsWith('/types'))).toHaveLength(
    reads
  );
  expect(plugin.display_name).toBe('Raw plugin name');
  fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));
  await waitFor(() =>
    expect(calls.find((call) => call.method === 'PATCH')?.body).toEqual({
      display_name: 'User supplied 新名称',
      config_json: { endpoint: 'https://draft.example' }
    })
  );
});
let instance: NotificationInstance;
let catalog: NotificationType[];
let filteredInstances: NotificationInstance[] | undefined;
let read_at: string | null;
let rejectRead: boolean;
let rejectStatus: boolean;
let secondReadAt: string | null;
let serverPagination: boolean;
let rejectSnapshots: boolean;
let statusRequest: Promise<void> | undefined;
let calls: Array<{
  url: string;
  method: string;
  body?: Record<string, unknown>;
}>;
let client: QueryClient;
function renderUI(ui: ReactNode) {
  client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } }
  });
  return render(
    <QueryClientProvider client={client}>{ui}</QueryClientProvider>
  );
}
beforeEach(() => {
  instance = { ...initialInstance };
  catalog = [type];
  filteredInstances = undefined;
  read_at = null;
  rejectRead = false;
  rejectStatus = false;
  secondReadAt = null;
  serverPagination = false;
  rejectSnapshots = false;
  statusRequest = undefined;
  calls = [];
  SignalStream.instances = [];
  useAuthStore.setState({
    sessionStatus: 'authenticated',
    csrfToken: 'csrf',
    actor: {
      id: 'user-1',
      current_workspace_id: 'workspace-1',
      effective_display_role: 'root'
    } as NonNullable<ReturnType<typeof useAuthStore.getState>['actor']>
  });
  vi.stubGlobal('EventSource', SignalStream);
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init?: RequestInit) => {
      const method = init?.method ?? 'GET';
      const body = init?.body
        ? (JSON.parse(String(init.body)) as Record<string, unknown>)
        : undefined;
      calls.push({ url, method, body });
      if (
        rejectSnapshots &&
        (url.includes('/channels?') || url.includes('/messages?'))
      )
        return new Response(
          JSON.stringify({
            error: { code: 'unavailable', message: 'Unavailable' }
          }),
          { status: 503, headers: { 'content-type': 'application/json' } }
        );
      const params = new URL(url, 'http://localhost').searchParams;
      const second = params.get('instance_id') === 'instance-2';
      const message = {
        id: second ? 'message-2' : 'message-1',
        workspace_id: 'workspace-1',
        instance_id: second ? 'instance-2' : instance.id,
        recipient_user_id: 'user-1',
        title: second
          ? '另一渠道消息'
          : params.get('cursor')
            ? '第二页消息'
            : '发布完成',
        content: '流程运行成功',
        read_at: second ? secondReadAt : read_at,
        created_at: instance.created_at
      };
      let data: unknown;
      if (url.endsWith('/types')) data = catalog;
      else if (url.endsWith('/instances') && method === 'POST') {
        instance = {
          ...instance,
          ...body,
          configured_secret_fields: []
        } as NotificationInstance;
        data = instance;
      } else if (url.includes('/notifications/instances') && method === 'GET')
        data = url.includes('?')
          ? (filteredInstances ?? [instance])
          : [instance];
      else if (url.includes('/instances/') && method === 'PATCH') {
        if (body?.status) {
          await statusRequest;
          if (rejectStatus)
            return new Response(
              JSON.stringify({
                error: { code: 'forbidden', message: 'Forbidden' }
              }),
              { status: 403, headers: { 'content-type': 'application/json' } }
            );
        }
        instance = { ...instance, ...body } as NotificationInstance;
        data = instance;
      } else if (url.endsWith('/unread-count'))
        data = { unread_count: read_at ? 0 : 1 };
      else if (url.includes('/channels?')) {
        const items = [
          {
            instance_id: instance.id,
            display_name: instance.display_name,
            type_code: 'system',
            deleted: false,
            latest_title: '最新消息摘要',
            latest_content: '渠道一内容',
            latest_created_at: instance.created_at,
            unread_count: read_at ? 0 : 1
          },
          {
            instance_id: 'instance-2',
            display_name: '另一渠道',
            type_code: 'system',
            deleted: true,
            latest_title: '另一消息摘要',
            latest_content: '渠道二内容',
            latest_created_at: instance.created_at,
            unread_count: secondReadAt ? 0 : 1
          }
        ].filter((item) =>
          params.get('read_status') === 'unread'
            ? item.unread_count > 0
            : params.get('read_status') === 'read'
              ? item.unread_count === 0
              : true
        );
        data = {
          items: serverPagination
            ? items.slice(
                params.get('cursor') ? 1 : 0,
                params.get('cursor') ? 2 : 1
              )
            : items,
          next_cursor:
            serverPagination && !params.get('cursor') ? 'channel-next' : null
        };
      } else if (url.endsWith('/unread')) {
        if (rejectRead)
          return new Response(
            JSON.stringify({
              error: { code: 'forbidden', message: 'Forbidden' }
            }),
            { status: 403, headers: { 'content-type': 'application/json' } }
          );
        read_at = null;
        data = { ...message, read_at };
      } else if (url.endsWith('/read') || url.endsWith('/read-all')) {
        if (rejectRead)
          return new Response(
            JSON.stringify({
              error: { code: 'forbidden', message: 'Forbidden' }
            }),
            { status: 403, headers: { 'content-type': 'application/json' } }
          );
        if (url.includes('/channels/instance-2/'))
          secondReadAt = '2026-10-13T01:00:00Z';
        else read_at = '2026-10-13T01:00:00Z';
        data = url.endsWith('/read-all')
          ? { updated_count: 1 }
          : { ...message, read_at };
      } else if (url.includes('/messages?'))
        data = {
          items:
            (params.get('read_status') === 'read' && !message.read_at) ||
            (params.get('read_status') === 'unread' && message.read_at)
              ? []
              : [message],
          next_cursor:
            serverPagination && !params.get('cursor') ? 'message-next' : null,
          unread_count: message.read_at ? 0 : 1
        };
      else throw new Error(`Unexpected route ${url}`);
      return new Response(JSON.stringify({ data, meta: null }), {
        headers: { 'content-type': 'application/json' }
      });
    })
  );
});
afterEach(async () => {
  await appI18n.changeLanguage('zh_Hans');
  client?.clear();
  resetAuthStore();
  vi.unstubAllGlobals();
});

describe('notification management and inbox AC-001/003/004/006', () => {
  test('creates only from available canonical notification types', async () => {
    catalog = [
      { ...type, config_form: { schema_version: '1.0.0', fields: [] } },
      {
        ...type,
        type_code: 'external',
        display_name: 'Unavailable external',
        available: false
      }
    ];
    renderUI(<NotificationsPage />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /新增/ })).toBeEnabled()
    );
    fireEvent.click(await screen.findByRole('button', { name: /新增/ }));
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '提交' })).toBeDisabled();
    fireEvent.mouseDown(screen.getByRole('combobox', { name: '通知类型' }));
    expect(screen.queryByText('Unavailable external')).not.toBeInTheDocument();
    fireEvent.click(
      screen.getByText('系统通知', {
        selector: '.ant-select-item-option-content'
      })
    );
    fireEvent.change(screen.getByRole('textbox', { name: '渠道名称' }), {
      target: { value: 'New notifications' }
    });
    fireEvent.click(screen.getByRole('button', { name: '提交' }));
    await screen.findByRole('button', { name: 'New notifications' });
    expect(calls.find((call) => call.method === 'POST')?.body).toEqual({
      type_code: 'system',
      installation_id: null,
      display_name: 'New notifications',
      config_json: {}
    });
  });
  test('keeps the selected installation identity when two installations contribute the same type', async () => {
    catalog = [
      {
        ...type,
        type_code: 'acme/mail',
        installation_id: 'installation-a',
        display_name: 'Mail A',
        config_form: { schema_version: '1.0.0', fields: [] }
      },
      {
        ...type,
        type_code: 'acme/mail',
        installation_id: 'installation-b',
        display_name: 'Mail B',
        config_form: { schema_version: '1.0.0', fields: [] }
      }
    ];
    renderUI(<NotificationsPage />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /新增/ })).toBeEnabled()
    );
    fireEvent.click(await screen.findByRole('button', { name: /新增/ }));
    fireEvent.mouseDown(screen.getByRole('combobox', { name: '通知类型' }));
    fireEvent.click(
      screen.getByText('Mail B', {
        selector: '.ant-select-item-option-content'
      })
    );
    fireEvent.change(screen.getByRole('textbox', { name: '渠道名称' }), {
      target: { value: 'Team mail' }
    });
    fireEvent.click(screen.getByRole('button', { name: '提交' }));
    await screen.findByRole('button', { name: 'Team mail' });
    expect(calls.find((call) => call.method === 'POST')?.body).toMatchObject({
      type_code: 'acme/mail',
      installation_id: 'installation-b'
    });
  });
  test('switching draft types retains the name but clears all prior schema values', async () => {
    catalog = [
      {
        ...type,
        type_code: 'vendor/first',
        installation_id: 'first-installation',
        display_name: 'First type'
      },
      {
        ...type,
        type_code: 'vendor/other',
        installation_id: 'other-installation',
        display_name: 'Second type',
        config_form: {
          schema_version: '1.0.0',
          fields: [{ ...type.config_form.fields[0], label: 'Second token' }]
        }
      }
    ];
    renderUI(<NotificationsPage />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /新增/ })).toBeEnabled()
    );
    fireEvent.click(screen.getByRole('button', { name: /新增/ }));
    fireEvent.change(screen.getByRole('textbox', { name: '渠道名称' }), {
      target: { value: 'Shared channel name' }
    });
    const choose = (label: string) => {
      fireEvent.mouseDown(screen.getByRole('combobox', { name: '通知类型' }));
      fireEvent.click(
        screen.getByText(label, { selector: '.ant-select-item-option-content' })
      );
    };
    choose('First type');
    fireEvent.change(screen.getByLabelText('访问令牌'), {
      target: { value: 'must-not-leak' }
    });
    choose('Second type');
    expect(screen.getByRole('textbox', { name: '渠道名称' })).toHaveValue(
      'Shared channel name'
    );
    expect(screen.queryByLabelText('访问令牌')).not.toBeInTheDocument();
    expect(screen.getByLabelText('Second token')).toHaveValue('');
    fireEvent.change(screen.getByLabelText('Second token'), {
      target: { value: 'discard-this-too' }
    });
    choose('First type');
    expect(screen.getByLabelText('访问令牌')).toHaveValue('');
    choose('Second type');
    fireEvent.change(screen.getByLabelText('Second token'), {
      target: { value: 'second-only' }
    });
    fireEvent.click(screen.getByRole('button', { name: '提交' }));
    await screen.findByRole('button', { name: 'Shared channel name' });
    expect(calls.find((call) => call.method === 'POST')?.body).toEqual({
      type_code: 'vendor/other',
      installation_id: 'other-installation',
      display_name: 'Shared channel name',
      config_json: { token: 'second-only' }
    });
  });
  test('applies canonical query conditions and renders backend results, then clears the conditions', async () => {
    renderUI(<NotificationsPage />);
    await screen.findByRole('button', { name: '系统站内通知' });
    fireEvent.click(screen.getByRole('button', { name: /筛选/ }));
    fireEvent.change(await screen.findByRole('textbox', { name: '渠道名称' }), {
      target: { value: 'does not exist' }
    });
    filteredInstances = [];
    fireEvent.click(screen.getByRole('button', { name: /^应\s*用$/u }));
    await screen.findByText('暂无通知渠道');
    const filtered = calls.find((call) => call.url.includes('display_name='));
    expect(
      new URL(filtered!.url, 'http://localhost').searchParams.get(
        'display_name'
      )
    ).toBe('does not exist');
    fireEvent.click(screen.getByRole('button', { name: /筛选/ }));
    fireEvent.click(await screen.findByRole('button', { name: /^清\s*除$/u }));
    await screen.findByRole('button', { name: '系统站内通知' });
    expect(screen.getByRole('button', { name: /筛选/ })).toHaveTextContent(
      '筛选'
    );
    expect(screen.getByRole('button', { name: /筛选/ })).not.toHaveTextContent(
      '(1)'
    );
  });
  test('edits the real settings drawer, preserves omitted secrets, and reloads saved names', async () => {
    renderUI(<NotificationsPage />);
    fireEvent.click(await screen.findByRole('button', { name: /^编\s*辑$/u }));
    const name = await screen.findByRole('textbox', { name: '渠道名称' });
    fireEvent.change(name, { target: { value: '团队通知' } });
    const token = document.querySelector('input[type="password"]');
    expect(token).toBeInstanceOf(HTMLInputElement);
    expect(token).toHaveValue('');
    fireEvent.click(screen.getByRole('button', { name: '保存修改' }));
    await screen.findByRole('button', { name: '团队通知' });
    const patch = calls.find((call) => call.method === 'PATCH');
    expect(patch?.body).toEqual({ display_name: '团队通知', config_json: {} });
    fireEvent.click(screen.getByRole('button', { name: /^编\s*辑$/u }));
    expect(
      await screen.findByRole('textbox', { name: '渠道名称' })
    ).toHaveValue('团队通知');
  });
  test('disables the instance using its backend status and shows plugin unavailability without dropping it', async () => {
    instance.available = false;
    instance.unavailable_reason = 'plugin_unavailable';
    renderUI(<NotificationsPage />);
    await screen.findByText('通知插件不可用');
    const toggle = screen.getByRole('switch', { name: '系统站内通知: 状态' });
    expect(toggle).toBeChecked();
    expect(
      screen.queryByRole('button', { name: /^停\s*用$/u })
    ).not.toBeInTheDocument();
    fireEvent.click(toggle);
    await waitFor(() => expect(toggle).not.toBeChecked());
    expect(calls.find((call) => call.method === 'PATCH')?.body).toEqual({
      status: 'disabled'
    });
    expect(
      screen.getByRole('button', { name: '系统站内通知' })
    ).toBeInTheDocument();
    await waitFor(() => expect(toggle).toBeEnabled());
    fireEvent.click(toggle);
    await waitFor(() => expect(toggle).toBeChecked());
    expect(
      calls.filter((call) => call.method === 'PATCH').at(-1)?.body
    ).toEqual({ status: 'enabled' });
  });
  test('keeps the backend switch state while saving and after a rejected status update', async () => {
    rejectStatus = true;
    let finishRequest!: () => void;
    statusRequest = new Promise<void>((resolve) => {
      finishRequest = resolve;
    });
    renderUI(<NotificationsPage />);
    const toggle = await screen.findByRole('switch', {
      name: '系统站内通知: 状态'
    });
    fireEvent.click(toggle);
    await waitFor(() => expect(toggle).toBeDisabled());
    expect(toggle).toBeChecked();
    expect(instance.status).toBe('enabled');
    finishRequest();
    await screen.findByText('通知实例操作失败，请刷新后重试。');
    expect(toggle).toBeChecked();
    expect(toggle).toBeEnabled();
    expect(instance.status).toBe('enabled');
  });
  test('marks messages read through HTTP and refetches after both a signal and reconnect', async () => {
    const view = renderUI(<NotificationBell />);
    fireEvent.click(screen.getByRole('menuitem', { name: '通知消息' }));
    fireEvent.click(
      await screen.findByRole('button', { name: /系统站内通知.*最新消息摘要/ })
    );
    await screen.findByText('发布完成');
    fireEvent.click(screen.getByRole('button', { name: '标为已读' }));
    await screen.findByText('已读', {
      exact: true,
      selector: '.ant-badge-status-text'
    });
    const source = SignalStream.instances[0];
    expect(source.options).toEqual({ withCredentials: true });
    const before = calls.filter((call) =>
      call.url.includes('/messages?')
    ).length;
    act(() => {
      source.refresh?.();
    });
    await waitFor(() =>
      expect(
        calls.filter((call) => call.url.includes('/messages?')).length
      ).toBeGreaterThan(before)
    );
    const after = calls.filter((call) =>
      call.url.includes('/messages?')
    ).length;
    act(() => {
      source.onerror?.();
      source.onopen?.();
    });
    await waitFor(() =>
      expect(
        calls.filter((call) => call.url.includes('/messages?')).length
      ).toBeGreaterThan(after)
    );
    expect(
      screen.getByText('已读', {
        exact: true,
        selector: '.ant-badge-status-text'
      })
    ).toBeInTheDocument();
    view.unmount();
    expect(source.close).toHaveBeenCalledOnce();
  });
  test('surfaces backend refusal without pretending the recipient read state changed', async () => {
    rejectRead = true;
    renderUI(<NotificationBell />);
    fireEvent.click(screen.getByRole('menuitem', { name: '通知消息' }));
    fireEvent.click(
      await screen.findByRole('button', { name: /系统站内通知.*最新消息摘要/ })
    );
    fireEvent.click(await screen.findByRole('button', { name: '标为已读' }));
    await screen.findByText('消息操作失败，请刷新后重试。');
    expect(
      screen.getByText('未读', {
        exact: true,
        selector: '.ant-badge-status-text'
      })
    ).toBeInTheDocument();
    expect(read_at).toBeNull();
    expect(calls.filter((call) => call.method === 'POST')[0].url).toContain(
      '/messages/message-1/read'
    );
  });
  test('marks all read and closes the previous actor connection when session scope changes', async () => {
    renderUI(<NotificationBell />);
    fireEvent.click(screen.getByRole('menuitem', { name: '通知消息' }));
    fireEvent.click(
      await screen.findByRole('button', { name: /系统站内通知.*最新消息摘要/ })
    );
    await screen.findByText('发布完成');
    fireEvent.click(screen.getByRole('button', { name: '全部已读' }));
    await screen.findByText('已读', {
      exact: true,
      selector: '.ant-badge-status-text'
    });
    const source = SignalStream.instances[0];
    act(() =>
      useAuthStore.setState({
        actor: { ...useAuthStore.getState().actor!, id: 'user-2' }
      })
    );
    await waitFor(() => expect(source.close).toHaveBeenCalledOnce());
    expect(SignalStream.instances).toHaveLength(2);
  });
});

test('selects channels without writes, confines all-read to the selected channel and permits marking unread', async () => {
  renderUI(<NotificationBell />);
  fireEvent.click(screen.getByRole('menuitem', { name: '通知消息' }));
  fireEvent.click(
    await screen.findByRole('button', { name: /另一渠道.*另一消息摘要/ })
  );
  await screen.findByText('另一渠道消息');
  expect(
    screen.getByRole('button', { name: '另一渠道消息' })
  ).toBeInTheDocument();
  expect(calls.filter((call) => call.method === 'POST')).toHaveLength(0);
  fireEvent.click(screen.getByRole('button', { name: '全部已读' }));
  await waitFor(() =>
    expect(calls.find((call) => call.method === 'POST')?.url).toContain(
      '/channels/instance-2/read-all'
    )
  );
  expect(read_at).toBeNull();
  expect(calls.some((call) => call.url.endsWith('/messages/read-all'))).toBe(
    false
  );
  fireEvent.click(
    screen.getByRole('button', { name: /系统站内通知.*最新消息摘要/ })
  );
  fireEvent.click(await screen.findByRole('button', { name: '发布完成' }));
  await screen.findByRole('button', { name: '标为未读' });
  fireEvent.click(screen.getByRole('button', { name: '标为未读' }));
  await screen.findByRole('button', { name: '标为已读' });
  expect(read_at).toBeNull();
  expect(
    calls.some((call) => call.url.endsWith('/messages/message-1/unread'))
  ).toBe(true);
});
test('filters both backend lists and resets both cursor pages', async () => {
  serverPagination = true;
  renderUI(<NotificationBell />);
  fireEvent.click(screen.getByRole('menuitem', { name: '通知消息' }));
  fireEvent.click(
    await screen.findByRole('button', { name: /系统站内通知.*最新消息摘要/ })
  );
  await screen.findByRole('button', { name: '发布完成' });
  fireEvent.click(screen.getAllByRole('button', { name: '下一页' })[1]);
  await screen.findByRole('button', { name: '第二页消息' });
  fireEvent.click(screen.getAllByRole('button', { name: '下一页' })[0]);
  await screen.findByRole('button', { name: /另一渠道.*另一消息摘要/ });
  fireEvent.click(screen.getByRole('tab', { name: '未读' }));
  await waitFor(() => {
    for (const path of ['/channels?', '/messages?']) {
      const call = calls.filter((call) => call.url.includes(path)).at(-1)!;
      const params = new URL(call.url, 'http://localhost').searchParams;
      expect(params.get('read_status')).toBe('unread');
      expect(params.has('cursor')).toBe(false);
    }
  });
  expect(
    await screen.findByRole('button', { name: '发布完成' })
  ).toBeInTheDocument();
});
test('clears the previous workspace selection before requesting scoped snapshots', async () => {
  renderUI(<NotificationBell />);
  fireEvent.click(screen.getByRole('menuitem', { name: '通知消息' }));
  fireEvent.click(
    await screen.findByRole('button', { name: /系统站内通知.*最新消息摘要/ })
  );
  await screen.findByRole('button', { name: '发布完成' });
  const count = calls.length;
  act(() =>
    useAuthStore.setState({
      actor: {
        ...useAuthStore.getState().actor!,
        current_workspace_id: 'workspace-2'
      }
    })
  );
  await screen.findByText('请选择通知渠道');
  await waitFor(() => expect(SignalStream.instances).toHaveLength(2));
  expect(
    calls.slice(count).some((call) => call.url.includes('/messages?'))
  ).toBe(false);
  expect(
    screen.queryByRole('button', { name: '发布完成' })
  ).not.toBeInTheDocument();
});

test('preserves the HTTP snapshot after a failed page fetch and does not falsify failed channel or unread writes', async () => {
  serverPagination = true;
  renderUI(<NotificationBell />);
  fireEvent.click(screen.getByRole('menuitem', { name: '通知消息' }));
  fireEvent.click(
    await screen.findByRole('button', { name: /系统站内通知.*最新消息摘要/ })
  );
  await screen.findByRole('button', { name: '发布完成' });
  rejectRead = true;
  fireEvent.click(screen.getByRole('button', { name: '全部已读' }));
  await screen.findByText('消息操作失败，请刷新后重试。');
  expect(read_at).toBeNull();
  expect(screen.getByRole('button', { name: '发布完成' })).toBeInTheDocument();
  rejectRead = false;
  fireEvent.click(screen.getByRole('button', { name: '发布完成' }));
  await screen.findByRole('button', { name: '标为未读' });
  rejectRead = true;
  fireEvent.click(screen.getByRole('button', { name: '标为未读' }));
  await screen.findByText('消息操作失败，请刷新后重试。');
  expect(read_at).not.toBeNull();
  expect(screen.getByRole('button', { name: '标为未读' })).toBeInTheDocument();
  rejectSnapshots = true;
  fireEvent.click(screen.getAllByRole('button', { name: '下一页' })[1]);
  await waitFor(() =>
    expect(calls.some((call) => call.url.includes('cursor=message-next'))).toBe(
      true
    )
  );
  await waitFor(() =>
    expect(screen.getAllByRole('button', { name: '下一页' })[1]).toBeEnabled()
  );
  expect(screen.getByRole('button', { name: '发布完成' })).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '标为未读' })).toBeInTheDocument();
});
