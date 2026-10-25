import { createRef } from 'react';
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, expect, test, vi } from 'vitest';
import {
  CommunicationBindingForm,
  type CommunicationBindingHandle
} from '../communications/CommunicationBindingForm';
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../../../../state/auth-store';

let client: QueryClient;
afterEach(async () => {
  cleanup();
  client?.clear();
  resetAuthStore();
  vi.unstubAllGlobals();
  await appI18n.changeLanguage('zh_Hans');
});
function setup(existing = false, fail = false) {
  const writes: Array<Record<string, unknown>> = [];
  const capabilities = {
    model_selection_enabled: true,
    reasoning_effort_enabled: true,
    models: [
      {
        id: 'model-1',
        name: 'Model One',
        context_window: null,
        reasoning_efforts: ['low', 'high'],
        default_reasoning_effort: 'low'
      }
    ]
  };
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string, init?: RequestInit) => {
      const path = new URL(input, 'http://localhost').pathname;
      if (init?.method === 'PUT') {
        writes.push(JSON.parse(String(init.body)));
        if (fail)
          return new Response(
            JSON.stringify({ error: { message: 'failure' } }),
            { status: 500 }
          );
        return respond(writes.at(-1));
      }
      if (path.endsWith('/options'))
        return respond({
          published_agent_flows: [
            { application_id: 'app-1', name: 'Published App' }
          ],
          enabled_mcp_instances: [
            { instance_id: 'extra', name: 'Extra service' },
            { instance_id: 'dedicated', name: 'Channel service' }
          ],
          run_capabilities: capabilities
        });
      if (path.endsWith('/tables'))
        return respond([{ id: 'table-1', title: 'Files' }]);
      return respond({
        plugin_mcp_instance_id: 'dedicated',
        default_system_prompt: 'Plugin prompt',
        binding: existing
          ? {
              application_id: 'app-1',
              model: 'model-1',
              reasoning_effort: 'high',
              file_table_id: null,
              mcp_instance_ids: ['dedicated', 'extra'],
              system_prompt: 'Custom prompt'
            }
          : null
      });
    })
  );
  useAuthStore.setState({ csrfToken: 'csrf' });
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const ref = createRef<CommunicationBindingHandle>();
  render(
    <QueryClientProvider client={client}>
      <CommunicationBindingForm
        ref={ref}
        instance_id={existing ? 'channel-1' : undefined}
        receive={{ default_system_prompt: 'Plugin prompt', mcp_tools: [] }}
      />
    </QueryClientProvider>
  );
  return { ref, writes, capabilities };
}
function respond(data: unknown) {
  return new Response(JSON.stringify({ data, meta: null }), {
    headers: { 'content-type': 'application/json' }
  });
}
async function select(label: string, option: string) {
  fireEvent.mouseDown(screen.getByRole('combobox', { name: label }));
  fireEvent.click(
    await screen.findByText(option, {
      selector: '.ant-select-item-option-content'
    })
  );
}
test('new binding requires an application and saves plugin tools, multiple MCPs, edited prompt and nullable default table', async () => {
  await appI18n.changeLanguage('en_US');
  const { ref, writes } = setup();
  await waitFor(() => expect(client.isFetching()).toBe(0));
  act(() => expect(ref.current?.validate()).toBe(false));
  await select('Agent-flow application', 'Published App');
  await waitFor(() => expect(client.isFetching()).toBe(0));
  await select('MCP services', 'Extra service');
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
      system_prompt: 'Edited instructions'
    }
  ]);
});
test('existing binding preserves MCP multiselection, prompt and model reasoning; failed save rejects', async () => {
  await appI18n.changeLanguage('en_US');
  const { ref, writes } = setup(true, true);
  await screen.findByDisplayValue('Custom prompt');
  await waitFor(() => expect(client.isFetching()).toBe(0));
  expect(screen.getByText('Model One')).toBeInTheDocument();
  expect(
    screen.getByRole('combobox', { name: 'Reasoning effort' })
  ).not.toBeDisabled();
  await act(async () => {
    await expect(ref.current?.save('channel-1')).rejects.toThrow();
  });
  expect(writes[0]).toMatchObject({
    mcp_instance_ids: ['dedicated', 'extra'],
    reasoning_effort: 'high',
    system_prompt: 'Custom prompt',
    file_table_id: null
  });
});
