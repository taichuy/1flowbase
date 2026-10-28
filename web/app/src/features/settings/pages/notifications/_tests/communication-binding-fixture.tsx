import { createRef } from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { vi } from 'vitest';
import type {
  CommunicationBindingInput,
  CommunicationBindingRecord,
  CommunicationCommandView,
  CommunicationOptions
} from '@1flowbase/api-client';
import {
  CommunicationBindingForm,
  type CommunicationBindingHandle
} from '../communications/CommunicationBindingForm';
import { useAuthStore } from '../../../../../state/auth-store';

const commands: CommunicationCommandView[] = [
  ...(['new', 'model', 'help'] as const).map((name) => ({
    command_id: `host/${name}`,
    name,
    description: `Backend ${name} description`,
    usage: name === 'model' ? '/model [model]' : `/${name}`,
    source_kind: 'builtin' as const,
    source_name: '1flowbase',
    installation_id: null,
    enabled: true,
    available: true,
    unavailable_reason: null
  })),
  {
    command_id: 'acme/reports/summary',
    name: 'summary',
    description: 'Summarize records using installed reporting settings.',
    usage: '/summary [week]',
    source_kind: 'plugin',
    source_name: 'Weekly reporting plugin',
    installation_id: 'reporting-plugin',
    enabled: false,
    available: true,
    unavailable_reason: null
  },
  {
    command_id: 'acme/paused/audit',
    name: 'audit',
    description: 'Audit records with the selected plugin.',
    usage: '/audit',
    source_kind: 'plugin',
    source_name: 'Audit plugin',
    installation_id: 'audit-plugin',
    enabled: false,
    available: false,
    unavailable_reason: 'plugin_unavailable'
  }
];
const retiredCommand: CommunicationCommandView = {
  command_id: 'acme/retired/archive',
  name: 'archive',
  description: 'The previous archive declaration is no longer installed.',
  usage: '',
  source_kind: 'plugin',
  source_name: 'Retired archive plugin',
  installation_id: null,
  enabled: true,
  available: false,
  unavailable_reason: 'command_not_installed'
};
const capabilities: CommunicationOptions['run_capabilities'] = {
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

export function setupCommunicationBinding({
  existing = false,
  failSave = false,
  failOptions = false,
  failBinding = false,
  omitRetiredFromCatalog = false,
  pluginUnavailableReason = 'plugin_unavailable',
  duplicateIdentity = false
}: {
  existing?: boolean;
  failSave?: boolean;
  failOptions?: boolean;
  failBinding?: boolean;
  omitRetiredFromCatalog?: boolean;
  pluginUnavailableReason?: string;
  duplicateIdentity?: boolean;
} = {}) {
  const writes: CommunicationBindingInput[] = [];
  const optionRequests: URL[] = [];
  let saved: CommunicationBindingInput | null = existing
    ? {
        application_id: 'app-1',
        model: 'model-1',
        reasoning_effort: 'high',
        file_table_id: null,
        mcp_instance_ids: ['dedicated', 'extra'],
        system_prompt: 'Custom prompt',
        command_settings: [
          { command_id: 'host/help', enabled: false },
          { command_id: 'acme/reports/summary', enabled: true },
          { command_id: 'acme/retired/archive', enabled: true }
        ]
      }
    : null;
  const record = (): CommunicationBindingRecord | null =>
    saved && {
      ...saved,
      workspace_id: 'workspace-1',
      instance_id: 'channel-1',
      actor_user_id: 'user-1',
      plugin_mcp_instance_id: 'dedicated',
      revision: writes.length + 1
    };
  const catalog = (
    application_id?: string | null,
    instance_id?: string | null
  ) => {
    const settings = instance_id ? saved?.command_settings : undefined;
    const entries = [
      ...commands,
      ...(settings?.some(
        (setting) => setting.command_id === retiredCommand.command_id
      ) && !omitRetiredFromCatalog
        ? [retiredCommand]
        : [])
    ];
    if (duplicateIdentity)
      entries.push({
        ...commands[3]!,
        installation_id: 'other-reporting-plugin',
        source_name: 'Second reporting installation'
      });
    return entries.map((command) => ({
      ...command,
      unavailable_reason:
        command.name === 'audit'
          ? pluginUnavailableReason
          : command.unavailable_reason,
      enabled:
        settings?.find((setting) => setting.command_id === command.command_id)
          ?.enabled ?? command.enabled,
      ...(duplicateIdentity && command.command_id === 'acme/reports/summary'
        ? { available: false, unavailable_reason: 'duplicate_command_identity' }
        : {}),
      ...(application_id === 'app-2' && command.command_id === 'host/model'
        ? {
            available: false,
            unavailable_reason: 'model_selection_unavailable'
          }
        : {})
    }));
  };
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string, init?: RequestInit) => {
      const url = new URL(input, 'http://localhost');
      if (init?.method === 'PUT' && url.pathname.endsWith('/binding')) {
        writes.push(JSON.parse(String(init.body)));
        if (failSave) return failure();
        saved = writes.at(-1)!;
        return respond(record());
      }
      if (url.pathname.endsWith('/options')) {
        optionRequests.push(url);
        if (failOptions) return failure();
        const data: CommunicationOptions = {
          published_agent_flows: [
            { application_id: 'app-1', name: 'Published App' },
            { application_id: 'app-2', name: 'Restricted App' }
          ],
          enabled_mcp_instances: [
            { instance_id: 'extra', name: 'Extra service' },
            { instance_id: 'dedicated', name: 'Channel service' }
          ],
          run_capabilities: capabilities,
          commands: catalog(
            url.searchParams.get('application_id'),
            url.searchParams.get('instance_id')
          )
        };
        return respond(data);
      }
      if (url.pathname.endsWith('/tables'))
        return respond([{ id: 'table-1', title: 'Files' }]);
      if (url.pathname.endsWith('/binding')) {
        if (failBinding) return failure();
        return respond({
          plugin_mcp_instance_id: 'dedicated',
          default_system_prompt: 'Plugin prompt',
          binding: record(),
          commands: catalog(saved?.application_id, 'channel-1')
        });
      }
      return failure();
    })
  );
  useAuthStore.setState({ csrfToken: 'csrf' });
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  const ref = createRef<CommunicationBindingHandle>();
  const mount = (instance_id?: string) =>
    render(
      <QueryClientProvider client={client}>
        <CommunicationBindingForm
          ref={ref}
          instance_id={instance_id}
          receive={{ default_system_prompt: 'Plugin prompt', mcp_tools: [] }}
        />
      </QueryClientProvider>
    );
  const view = mount(existing ? 'channel-1' : undefined);
  return { ref, writes, client, view, mount, optionRequests };
}

function respond(data: unknown) {
  return new Response(JSON.stringify({ data, meta: null }), {
    headers: { 'content-type': 'application/json' }
  });
}
function failure() {
  return new Response(JSON.stringify({ error: { message: 'failure' } }), {
    status: 500
  });
}
export async function selectBindingOption(label: string, option: string) {
  fireEvent.mouseDown(screen.getByRole('combobox', { name: label }));
  fireEvent.click(
    await screen.findByText(option, {
      selector: '.ant-select-item-option-content'
    })
  );
}
