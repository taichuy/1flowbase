import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import type {
  NotificationAuthorizationView,
  NotificationDeliveryWindowView,
  NotificationInstance,
  NotificationType
} from '@1flowbase/api-client';
import { NotificationAuthorization } from '../NotificationAuthorization';
import { formatDateTime } from '../../../../../shared/i18n/format';
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import { NotificationInstanceDrawer } from '../NotificationInstanceDrawer';
import { resetAuthStore, useAuthStore } from '../../../../../state/auth-store';

const type: NotificationType = {
  type_code: 'vendor/private-notification',
  display_name: '扫码渠道',
  description: null,
  config_schema: {},
  config_form: {
    schema_version: '1.0.0',
    fields: [
      {
        key: 'endpoint',
        label: '服务地址',
        type: 'string',
        required: false,
        options: [],
        visible_when: [],
        disabled_when: []
      }
    ]
  },
  input_schema: {},
  input_form: { schema_version: '1.0.0', fields: [] },
  output_schema: {},
  authorization: {
    actions: [{ code: 'scan', label: '扫码授权', kind: 'qr_code' }],
    managed_secret_keys: ['bot_token']
  },
  contribution_id: 'notification',
  installation_id: 'installation-1',
  available: true,
  unavailable_reason: null
};
const instance: NotificationInstance = {
  id: 'instance-1',
  workspace_id: 'workspace-1',
  type_code: type.type_code,
  installation_id: type.installation_id,
  display_name: '我的渠道',
  status: 'disabled',
  config_json: { endpoint: 'https://example.com' },
  configured_secret_fields: [],
  authorization: { status: 'unauthorized' },
  available: false,
  unavailable_reason: 'instance_disabled',
  created_at: '2026-10-15T00:00:00Z',
  updated_at: '2026-10-15T00:00:00Z'
};
const pending: NotificationAuthorizationView = {
  status: 'pending',
  user_action: {
    kind: 'qr_code',
    qr_code_url: 'https://example.com/qr',
    poll_interval_seconds: 5
  }
};
let commands: Array<{
  path: string;
  body: Record<string, unknown>;
  signal?: AbortSignal | null;
}>;
let poll: () => Promise<NotificationAuthorizationView>;
let begin: NotificationAuthorizationView;
let maintained: NotificationAuthorizationView;
const saved = vi.fn();
function respond(data: unknown) {
  return new Response(JSON.stringify({ data, meta: null }), {
    headers: { 'content-type': 'application/json' }
  });
}
// Capture the implementation, because spyOn reuses the shared vi.fn in place.
const getContext = vi
  .mocked(HTMLCanvasElement.prototype.getContext)
  .getMockImplementation()!;
