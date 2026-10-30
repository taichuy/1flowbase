import {
  act,
  cleanup,
  fireEvent,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import type { QueryClient } from '@tanstack/react-query';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import { resetAuthStore } from '../../../../../state/auth-store';
import {
  selectBindingOption,
  setupCommunicationBinding
} from './communication-binding-fixture';

let client: QueryClient;
beforeEach(async () => {
  await appI18n.changeLanguage('en_US');
});
afterEach(async () => {
  cleanup();
  client?.clear();
  resetAuthStore();
  vi.unstubAllGlobals();
  await appI18n.changeLanguage('zh_Hans');
});
function setup(...args: Parameters<typeof setupCommunicationBinding>) {
  const result = setupCommunicationBinding(...args);
  client = result.client;
  return result;
}
const commandSwitch = (name: string) =>
  screen.getByRole('switch', { name: `Enable ${name}` });
const settle = (
  state: 'ready' | 'options-error' | 'binding-error' = 'ready'
) =>
  waitFor(() => {
    // Query completion precedes the observer's React render. Wait for the
    // command catalogue (or its load error) to reach the form as well.
    expect(client.isFetching()).toBe(0);
    if (state === 'ready') {
      expect(commandSwitch('new')).toBeEnabled();
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    } else {
      expect(screen.getByRole('alert')).toHaveTextContent(
        'Could not load channel settings.'
      );
      if (state === 'binding-error') {
        expect(commandSwitch('new')).toBeDisabled();
      } else {
        expect(screen.queryByRole('switch')).not.toBeInTheDocument();
      }
    }
  });

test('new binding requires an application and saves backend command defaults with tools, prompt and nullable table', async () => {
  const { ref, writes, optionRequests } = setup();
  await settle();
  expect(commandSwitch('new')).toBeChecked();
  expect(commandSwitch('model')).toBeChecked();
  expect(commandSwitch('help')).toBeChecked();
  expect(commandSwitch('summary')).not.toBeChecked();
  expect(commandSwitch('audit')).toBeDisabled();
  act(() => expect(ref.current?.validate()).toBe(false));
  await selectBindingOption('Agent-flow application', 'Published App');
  await settle();
  await selectBindingOption('MCP services', 'Extra service');
  fireEvent.change(screen.getByRole('textbox', { name: 'System prompt' }), {
    target: { value: 'Edited instructions' }
  });
  await act(async () => {
    expect(ref.current?.validate()).toBe(true);
    await ref.current?.save('new-channel');
  });
  expect(writes).toEqual([
    {
      application_id: 'app-1',
      model: null,
      reasoning_effort: null,
      file_table_id: null,
      mcp_instance_ids: ['extra', 'dedicated'],
      system_prompt: 'Edited instructions',
      command_settings: [
        { command_id: 'host/new', enabled: true },
        { command_id: 'host/model', enabled: true },
        { command_id: 'host/help', enabled: true },
        { command_id: 'acme/reports/summary', enabled: false },
        { command_id: 'acme/paused/audit', enabled: false }
      ]
    }
  ]);
  expect(
    optionRequests.every((url) => !url.searchParams.has('instance_id'))
  ).toBe(true);
});

test('source metadata and availability come from the catalogue; only built-in descriptions are localized', async () => {
  setup();
  await settle();
  const row = screen.getByRole('row', { name: /\/summary \[week\]/ });
  expect(within(row).getByText('Plugin')).toBeInTheDocument();
  expect(within(row).getByText('Weekly reporting plugin')).toBeInTheDocument();
  expect(
    within(row).getByText(
      'Summarize records using installed reporting settings.'
    )
  ).toBeInTheDocument();
  expect(commandSwitch('summary')).toBeEnabled();
  expect(commandSwitch('audit')).toBeDisabled();
  expect(
    screen.getByText('Unavailable: The command plugin is unavailable')
  ).toBeInTheDocument();
  await act(async () => {
    await appI18n.changeLanguage('zh_Hans');
  });
  expect(
    screen.getByText('停止当前回复并开始新的 AI 会话，保留已选模型。')
  ).toBeInTheDocument();
  expect(
    screen.getByText('Summarize records using installed reporting settings.')
  ).toBeInTheDocument();
  expect(
    screen.getByRole('switch', { name: '启用 summary' })
  ).not.toBeChecked();
});

test('editing switches saves canonical fields and reopening reads the persisted settings', async () => {
  const { ref, writes, view, mount } = setup({ existing: true });
  await screen.findByDisplayValue('Custom prompt');
  await settle();
  expect(commandSwitch('help')).not.toBeChecked();
  expect(commandSwitch('summary')).toBeChecked();
  fireEvent.click(commandSwitch('help'));
  fireEvent.click(commandSwitch('summary'));
  fireEvent.change(screen.getByRole('textbox', { name: 'System prompt' }), {
    target: { value: 'Saved command instructions' }
  });
  await act(async () => {
    expect(ref.current?.validate()).toBe(true);
    await ref.current?.save('channel-1');
  });
  expect(writes[0]).toEqual({
    application_id: 'app-1',
    model: 'model-1',
    reasoning_effort: 'high',
    file_table_id: null,
    mcp_instance_ids: ['dedicated', 'extra'],
    system_prompt: 'Saved command instructions',
    command_settings: expect.arrayContaining([
      { command_id: 'host/new', enabled: true },
      { command_id: 'host/model', enabled: true },
      { command_id: 'host/help', enabled: true },
      { command_id: 'acme/reports/summary', enabled: false },
      { command_id: 'acme/paused/audit', enabled: false },
      { command_id: 'acme/retired/archive', enabled: true }
    ])
  });
  expect(writes[0].command_settings).toHaveLength(6);
  view.unmount();
  client.clear();
  mount('channel-1');
  await screen.findByDisplayValue('Saved command instructions');
  await settle();
  expect(commandSwitch('help')).toBeChecked();
  expect(commandSwitch('summary')).not.toBeChecked();
  expect(commandSwitch('archive')).toBeChecked();
});

test('an unrecognized plugin reason remains visible with its supplied description', async () => {
  setup({ pluginUnavailableReason: 'Access to audit records has expired.' });
  await settle();
  expect(
    screen.getByText('Unavailable: Access to audit records has expired.')
  ).toBeInTheDocument();
  expect(
    screen.getByText('Audit records with the selected plugin.')
  ).toBeInTheDocument();
  expect(commandSwitch('audit')).toBeDisabled();
});

test('unavailable enabled commands remain disableable and saved missing declarations are retained', async () => {
  const { ref, writes } = setup({ existing: true });
  await screen.findByDisplayValue('Custom prompt');
  await settle();
  expect(commandSwitch('archive')).toBeChecked();
  expect(commandSwitch('archive')).toBeEnabled();
  expect(
    screen.getByText(
      'Unavailable: The command declaration is no longer installed'
    )
  ).toBeInTheDocument();
  fireEvent.click(commandSwitch('archive'));
  expect(commandSwitch('archive')).not.toBeChecked();
  expect(commandSwitch('archive')).toBeDisabled();
  fireEvent.click(commandSwitch('archive'));
  fireEvent.click(commandSwitch('audit'));
  await act(async () => {
    await ref.current?.save('channel-1');
  });
  expect(writes[0].command_settings).toEqual(
    expect.arrayContaining([
      { command_id: 'acme/retired/archive', enabled: false },
      { command_id: 'acme/paused/audit', enabled: false }
    ])
  );
});

test('changing the application keeps unsaved toggles and requests the saved-channel capability projection', async () => {
  const { ref, writes, optionRequests } = setup({ existing: true });
  await screen.findByDisplayValue('Custom prompt');
  await settle();
  fireEvent.click(commandSwitch('new'));
  fireEvent.click(commandSwitch('help'));
  await selectBindingOption('Agent-flow application', 'Restricted App');
  await settle();
  expect(optionRequests.at(-1)?.searchParams.get('application_id')).toBe(
    'app-2'
  );
  expect(
    optionRequests.every(
      (url) => url.searchParams.get('instance_id') === 'channel-1'
    )
  ).toBe(true);
  expect(commandSwitch('new')).not.toBeChecked();
  expect(commandSwitch('help')).toBeChecked();
  expect(commandSwitch('model')).toBeChecked();
  expect(commandSwitch('model')).toBeEnabled();
  // The general model selector is enabled in this fixture. The command's own
  // backend availability must still govern its switch and reason.
  expect(screen.getByRole('combobox', { name: 'Model' })).toBeEnabled();
  expect(
    screen.getByText(
      'Unavailable: This application does not allow model selection'
    )
  ).toBeInTheDocument();
  fireEvent.click(commandSwitch('model'));
  expect(commandSwitch('model')).toBeDisabled();
  await act(async () => {
    await ref.current?.save('channel-1');
  });
  expect(writes[0]).toMatchObject({
    application_id: 'app-2',
    model: null,
    reasoning_effort: null,
    command_settings: expect.arrayContaining([
      { command_id: 'host/new', enabled: false },
      { command_id: 'host/model', enabled: false },
      { command_id: 'host/help', enabled: true },
      { command_id: 'acme/retired/archive', enabled: true }
    ])
  });
});

test('an unavailable bound application stays readable and displays its backend command reason in both languages', async () => {
  setup({ existing: true, applicationUnavailable: true });
  await screen.findByDisplayValue('Custom prompt');
  await settle();
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  expect(
    screen.getByText(
      'Unavailable: The application is unavailable or inaccessible'
    )
  ).toBeInTheDocument();
  expect(commandSwitch('model')).toBeChecked();
  expect(commandSwitch('model')).toBeEnabled();
  expect(commandSwitch('summary')).toBeChecked();
  fireEvent.click(commandSwitch('model'));
  expect(commandSwitch('model')).toBeDisabled();
  await act(async () => {
    await appI18n.changeLanguage('zh_Hans');
  });
  expect(screen.getByText('不可用：应用不可用或无权访问')).toBeInTheDocument();
  expect(screen.getByRole('switch', { name: '启用 model' })).not.toBeChecked();
});

test('conflicting installations remain visible and disabling their shared command writes one setting', async () => {
  const { ref, writes } = setup({ existing: true, duplicateIdentity: true });
  await screen.findByDisplayValue('Custom prompt');
  await settle();
  const switches = screen.getAllByRole('switch', { name: 'Enable summary' });
  expect(switches).toHaveLength(2);
  for (const control of switches) expect(control).toBeChecked();
  expect(screen.getByText('Second reporting installation')).toBeInTheDocument();
  fireEvent.click(switches[0]);
  for (const control of switches) {
    expect(control).not.toBeChecked();
    expect(control).toBeDisabled();
  }
  await act(async () => {
    await ref.current?.save('channel-1');
  });
  expect(
    writes[0].command_settings.filter(
      (setting) => setting.command_id === 'acme/reports/summary'
    )
  ).toEqual([{ command_id: 'acme/reports/summary', enabled: false }]);
});

test('an unrelated edit does not erase a saved setting omitted from a refreshed catalogue', async () => {
  const { ref, writes } = setup({
    existing: true,
    omitRetiredFromCatalog: true
  });
  await screen.findByDisplayValue('Custom prompt');
  await settle();
  fireEvent.change(screen.getByRole('textbox', { name: 'System prompt' }), {
    target: { value: 'Keep every saved switch' }
  });
  await act(async () => {
    await ref.current?.save('channel-1');
  });
  expect(writes[0].command_settings).toContainEqual({
    command_id: 'acme/retired/archive',
    enabled: true
  });
});

test('failed save rejects without losing command, MCP, prompt or model drafts', async () => {
  const { ref, writes } = setup({ existing: true, failSave: true });
  await screen.findByDisplayValue('Custom prompt');
  await settle();
  fireEvent.click(commandSwitch('help'));
  expect(screen.getByText('Model One')).toBeInTheDocument();
  expect(
    screen.getByRole('combobox', { name: 'Reasoning effort' })
  ).toBeEnabled();
  await act(async () => {
    await expect(ref.current?.save('channel-1')).rejects.toThrow();
  });
  expect(commandSwitch('help')).toBeChecked();
  expect(writes[0]).toMatchObject({
    mcp_instance_ids: ['dedicated', 'extra'],
    reasoning_effort: 'high',
    system_prompt: 'Custom prompt',
    file_table_id: null,
    command_settings: expect.arrayContaining([
      { command_id: 'host/help', enabled: true }
    ])
  });
});

test.each(['options', 'binding'] as const)(
  'a failed %s load is an error and cannot save inferred settings',
  async (source) => {
    const { ref, writes } = setup({
      existing: source === 'binding',
      failOptions: source === 'options',
      failBinding: source === 'binding'
    });
    await settle(`${source}-error`);
    expect(screen.getByRole('alert')).toHaveTextContent(
      'Could not load channel settings.'
    );
    expect(screen.queryByText('No chat commands')).not.toBeInTheDocument();
    if (source === 'binding') {
      expect(commandSwitch('new')).toBeDisabled();
    } else {
      expect(screen.queryByRole('switch')).not.toBeInTheDocument();
    }
    act(() => expect(ref.current?.validate()).toBe(false));
    expect(writes).toEqual([]);
  }
);
