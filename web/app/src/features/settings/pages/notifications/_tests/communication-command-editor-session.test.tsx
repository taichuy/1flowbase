import type {
  CommunicationCommandConfiguration,
  CommunicationCommandView,
  EventCatalogEntry
} from '@1flowbase/api-client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { ConfigProvider } from 'antd';
import enUS from 'antd/locale/en_US';
import { I18nextProvider } from 'react-i18next';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../../../../state/auth-store';
import { selectNavigationQueryScope } from '../../../../../state/navigation-query-scope';
import { communicationEventCatalogOptions } from '../../../../notifications/api/communications';
import { CommunicationCommandsEditor } from '../communications/CommunicationCommandsEditor';

interface VisibleCommit {
  action: string;
  identity: string | null;
  name: string | null;
  description: string | null;
  enabled: boolean;
  pendingField: string | null;
  fields: string[];
}
const observed = vi.hoisted(() => ({
  action: '',
  commits: [] as VisibleCommit[]
}));

// Keep the real Modal, Portal, Form and all descendant commits observable.
// A parent layout effect misses commits made by the Modal's own lifecycle.
vi.mock('../communications/CommunicationCommandEditor', async (original) => {
  const actual = await original<typeof import('../communications/CommunicationCommandEditor')>();
  const { Profiler } = await import('react');
  const { screen, within } = await import('@testing-library/react');
  return {
    CommunicationCommandEditor: (props: Parameters<typeof actual.CommunicationCommandEditor>[0]) => {
      function recordVisibleCommit() {
        const dialog = screen.queryByRole('dialog');
        if (!dialog) return;
        const ui = within(dialog);
        const text = (name: string) =>
          (ui.queryByRole('textbox', { name }) as HTMLInputElement | HTMLTextAreaElement | null)?.value ?? null;
        observed.commits.push({
          action: observed.action,
          identity: text('Command identity'),
          name: text('Slash command name'),
          description: text('Description'),
          enabled: ui.queryByRole('switch')?.getAttribute('aria-checked') === 'true',
          pendingField: text('Payload field name'),
          fields: Array.from(dialog.querySelectorAll('label'))
            .map((label) => label.textContent ?? '')
            .filter((label) => label.startsWith('Payload field: '))
        });
      }
      return (
        <Profiler id="communication-command-visible-commits" onRender={recordVisibleCommit}>
          <actual.CommunicationCommandEditor {...props} />
        </Profiler>
      );
    }
  };
});

let client: QueryClient | undefined;
beforeEach(async () => {
  localStorage.clear();
  observed.action = '';
  observed.commits = [];
  await appI18n.changeLanguage('en_US');
});
afterEach(async () => {
  cleanup();
  client?.clear();
  resetAuthStore();
  localStorage.clear();
  vi.unstubAllGlobals();
  await appI18n.changeLanguage('zh_Hans');
});

const savedCommands: CommunicationCommandConfiguration[] = [
  {
    kind: 'event', command_id: 'user/invoice', name: 'invoice',
    description: 'Saved invoice command', enabled: true,
    contract_id: 'custom.invoice', input_mapping: { invoice: '/arguments' }
  },
  {
    kind: 'event', command_id: 'user/receipt', name: 'receipt',
    description: 'Saved receipt command', enabled: false,
    contract_id: 'custom.invoice', input_mapping: { invoice: '/input_id' }
  }
];

function setup(initialValue: CommunicationCommandConfiguration[] = []) {
  useAuthStore.setState({
    csrfToken: 'csrf',
    actor: {
      id: 'user-1', account: 'test', effective_display_role: 'root',
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'user-1', account: 'test', email: '', phone: null, nickname: '',
      name: '', avatar_url: null, introduction: '',
      effective_display_role: 'root', permissions: []
    }
  });
  let catalog: EventCatalogEntry[] = [{
    id: 'event-1', enabled: true, origin: 'custom', contract_id: 'custom.invoice',
    name: 'Invoice requested', description: 'Invoice request', owner_id: null,
    scope: { kind: 'workspace', workspace_id: 'workspace-1' },
    payload_schema: {
      type: 'object', properties: { invoice: { type: 'string' } },
      required: ['invoice'], additionalProperties: true
    },
    supported_phases: ['subscribed', 'published', 'consumed']
  }];
  vi.stubGlobal('fetch', vi.fn(async (input: string) => {
    const url = new URL(input, 'http://localhost');
    if (url.pathname !== '/api/console/events')
      throw new Error(`Unexpected session-fixture request: ${url.pathname}`);
    return new Response(JSON.stringify({ data: catalog, meta: null }), {
      headers: { 'content-type': 'application/json' }
    });
  }));
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false } }
  });
  client = queryClient;
  let value = initialValue;
  let commands: CommunicationCommandView[] = [{
    command_id: 'host/help', name: 'help', description: 'Built-in help', usage: '/help',
    source_kind: 'builtin', source_name: '1flowbase', installation_id: null,
    enabled: true, available: true, unavailable_reason: null
  }];
  const scene = () => (
    <I18nextProvider i18n={appI18n}>
      <ConfigProvider locale={enUS}>
        <QueryClientProvider client={queryClient}>
          <CommunicationCommandsEditor
            value={value} commands={commands} loading={false} disabled={false}
            onChange={(next) => { value = next; view.rerender(scene()); }}
          />
        </QueryClientProvider>
      </ConfigProvider>
    </I18nextProvider>
  );
  const view = render(scene());
  return {
    async refresh() {
      value = value.map((command) => command.kind === 'event'
        ? { ...command, input_mapping: { ...command.input_mapping } }
        : { ...command });
      commands = commands.map((command) => ({ ...command }));
      catalog = catalog.map((event) => ({ ...event, name: 'Invoice refreshed' }));
      view.rerender(scene());
      await act(async () => {
        await queryClient.refetchQueries({
          queryKey: communicationEventCatalogOptions(
            selectNavigationQueryScope(useAuthStore.getState()), true
          ).queryKey
        });
      });
      await waitFor(() => expect(screen.getByRole('dialog')).toHaveTextContent('Invoice refreshed'));
    }
  };
}