beforeEach(() => {
  // Keep Antd's real QRCodeCanvas path; the shared jsdom context omits fillRect.
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(
    function (this: HTMLCanvasElement, contextId: string, options?: unknown) {
      const context = getContext.call(this, contextId, options);
      if (contextId === '2d' && context) {
        Object.assign(context, { fillRect: vi.fn() });
      }
      return context;
    }
  );
  commands = [];
  poll = async () => ({ status: 'authorized' });
  begin = pending;
  maintained = {
    status: 'authorized',
    delivery_window: {
      status: 'active',
      last_received_at_ms: 1_860_000_000_000,
      expires_at_ms: 1_860_086_400_000
    }
  };
  saved.mockReset();
  useAuthStore.setState({ csrfToken: 'csrf' });
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init?: RequestInit) => {
      const body = JSON.parse(String(init?.body ?? '{}')) as Record<
        string,
        unknown
      >;
      const path = new URL(url, 'http://localhost').pathname;
      commands.push({ path, body, signal: init?.signal });
      if (path.endsWith('/authorization')) {
        if (body.type === 'begin') return respond(begin);
        if (body.type === 'poll') return respond(await poll());
        if (body.type === 'maintain') return respond(maintained);
        if (body.type === 'cancel') return respond({ status: 'unauthorized' });
      }
      return respond({
        ...instance,
        ...body,
        id: 'persisted-id',
        authorization: { status: 'unauthorized' }
      });
    })
  );
});
afterEach(async () => {
  cleanup();
  await act(async () => {
    await appI18n.changeLanguage('zh_Hans');
    resetAuthStore();
  });
  vi.restoreAllMocks();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

test('persists a new instance before metadata-driven QR, retains edits and polls to authorized', async () => {
  vi.useFakeTimers();
  await renderSettled(
    <NotificationInstanceDrawer
      types={[type]}
      onClose={vi.fn()}
      onSaved={saved}
    />
  );
  await mouseDown(screen.getByRole('combobox', { name: '渠道类型' }));
  await click(
    screen.getByText('扫码渠道', {
      selector: '.ant-select-item-option-content'
    })
  );
  await change(screen.getByRole('textbox', { name: '渠道名称' }), {
    target: { value: '新渠道' }
  });
  await change(screen.getByRole('textbox', { name: '服务地址' }), {
    target: { value: 'https://example.com/custom' }
  });
  await click(screen.getByRole('button', { name: '保存并扫码授权' }));
  // beginOnMount deliberately schedules authorization at 0 ms after persistence.
  // Advance only that startup task; the 5 s polling interval remains unelapsed.
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
  expect(screen.getByText('请扫码并在设备上确认授权。')).toBeInTheDocument();
  expect(
    document.querySelector('.notification-instance-drawer canvas')
  ).toBeInTheDocument();
  expect(commands[0]?.path).toBe('/api/console/communications/instances');
  expect(commands[0]?.body).toMatchObject({
    type_code: type.type_code,
    installation_id: type.installation_id,
    config_json: { endpoint: 'https://example.com/custom' }
  });
  expect(commands[1]).toMatchObject({
    path: '/api/console/communications/instances/persisted-id/authorization',
    body: { type: 'begin', action: 'scan' }
  });
  expect(saved).toHaveBeenCalledWith({ keepOpen: true });
  expect(
    screen.queryByRole('combobox', { name: '渠道类型' })
  ).not.toBeInTheDocument();
  expect(screen.getByRole('textbox', { name: '渠道类型' })).toHaveValue(
    '扫码渠道'
  );
  expect(screen.getByRole('textbox', { name: '渠道名称' })).toHaveValue(
    '新渠道'
  );
  expect(screen.getByRole('textbox', { name: '服务地址' })).toHaveValue(
    'https://example.com/custom'
  );
  expect(screen.getByText('授权完成不代表消息已送达。')).toBeInTheDocument();
  await act(async () => {
    await vi.advanceTimersByTimeAsync(4999);
  });
  expect(
    commands.filter((command) => command.body.type === 'poll')
  ).toHaveLength(0);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1);
  });
  expect(screen.getByText('已授权')).toBeInTheDocument();
  expect(screen.getByText('授权已完成。')).toBeInTheDocument();
  expect(screen.getByText('通知实例已停用')).toBeInTheDocument();
  expect(commands.some((command) => command.body.status === 'enabled')).toBe(
    false
  );
  await act(async () => {
    await vi.advanceTimersByTimeAsync(15000);
  });
  expect(
    commands.filter((command) => command.body.type === 'poll')
  ).toHaveLength(1);
});

test('explicit cancel aborts an in-flight poll and ignores its late authorized response', async () => {
  let finish!: (view: NotificationAuthorizationView) => void;
  poll = () =>
    new Promise((resolve) => {
      finish = resolve;
    });
  vi.useFakeTimers();
  await renderSettled(
    <NotificationInstanceDrawer
      instance={{ ...instance, authorization: pending }}
      types={[type]}
      onClose={vi.fn()}
      onSaved={saved}
    />
  );
  await act(async () => {
    await vi.advanceTimersByTimeAsync(5000);
  });
  const pollCommand = commands.find((command) => command.body.type === 'poll');
  expect(pollCommand).toMatchObject({
    body: { type: 'poll' },
    signal: { aborted: false }
  });
  await click(screen.getByRole('button', { name: '取消授权' }));
  expect(pollCommand?.signal?.aborted).toBe(true);
  expect(screen.getByText('未授权', { selector: 'span' })).toBeInTheDocument();
  await act(async () => {
    finish({ status: 'authorized' });
  });
  expect(screen.queryByText('已授权')).not.toBeInTheDocument();
});

test('unmount stops polling and aborts the request without cancelling backend authorization', async () => {
  poll = () => new Promise(() => {});
  vi.useFakeTimers();
  const { unmount } = await renderSettled(
    <NotificationInstanceDrawer
      instance={{ ...instance, authorization: pending }}
      types={[type]}
      onClose={vi.fn()}
      onSaved={saved}
    />
  );
  await act(async () => {
    await vi.advanceTimersByTimeAsync(5000);
  });
  const pollCommand = commands.find((command) => command.body.type === 'poll');
  unmount();
  expect(pollCommand?.signal?.aborted).toBe(true);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(15000);
  });
  expect(
    commands.filter((command) => command.body.type === 'poll')
  ).toHaveLength(1);
  expect(commands.some((command) => command.body.type === 'cancel')).toBe(
    false
  );
});