async function openAdd(action: string) {
  observed.action = action;
  fireEvent.click(screen.getByRole('button', { name: 'Add command' }));
  const dialog = await screen.findByRole('dialog');
  await waitFor(() => expect(within(dialog).getByRole('button', { name: 'Submit' })).toBeEnabled());
  return dialog;
}

async function openEdit(name: string, action: string) {
  observed.action = action;
  fireEvent.click(within(screen.getByRole('row', { name: new RegExp(`/${name} `) }))
    .getByRole('button', { name: 'Edit' }));
  const dialog = await screen.findByRole('dialog');
  await waitFor(() => expect(within(dialog).getByRole('button', { name: 'Submit' })).toBeEnabled());
  return dialog;
}

async function cancelAndFinishExit(dialog: HTMLElement) {
  fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
  // Wait for actual destruction, not only inaccessible or display:none content.
  await waitFor(() => expect(dialog).not.toBeInTheDocument());
}

async function chooseTarget(dialog: HTMLElement) {
  fireEvent.mouseDown(within(dialog).getByRole('combobox', { name: 'Target custom event' }));
  fireEvent.click(await screen.findByText('Invoice requested', {
    selector: '.ant-select-item-option-content'
  }));
  await within(dialog).findByRole('combobox', { name: 'Payload field: invoice' });
}

function dirtySession(dialog: HTMLElement) {
  const ui = within(dialog);
  fireEvent.change(ui.getByRole('textbox', { name: 'Slash command name' }), {
    target: { value: 'unsaved' }
  });
  fireEvent.change(ui.getByRole('textbox', { name: 'Description' }), {
    target: { value: 'Unsaved description' }
  });
  fireEvent.change(ui.getByRole('textbox', { name: 'Payload field name' }), {
    target: { value: 'custom_unsaved' }
  });
  fireEvent.click(ui.getByRole('button', { name: 'Add field mapping' }));
  expect(ui.getByRole('combobox', { name: 'Payload field: custom_unsaved' })).toBeInTheDocument();
  fireEvent.change(ui.getByRole('textbox', { name: 'Payload field name' }), {
    target: { value: 'pending_unsaved' }
  });
}

function firstVisible(action: string) {
  const commits = observed.commits.filter((commit) => commit.action === action);
  expect(commits.length).toBeGreaterThan(0);
  return commits[0];
}

function expectNewSession(action: string) {
  expect(firstVisible(action)).toMatchObject({
    identity: '', name: '', description: '', enabled: true, pendingField: '', fields: []
  });
}

test('fully closed Add sessions start with empty form and custom-field drafts on their first visible commit', async () => {
  setup();
  const first = await openAdd('first-add');
  await chooseTarget(first);
  fireEvent.change(within(first).getByRole('textbox', { name: 'Command identity' }), {
    target: { value: 'user/unsaved' }
  });
  dirtySession(first);
  fireEvent.click(within(first).getByRole('switch'));
  await cancelAndFinishExit(first);
  const reopened = await openAdd('reopen-add');
  expectNewSession('reopen-add');
  expect(within(reopened).getByRole('textbox', { name: 'Command identity' })).toBeEnabled();
});

test('closed saved edits reopen from saved values and stay distinct from new and other command sessions', async () => {
  setup(savedCommands);
  const first = await openEdit('invoice', 'first-edit');
  dirtySession(first);
  fireEvent.click(within(first).getByRole('switch'));
  await cancelAndFinishExit(first);
  const reopened = await openEdit('invoice', 'reopen-same-edit');
  expect(firstVisible('reopen-same-edit')).toMatchObject({
    identity: 'user/invoice', name: 'invoice', description: 'Saved invoice command',
    enabled: true, pendingField: '', fields: ['Payload field: invoice']
  });
  expect(within(reopened).getByRole('textbox', { name: 'Command identity' })).toBeDisabled();
  dirtySession(reopened);
  await cancelAndFinishExit(reopened);
  const fresh = await openAdd('edit-to-new');
  expectNewSession('edit-to-new');
  await cancelAndFinishExit(fresh);
  await openEdit('receipt', 'different-edit');
  expect(firstVisible('different-edit')).toMatchObject({
    identity: 'user/receipt', name: 'receipt', description: 'Saved receipt command',
    enabled: false, pendingField: '', fields: ['Payload field: invoice']
  });
});

test('immutable parent and event-catalog refresh keep an open dirty edit and its Modal DOM instance', async () => {
  const { refresh } = setup(savedCommands);
  const dialog = await openEdit('invoice', 'open-before-refresh');
  dirtySession(dialog);
  fireEvent.click(within(dialog).getByRole('switch'));
  observed.action = 'same-session-refresh';
  await refresh();
  expect(screen.getByRole('dialog')).toBe(dialog);
  const commits = observed.commits.filter((commit) => commit.action === 'same-session-refresh');
  expect(commits.length).toBeGreaterThan(0);
  for (const commit of commits) {
    expect(commit).toMatchObject({
      identity: 'user/invoice', name: 'unsaved', description: 'Unsaved description',
      enabled: false, pendingField: 'pending_unsaved',
      fields: ['Payload field: custom_unsaved', 'Payload field: invoice']
    });
  }
  expect(within(dialog).getByRole('textbox', { name: 'Payload field name' })).toHaveValue('pending_unsaved');
});