test('invalid authorization offers the declared reauthorization action independently of disabled status', async () => {
  await renderSettled(
    <NotificationInstanceDrawer
      instance={{ ...instance, authorization: { status: 'invalid' } }}
      types={[type]}
      onClose={vi.fn()}
      onSaved={saved}
    />
  );
  expect(
    screen.getByText('授权已失效', { selector: 'span' })
  ).toBeInTheDocument();
  await click(screen.getByRole('button', { name: '重新授权：扫码授权' }));
  await screen.findByText('请扫码并在设备上确认授权。');
  expect(commands[0]?.body).toEqual({ type: 'begin', action: 'scan' });
  expect(commands[0]?.path).toContain('instance-1/authorization');
});

test('legacy null authorization preserves schema editing and normal save-and-close', async () => {
  await renderSettled(
    <NotificationInstanceDrawer
      instance={{ ...instance, authorization: null }}
      types={[{ ...type, authorization: undefined }]}
      onClose={vi.fn()}
      onSaved={saved}
    />
  );
  expect(screen.queryByText(/授权状态/)).not.toBeInTheDocument();
  expect(screen.getByRole('textbox', { name: '渠道类型' })).toHaveValue(
    type.display_name
  );
  await change(screen.getByRole('textbox', { name: '服务地址' }), {
    target: { value: 'https://example.com/edited' }
  });
  await click(screen.getByRole('button', { name: '保存修改' }));
  await waitFor(() => expect(saved).toHaveBeenCalledWith());
  expect(commands[0]?.body).toEqual({
    display_name: '我的渠道',
    config_json: { endpoint: 'https://example.com/edited' }
  });
  expect(
    commands.some((command) => command.path.endsWith('/authorization'))
  ).toBe(false);
});

test('locale refresh preserves the edited notification config and display name through save', async () => {
  await renderSettled(
    <NotificationInstanceDrawer
      instance={{ ...instance, authorization: null }}
      types={[{ ...type, authorization: undefined }]}
      onClose={vi.fn()}
      onSaved={saved}
    />
  );
  await change(screen.getByRole('textbox', { name: '服务地址' }), {
    target: { value: 'https://example.com/locale-draft' }
  });
  await change(screen.getByRole('textbox', { name: '渠道名称' }), {
    target: { value: 'Draft across locales' }
  });
  await act(async () => {
    await appI18n.changeLanguage('en_US');
  });
  expect(screen.getByRole('textbox', { name: 'Channel name' })).toHaveValue(
    'Draft across locales'
  );
  expect(
    screen.getByDisplayValue('https://example.com/locale-draft')
  ).toBeInTheDocument();
  await click(screen.getByRole('button', { name: 'Save changes' }));
  await waitFor(() => expect(saved).toHaveBeenCalledWith());
  expect(commands[0]?.body).toEqual({
    display_name: 'Draft across locales',
    config_json: { endpoint: 'https://example.com/locale-draft' }
  });
});

test('authorization errors remain safe and do not claim a backend status change', async () => {
  vi.stubGlobal(
    'fetch',
    vi.fn(async () => {
      throw new Error('SECRET_TOKEN raw provider exception');
    })
  );
  await renderSettled(
    <NotificationInstanceDrawer
      instance={instance}
      types={[type]}
      onClose={vi.fn()}
      onSaved={saved}
    />
  );
  await click(screen.getByRole('button', { name: '扫码授权' }));
  await screen.findByText('授权操作未完成，请重试。');
  expect(screen.getByText('未授权', { selector: 'span' })).toBeInTheDocument();
  expect(
    screen.queryByText(/SECRET_TOKEN|raw provider exception/)
  ).not.toBeInTheDocument();
  expect(saved).not.toHaveBeenCalled();
});

const windowTimes = {
  last_received_at_ms: 1_760_000_000_000,
  expires_at_ms: 1_760_086_400_000
};
const subscriptionCases: Array<{
  delivery_window: NotificationDeliveryWindowView;
  label: string;
  english: string;
}> = [
  {
    delivery_window: { status: 'unknown' },
    label: '订阅状态待确认',
    english: 'Subscription status unconfirmed'
  },
  {
    delivery_window: { status: 'unknown', ...windowTimes },
    label: '订阅状态待确认',
    english: 'Subscription status unconfirmed'
  },
  {
    delivery_window: { status: 'waiting_for_message' },
    label: '等待用户消息',
    english: 'Waiting for a user message'
  },
  {
    delivery_window: { status: 'active', ...windowTimes },
    label: '订阅有效/可发送',
    english: 'Subscription active / ready to send'
  },
  {
    delivery_window: { status: 'expired', ...windowTimes },
    label: '订阅已过期',
    english: 'Subscription expired'
  }
];

test.each(subscriptionCases)(
  'shows independent backend subscription state $label and its supplied times',
  async ({ delivery_window, label, english }) => {
    await renderSettled(
      <NotificationAuthorization
        instance_id={instance.id}
        descriptor={type.authorization!}
        authorization={{ status: 'authorized', delivery_window }}
        onChanged={saved}
      />
    );
    expect(screen.getByText('已授权')).toBeInTheDocument();
    expect(screen.getByText('通知订阅')).toBeInTheDocument();
    expect(screen.getByText(label)).toBeInTheDocument();
    if (delivery_window.last_received_at_ms !== undefined) {
      expect(screen.getByText('最近消息时间')).toBeInTheDocument();
      expect(screen.getByText('可用期限')).toBeInTheDocument();
      expect(
        screen.getByText(
          formatDateTime(new Date(windowTimes.last_received_at_ms))
        )
      ).toBeInTheDocument();
      expect(
        screen.getByText(formatDateTime(new Date(windowTimes.expires_at_ms)))
      ).toBeInTheDocument();
    } else {
      expect(
        screen.queryByText(/最近消息时间|可用期限/)
      ).not.toBeInTheDocument();
    }
    // The same old timestamps can be active, expired or unknown: only backend status decides.
    expect(
      screen.getByText(
        '请向机器人发送任意消息以开启或续期24小时通知订阅窗口，然后刷新订阅状态。渠道仍需启用后才能发送通知。'
      )
    ).toBeInTheDocument();
    expect(commands).toHaveLength(0);
    await act(async () => {
      await appI18n.changeLanguage('en_US');
    });
    expect(screen.getByText(english)).toBeInTheDocument();
  }
);

test('manual Maintain refresh updates the window without reauthorizing, enabling or polling', async () => {
  vi.useFakeTimers();
  await renderSettled(
    <NotificationAuthorization
      instance_id={instance.id}
      descriptor={type.authorization!}
      authorization={{
        status: 'authorized',
        delivery_window: { status: 'expired', ...windowTimes }
      }}
      onChanged={saved}
    />
  );
  expect(screen.getByText('订阅已过期')).toBeInTheDocument();
  await click(screen.getByRole('button', { name: /刷新状态/ }));
  expect(screen.getByText('订阅有效/可发送')).toBeInTheDocument();
  expect(
    screen.getByText(
      formatDateTime(new Date(maintained.delivery_window!.expires_at_ms!))
    )
  ).toBeInTheDocument();
  expect(commands).toHaveLength(1);
  expect(commands[0]).toMatchObject({
    path: '/api/console/communications/instances/instance-1/authorization',
    body: { type: 'maintain' }
  });
  expect(saved).toHaveBeenCalledOnce();
  await act(async () => {
    await vi.advanceTimersByTimeAsync(15000);
  });
  expect(commands).toHaveLength(1);
});

test.each(['unauthorized', 'pending', 'invalid'] as const)(
  'does not offer subscription refresh for %s authorization',
  async (status) => {
    await renderSettled(
      <NotificationAuthorization
        instance_id={instance.id}
        descriptor={type.authorization!}
        authorization={{ status }}
        onChanged={saved}
      />
    );
    expect(
      screen.queryByRole('button', { name: /刷新状态/ })
    ).not.toBeInTheDocument();
  }
);

test('authorization without a delivery window keeps generic messaging without inventing a subscription', async () => {
  await renderSettled(
    <NotificationAuthorization
      instance_id={instance.id}
      descriptor={type.authorization!}
      authorization={{ status: 'authorized' }}
      onChanged={saved}
    />
  );
  expect(screen.getByText('授权已完成。')).toBeInTheDocument();
  expect(screen.queryByText(/通知订阅|24小时/)).not.toBeInTheDocument();
  expect(screen.queryByText(/微信/)).not.toBeInTheDocument();
});

test('failed Maintain preserves authorization and subscription evidence without leaking errors', async () => {
  vi.stubGlobal(
    'fetch',
    vi.fn(async () => {
      throw new Error('SECRET_TOKEN');
    })
  );
  await renderSettled(
    <NotificationAuthorization
      instance_id={instance.id}
      descriptor={type.authorization!}
      authorization={{
        status: 'authorized',
        delivery_window: { status: 'unknown', ...windowTimes }
      }}
      onChanged={saved}
    />
  );
  await click(screen.getByRole('button', { name: /刷新状态/ }));
  await screen.findByText('授权操作未完成，请重试。');
  expect(screen.getByText('已授权')).toBeInTheDocument();
  expect(screen.getByText('订阅状态待确认')).toBeInTheDocument();
  expect(screen.queryByText('SECRET_TOKEN')).not.toBeInTheDocument();
  expect(saved).not.toHaveBeenCalled();
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
async function mouseDown(...args: Parameters<typeof fireEvent.mouseDown>) {
  await act(async () => {
    fireEvent.mouseDown(...args);
  });
}
