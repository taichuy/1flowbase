import '../../../../../test/fixtures/rc-util-unique-ids';
import type {
  ApiEventOperationDetails,
  EventCatalogEntry,
  EventRuntimeSnapshot
} from '@1flowbase/api-client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { I18nextProvider } from 'react-i18next';
import { App as AntApp } from 'antd';
import { beforeEach, describe, expect, test, vi } from 'vitest';
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import { EventsPage } from '../EventsPage';

const api = vi.hoisted(() => ({
  listEventCatalog: vi.fn(),
  createEventDefinition: vi.fn(),
  updateEventDefinition: vi.fn(),
  getEventRuntime: vi.fn(),
  listApiEventOperations: vi.fn(),
  getApiEventOperation: vi.fn()
}));
const clipboard = vi.hoisted(() => ({ copy: vi.fn() }));
vi.mock('../../../../../shared/ui/clipboard/use-clipboard-copy', () => ({
  useClipboardCopy: () => ({ copied: false, copy: clipboard.copy })
}));
vi.mock('../../../../../shared/code-block/monaco-runtime', () => ({
  loadMonacoEditorModule: async () => ({
    default: ({
      value,
      options
    }: {
      value: string;
      options?: { readOnly?: boolean };
    }) => (
      <pre data-testid="schema-json-editor" data-readonly={options?.readOnly}>
        {value}
      </pre>
    )
  })
}));
const access = vi.hoisted(() => ({
  operations: new Set([
    'events.catalog.list',
    'events.api_operations.list',
    'events.api_operations.get',
    'events.definitions.create',
    'events.definitions.update',
    'events.runtime.get'
  ])
}));
vi.mock('@1flowbase/api-client', async (original) => ({
  ...(await original<typeof import('@1flowbase/api-client')>()),
  ...api
}));
vi.mock('../../../../../state/auth-store', () => ({
  useAuthStore: () => 'csrf'
}));
vi.mock('../../../../../state/console-operation-access', () => ({
  useConsoleOperationAccess: () => (id: string) => access.operations.has(id)
}));

const schema = {
  type: 'object',
  additionalProperties: false,
  properties: {
    amount: { type: 'number', minimum: 1 },
    details: {
      type: 'object',
      properties: { note: { type: 'string', maxLength: 20 } }
    }
  },
  required: ['amount']
};
const custom: EventCatalogEntry = {
  id: 'definition',
  group_path: ['interface_events'],
  interface_event: { interface_id: 'orders.create', phase: 'before' },
  enabled: false,
  owner_id: null,
  scope: { kind: 'workspace', workspace_id: 'workspace' },
  contract_id: 'custom.order',
  name: 'Order',
  description: 'New order',
  payload_schema: schema,
  origin: 'custom',
  supported_phases: ['subscribed', 'published', 'consumed']
};
const declarations: EventCatalogEntry[] = (
  ['before', 'completion'] as const
).map((phase) => ({
  ...custom,
  id: null,
  enabled: null,
  name: `Create order ${phase}`,
  origin: 'core',
  contract_id: `interface.orders.create.${phase}`,
  interface_event: { interface_id: 'orders.create', phase },
  group_path: ['interfaces', 'orders'],
  payload_schema: {
    type: 'object',
    properties:
      phase === 'before'
        ? { input: schema }
        : {
            output: schema,
            terminal: { enum: ['completed', 'failed', 'rejected', 'cancelled'] }
          }
  }
}));
const documented: ApiEventOperationDetails = {
  operation_id: 'create_order',
  method: 'POST',
  path: '/api/console/orders',
  name: 'Create order',
  description: '',
  scope: custom.scope,
  before: { status: 'available' },
  completion: { status: 'available' },
  request_schema: schema,
  result_schema: schema,
  before_schema: declarations[0].payload_schema,
  completion_schema: declarations[1].payload_schema
};
const snapshot: EventRuntimeSnapshot = {
  instance_id: 'host-instance',
  workspace_id: 'workspace',
  configured: true,
  admission_rejected: 3,
  source_queue_refresh_error: null,
  source_views: [],
  source_metadata_bytes: 0,
  observation_bytes: 300,
  observation_max_bytes: 2000,
  observation_ttl_ms: 60000,
  observation_loss: {
    accounting_available: true,
    window_started_at_unix_ms: 1793894400000,
    window_duration_ms: 120000,
    expired: 11,
    evicted: 8,
    oversize_dropped: 6
  },
  workflow_subscriptions: [],
  workflow_refresh_error: null,
  queues: [
    {
      subscription_id: 'subscription',
      workspace_id: 'workspace',
      contract_id: 'custom.order',
      trigger_phase: 'published',
      consumer_ids: ['consumer'],
      closed: false,
      buffered: 7,
      in_flight: 2,
      accepted: 12,
      consumed: 5,
      admission_rejected: 4
    }
  ],
  activity: [
    {
      event_id: 'event',
      workspace_id: 'workspace',
      contract_id: 'custom.order',
      phase: 'consumed',
      source_event_id: 'source-event',
      subscription_id: 'subscription',
      consumer_id: 'consumer',
      admission: 'resource_exhausted',
      observed_at_unix_ms: 1000
    }
  ]
};
function withWorkspaceView(value: EventRuntimeSnapshot): EventRuntimeSnapshot {
  return {
    ...value,
    source_views: [
      {
        instance_id: value.instance_id,
        configured: value.configured,
        observation_max_bytes: value.observation_max_bytes,
        observation_ttl_ms: value.observation_ttl_ms,
        scope: { kind: 'workspace', workspace_id: value.workspace_id },
        sources: [],
        workflow_queues: value.queues,
        activity: value.activity,
        observation_bytes: value.observation_bytes,
        source_metadata_bytes: value.source_metadata_bytes,
        observation_loss: value.observation_loss
      }
    ]
  };
}
function renderPage() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } }
  });
  return render(
    <I18nextProvider i18n={appI18n}>
      <QueryClientProvider client={client}>
        <AntApp>
          <EventsPage />
        </AntApp>
      </QueryClientProvider>
    </I18nextProvider>
  );
}
beforeEach(async () => {
  vi.clearAllMocks();
  window.history.replaceState({}, '', '/settings/events');
  for (const key of Object.keys(window.localStorage)) {
    if (key.startsWith('settings.events.')) window.localStorage.removeItem(key);
  }
  await appI18n.changeLanguage('en_US');
  access.operations = new Set([
    'events.catalog.list',
    'events.api_operations.list',
    'events.api_operations.get',
    'events.definitions.create',
    'events.definitions.update',
    'events.runtime.get'
  ]);
  api.listEventCatalog.mockResolvedValue([custom]);
  api.listApiEventOperations.mockResolvedValue([]);
  api.getApiEventOperation.mockResolvedValue(documented);
  api.getEventRuntime.mockResolvedValue(withWorkspaceView(snapshot));
  api.createEventDefinition.mockResolvedValue({ id: 'new' });
  api.updateEventDefinition.mockResolvedValue({ id: 'definition' });
});

describe('event center configuration and observations', () => {
  test('catalog has business grouping and management columns and keeps source ownership in read-only details', async () => {
    api.listEventCatalog.mockResolvedValue([
      custom,
      {
        ...custom,
        id: null,
        name: 'Core ready',
        contract_id: 'core.ready',
        origin: 'core',
        enabled: null,
        owner_id: 'core.owner',
        scope: { kind: 'system' },
        source: {
          selection: { source_id: 'core.source', scope: { kind: 'system' } },
          point_id: 'core.point'
        }
      }
    ]);
    renderPage();
    await screen.findByRole('button', { name: 'Core ready' });
    expect(
      screen.getAllByRole('columnheader').map((node) => node.textContent)
    ).toEqual([
      'Name',
      'Business group',
      'Event ID',
      'Source category',
      'Scope',
      'Enabled',
      'Supported trigger phases',
      'Actions'
    ]);
    expect(screen.getByText('Workspace · workspace')).toBeInTheDocument();
    expect(screen.queryByText('core.owner')).not.toBeInTheDocument();
    expect(screen.queryByText('core.point')).not.toBeInTheDocument();
    const row = screen.getByRole('row', { name: /Core ready/ });
    expect(within(row).getByRole('switch', { name: 'Enabled' })).toBeChecked();
    expect(within(row).getByRole('switch', { name: 'Enabled' })).toBeDisabled();
    expect(
      within(row).queryByRole('button', { name: 'Edit' })
    ).not.toBeInTheDocument();
    fireEvent.click(within(row).getByRole('button', { name: 'Details' }));
    const drawer = screen.getByRole('dialog');
    expect(within(drawer).getByText('core.owner')).toBeInTheDocument();
    expect(within(drawer).getByText('core.point')).toBeInTheDocument();
    expect(
      within(drawer).getByRole('switch', { name: 'Enabled' })
    ).toBeChecked();
    expect(
      within(drawer).getByRole('switch', { name: 'Enabled' })
    ).toBeDisabled();
    expect(
      within(drawer).queryByText('Contract version')
    ).not.toBeInTheDocument();
  });

  test.each(['core', 'plugin'] as const)(
    '%s events show an enabled disabled switch and never issue updates',
    async (origin) => {
      api.listEventCatalog.mockResolvedValue([
        {
          ...custom,
          id: null,
          origin,
          enabled: null,
          owner_id: `${origin}.owner`
        }
      ]);
      renderPage();
      const row = await screen.findByRole('row', { name: /Order/ });
      const toggle = within(row).getByRole('switch', { name: 'Enabled' });
      expect(toggle).toBeChecked();
      expect(toggle).toBeDisabled();
      fireEvent.click(toggle);
      expect(api.updateEventDefinition).not.toHaveBeenCalled();
    }
  );

  test('same-contract subtype leaves and user definitions remain distinct rows', async () => {
    const source = {
      source_id: 'business.facts',
      scope: { kind: 'system' as const }
    };
    api.listEventCatalog.mockResolvedValue([
      ...['reserved', 'settled'].map((kind) => ({
        ...custom,
        id: null,
        name: kind,
        origin: 'plugin',
        group_path: ['billing', kind],
        source: {
          point_id: 'business.facts',
          selection: { ...source, filters: { kind } }
        }
      })),
      custom,
      { ...custom, id: 'another-definition', name: 'Another rule' }
    ]);
    renderPage();
    for (const name of ['reserved', 'settled', 'Order', 'Another rule'])
      expect(await screen.findByRole('button', { name })).toBeInTheDocument();
    expect(screen.getAllByRole('row')).toHaveLength(5);
    fireEvent.click(screen.getByRole('button', { name: 'settled' }));
    expect(
      within(screen.getByRole('dialog')).getByText('settled', {
        selector: '.ant-descriptions-item-content'
      })
    ).toBeInTheDocument();
  });
  test('toggles a custom event both ways, preserves its definition and waits for the server', async () => {
    let resolveUpdate!: (value: object) => void;
    api.updateEventDefinition.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveUpdate = resolve;
        })
    );
    renderPage();
    const row = await screen.findByRole('row', { name: /Order/ });
    const toggle = within(row).getByRole('switch', { name: 'Enabled' });
    expect(toggle).not.toBeChecked();
    expect(toggle).toBeEnabled();
    fireEvent.click(toggle);
    await waitFor(() =>
      expect(api.updateEventDefinition).toHaveBeenCalledWith(
        'definition',
        {
          name: 'Order',
          description: 'New order',
          interface_event: custom.interface_event,
          enabled: true
        },
        'csrf'
      )
    );
    expect(toggle).toBeDisabled();
    expect(toggle).not.toBeChecked();
    api.listEventCatalog.mockResolvedValue([{ ...custom, enabled: true }]);
    await act(async () => resolveUpdate({ id: 'definition' }));
    await waitFor(() => expect(toggle).toBeChecked());
    await waitFor(() => expect(toggle).toBeEnabled());
    api.listEventCatalog.mockResolvedValue([custom]);
    fireEvent.click(toggle);
    await waitFor(() =>
      expect(api.updateEventDefinition).toHaveBeenLastCalledWith(
        'definition',
        {
          name: 'Order',
          description: 'New order',
          interface_event: custom.interface_event,
          enabled: false
        },
        'csrf'
      )
    );
    await waitFor(() => expect(toggle).not.toBeChecked());
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  test('a failed toggle keeps the server state and shows an error', async () => {
    api.updateEventDefinition.mockRejectedValueOnce(new Error('update failed'));
    renderPage();
    const row = await screen.findByRole('row', { name: /Order/ });
    const toggle = within(row).getByRole('switch', { name: 'Enabled' });
    fireEvent.click(toggle);
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'The event could not be saved.'
    );
    expect(toggle).not.toBeChecked();
    expect(toggle).toBeEnabled();
  });

  test('custom event switches respect update permissions', async () => {
    access.operations.delete('events.definitions.update');
    renderPage();
    const row = await screen.findByRole('row', { name: /Order/ });
    const toggle = within(row).getByRole('switch', { name: 'Enabled' });
    expect(toggle).not.toBeChecked();
    expect(toggle).toBeDisabled();
    fireEvent.click(toggle);
    expect(api.updateEventDefinition).not.toHaveBeenCalled();
  });

  test.each([schema, {}, true, false, null])(
    'previews and copies the complete read-only schema %j',
    async (payload_schema) => {
      api.listEventCatalog.mockResolvedValue([
        {
          ...custom,
          origin: 'core',
          enabled: null,
          owner_id: 'core.owner',
          id: null,
          payload_schema
        }
      ]);
      renderPage();
      const row = await screen.findByRole('row', { name: /Order/ });
      fireEvent.click(within(row).getByRole('button', { name: 'Details' }));
      const editor = await screen.findByTestId('schema-json-editor');
      expect(JSON.parse(editor.textContent ?? '')).toEqual(payload_schema);
      expect(editor).toHaveAttribute('data-readonly', 'true');
      expect(
        screen.queryByRole('button', { name: 'Save' })
      ).not.toBeInTheDocument();
      fireEvent.click(
        screen.getByRole('button', { name: 'Copy Payload schema JSON' })
      );
      await waitFor(() =>
        expect(clipboard.copy).toHaveBeenCalledWith(
          JSON.stringify(payload_schema, null, 2)
        )
      );
      fireEvent.click(
        screen.getByRole('button', {
          name: 'Zoom in to view Payload schema JSON'
        })
      );
      const enlarged = await screen.findByRole('dialog', {
        name: 'Payload schema JSON'
      });
      expect(
        JSON.parse(
          within(enlarged).getByTestId('schema-json-editor').textContent ?? ''
        )
      ).toEqual(payload_schema);
    }
  );

  test('opens event details with a single direct row action click', async () => {
    api.listEventCatalog.mockResolvedValue([
      {
        ...custom,
        origin: 'core',
        enabled: null,
        owner_id: 'core.owner',
        id: null
      }
    ]);
    renderPage();
    const view = await screen.findByRole('button', { name: 'Details' });
    expect(
      screen.queryByRole('button', { name: 'Actions' })
    ).not.toBeInTheDocument();
    fireEvent.click(view);
    expect(screen.getByRole('dialog')).toBeInTheDocument();
  });

  test.each([
    ['queues', 'Current queues'],
    ['activity', 'Recent activity']
  ])('opens and restores the %s tab from its URL', async (tab, label) => {
    window.history.replaceState({}, '', `/settings/events?tab=${tab}`);
    const view = renderPage();
    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: 'Observation details' })
      ).toBeEnabled()
    );
    expect(screen.getByRole('tab', { name: label })).toHaveAttribute(
      'aria-selected',
      'true'
    );
    expect(api.listEventCatalog).not.toHaveBeenCalled();
    view.unmount();
    renderPage();
    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: 'Observation details' })
      ).toBeEnabled()
    );
    expect(screen.getByRole('tab', { name: label })).toHaveAttribute(
      'aria-selected',
      'true'
    );
  });
  test('tab navigation updates the URL and follows browser back and forward', async () => {
    renderPage();
    await screen.findByRole('button', { name: 'Order' });
    fireEvent.click(screen.getByRole('tab', { name: 'Current queues' }));
    expect(window.location.search).toBe('?tab=queues');
    fireEvent.click(screen.getByRole('tab', { name: 'Recent activity' }));
    expect(window.location.search).toBe('?tab=activity');
    await act(async () => {
      window.history.back();
      await waitFor(() => expect(window.location.search).toBe('?tab=queues'));
    });
    expect(screen.getByRole('tab', { name: 'Current queues' })).toHaveAttribute(
      'aria-selected',
      'true'
    );
    await act(async () => {
      window.history.forward();
      await waitFor(() => expect(window.location.search).toBe('?tab=activity'));
    });
    expect(
      screen.getByRole('tab', { name: 'Recent activity' })
    ).toHaveAttribute('aria-selected', 'true');
    fireEvent.click(screen.getByRole('tab', { name: 'Event catalog' }));
    expect(window.location.search).toBe('?tab=catalog');
  });
  test('shared column settings change the visible fields and persist the selection', async () => {
    access.operations.add('patch_me_meta');
    const view = renderPage();
    await screen.findByRole('button', { name: 'Order' });
    fireEvent.click(
      screen.getByRole('button', { name: 'Field configuration' })
    );
    const fields = await screen.findByRole('dialog', {
      name: 'Customize visible fields'
    });
    fireEvent.click(within(fields).getByRole('checkbox', { name: 'Scope' }));
    await waitFor(() =>
      expect(
        screen.queryByRole('columnheader', { name: 'Scope' })
      ).not.toBeInTheDocument()
    );
    view.unmount();
    renderPage();
    await screen.findByRole('button', { name: 'Order' });
    expect(
      screen.queryByRole('columnheader', { name: 'Scope' })
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: 'Event ID' })
    ).toBeInTheDocument();
  });
  test('a runtime URL cannot bypass catalog-only permissions', async () => {
    access.operations = new Set(['events.catalog.list']);
    window.history.replaceState({}, '', '/settings/events?tab=queues');
    renderPage();
    await screen.findByRole('button', { name: 'Order' });
    expect(window.location.search).toBe('?tab=catalog');
    expect(
      screen.queryByRole('tab', { name: 'Current queues' })
    ).not.toBeInTheDocument();
    expect(api.getEventRuntime).not.toHaveBeenCalled();
  });
  test.each(['before', 'completion'] as const)(
    'creates an interface rule for %s with inherited schema and no free-form contract',
    async (phase) => {
      api.listEventCatalog.mockResolvedValue([custom, ...declarations]);
      api.listApiEventOperations.mockResolvedValue([documented]);
      renderPage();
      fireEvent.click(
        await screen.findByRole('button', { name: 'Create event' })
      );
      const drawer = screen.getByRole('dialog');
      expect(
        within(drawer).queryByLabelText('Event ID')
      ).not.toBeInTheDocument();
      expect(
        within(drawer).queryByRole('button', { name: 'Add schema field' })
      ).not.toBeInTheDocument();
      fireEvent.change(within(drawer).getByLabelText('Name'), {
        target: { value: 'Request' }
      });
      fireEvent.mouseDown(
        within(drawer).getByRole('combobox', { name: 'Interface / operation' })
      );
      fireEvent.click(
        await screen.findByText('POST /api/console/orders · Create order')
      );
      await waitFor(() =>
        expect(
          within(drawer).getByRole('combobox', { name: 'Interface phase' })
        ).toBeEnabled()
      );
      fireEvent.mouseDown(
        within(drawer).getByRole('combobox', { name: 'Interface phase' })
      );
      fireEvent.click(
        await screen.findByText(
          phase === 'before' ? 'Before request' : 'After completion'
        )
      );
      const editor = await within(drawer).findByTestId('schema-json-editor');
      expect(editor).toHaveAttribute('data-readonly', 'true');
      expect(JSON.parse(editor.textContent ?? '')).toEqual(
        declarations.find((item) => item.interface_event?.phase === phase)
          ?.payload_schema
      );
      if (phase === 'completion') {
        const results = within(drawer).getByRole('combobox', {
          name: 'Terminal results'
        });
        for (const label of ['Completed', 'Failed', 'Rejected', 'Cancelled']) {
          fireEvent.mouseDown(results);
          fireEvent.click(
            await screen.findByText(label, {
              selector: '.ant-select-item-option-content'
            })
          );
        }
        fireEvent.keyDown(results, { key: 'Escape' });
      }
      fireEvent.click(within(drawer).getByRole('switch', { name: 'Enabled' }));
      await waitFor(() =>
        expect(
          within(drawer).getByRole('button', { name: 'Save' })
        ).toBeEnabled()
      );
      fireEvent.click(within(drawer).getByRole('button', { name: 'Save' }));
      await waitFor(() =>
        expect(api.createEventDefinition).toHaveBeenCalledWith(
          {
            name: 'Request',
            description: '',
            enabled: false,
            interface_event: {
              operation_id: 'create_order',
              phase,
              ...(phase === 'completion'
                ? { results: ['completed', 'failed', 'rejected', 'cancelled'] }
                : {})
            }
          },
          'csrf'
        )
      );
      expect(api.createEventDefinition.mock.calls[0][0]).not.toHaveProperty(
        'payload_schema'
      );
      expect(api.createEventDefinition.mock.calls[0][0]).not.toHaveProperty(
        'contract_id'
      );
    }
  );
  test('new rules cannot invent interfaces when the declaration catalog is empty', async () => {
    renderPage();
    fireEvent.click(
      await screen.findByRole('button', { name: 'Create event' })
    );
    expect(
      within(screen.getByRole('dialog')).getByRole('button', { name: 'Save' })
    ).toBeDisabled();
    expect(api.createEventDefinition).not.toHaveBeenCalled();
  });
  test('custom event details stay read-only even when editing is authorized', async () => {
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Details' }));
    expect(
      JSON.parse(
        (await screen.findByTestId('schema-json-editor')).textContent ?? ''
      )
    ).toEqual(schema);
    expect(
      screen.queryByRole('button', { name: 'Save' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Add schema field' })
    ).not.toBeInTheDocument();
    expect(api.updateEventDefinition).not.toHaveBeenCalled();
  });
  test('editing a disabled interface rule inherits schema and sends the binding', async () => {
    api.listEventCatalog.mockResolvedValue([custom, ...declarations]);
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Edit' }));
    const drawer = screen.getByRole('dialog');
    expect(within(drawer).queryByLabelText('Event ID')).not.toBeInTheDocument();
    expect(
      within(drawer).queryByLabelText('Contract version')
    ).not.toBeInTheDocument();
    fireEvent.change(within(drawer).getByLabelText('Name'), {
      target: { value: 'Changed order' }
    });
    await waitFor(() =>
      expect(within(drawer).getByRole('button', { name: 'Save' })).toBeEnabled()
    );
    fireEvent.click(within(drawer).getByRole('button', { name: 'Save' }));
    await waitFor(() =>
      expect(api.updateEventDefinition).toHaveBeenCalledWith(
        'definition',
        {
          name: 'Changed order',
          description: 'New order',
          enabled: false,
          interface_event: custom.interface_event
        },
        'csrf'
      )
    );
  });
  test('complete API discovery supports method path and name search and shows genuine phase restrictions', async () => {
    const restricted: ApiEventOperationDetails = {
      ...documented,
      operation_id: 'download_report',
      method: 'GET',
      path: '/api/console/reports/{id}',
      name: 'Download report',
      completion: { status: 'unavailable', reason: 'binary_response' },
      completion_schema: null
    };
    api.listApiEventOperations.mockResolvedValue([documented, restricted]);
    api.getApiEventOperation.mockImplementation(async (id: string) =>
      id === restricted.operation_id ? restricted : documented
    );
    renderPage();
    fireEvent.click(
      await screen.findByRole('button', { name: 'Create event' })
    );
    const drawer = screen.getByRole('dialog');
    const select = within(drawer).getByRole('combobox', {
      name: 'Interface / operation'
    });
    fireEvent.mouseDown(select);
    fireEvent.change(select, { target: { value: 'reports' } });
    fireEvent.click(
      await screen.findByText('GET /api/console/reports/{id} · Download report')
    );
    expect(
      screen.queryByText('interfaces / api-server')
    ).not.toBeInTheDocument();
    await waitFor(() =>
      expect(
        within(drawer).getByRole('combobox', { name: 'Interface phase' })
      ).toBeEnabled()
    );
    fireEvent.mouseDown(
      within(drawer).getByRole('combobox', { name: 'Interface phase' })
    );
    fireEvent.click(await screen.findByText('After completion'));
    expect(
      await within(drawer).findByText(
        'This phase returns binary content without an event result contract.'
      )
    ).toBeInTheDocument();
    expect(within(drawer).getByRole('button', { name: 'Save' })).toBeDisabled();
    expect(api.createEventDefinition).not.toHaveBeenCalled();
  });
  test('existing legacy rules retain their stored schema even after the source catalog changes', async () => {
    api.listEventCatalog.mockResolvedValue([custom]);
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Edit' }));
    const drawer = screen.getByRole('dialog');
    expect(
      await within(drawer).findByText(
        'This existing rule keeps its original interface binding and payload schema.'
      )
    ).toBeInTheDocument();
    expect(
      JSON.parse(
        (await within(drawer).findByTestId('schema-json-editor')).textContent ??
          ''
      )
    ).toEqual(custom.payload_schema);
    expect(api.listApiEventOperations).not.toHaveBeenCalled();
    fireEvent.change(within(drawer).getByLabelText('Name'), {
      target: { value: 'Preserved legacy' }
    });
    fireEvent.click(within(drawer).getByRole('button', { name: 'Save' }));
    await waitFor(() =>
      expect(api.updateEventDefinition).toHaveBeenCalledWith(
        'definition',
        {
          name: 'Preserved legacy',
          description: custom.description,
          enabled: false,
          interface_event: custom.interface_event
        },
        'csrf'
      )
    );
  });
  test('registered source contracts are read-only and do not fabricate supported trigger phases', async () => {
    api.listEventCatalog.mockResolvedValue([
      {
        ...custom,
        id: null,
        origin: 'plugin',
        enabled: null,
        owner_id: 'plugin.owner',
        name: 'Plugin event',
        supported_phases: []
      }
    ]);
    renderPage();
    expect(
      await screen.findByText('No source adapter available')
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Plugin event' }));
    const drawer = screen.getByRole('dialog');
    expect(
      within(drawer).queryByRole('button', { name: 'Save' })
    ).not.toBeInTheDocument();
    expect(within(drawer).getByText('plugin.owner')).toBeInTheDocument();
    expect(
      within(drawer).getByText('Workspace · workspace')
    ).toBeInTheDocument();
    expect(api.updateEventDefinition).not.toHaveBeenCalled();
  });
  test('shows queue facts separately from the activity window and exposes rejected admission without success metrics', async () => {
    renderPage();
    fireEvent.click(screen.getByRole('tab', { name: 'Current queues' }));
    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: 'Observation details' })
      ).toBeEnabled()
    );
    expect(
      screen.getByText(/Events are broadcast across subscriptions/)
    ).toHaveTextContent('consumers compete within that queue');
    fireEvent.click(screen.getByRole('button', { name: 'custom.order' }));
    expect(
      within(screen.getByRole('dialog')).getByText('Dequeued deliveries')
    ).toBeInTheDocument();
    expect(
      screen.getByText(/does not indicate workflow success or failure/)
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    fireEvent.click(
      screen.getByRole('button', { name: 'Observation details' })
    );
    expect(
      screen.getByText('Rejections retained in activity window')
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(
      screen.queryByRole('button', { name: /^consume(?: event)?$/i })
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('tab', { name: 'Recent activity' }));
    expect(
      await screen.findByText('Resource budget exhausted')
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Details' }));
    expect(
      within(screen.getByRole('dialog')).getByText('source-event')
    ).toBeInTheDocument();
  });
  test('keeps configuration usable when runtime resources are not configured', async () => {
    api.getEventRuntime.mockResolvedValue(
      withWorkspaceView({
        ...snapshot,
        configured: false,
        queues: [],
        activity: []
      })
    );
    renderPage();
    fireEvent.click(screen.getByRole('tab', { name: 'Current queues' }));
    expect(
      await screen.findByText('Runtime resources are not configured')
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('tab', { name: 'Event catalog' }));
    expect(
      await screen.findByRole('button', { name: 'Create event' })
    ).toBeEnabled();
  });
  test('shows scoped observation loss counts with their coverage window', async () => {
    renderPage();
    fireEvent.click(screen.getByRole('tab', { name: 'Recent activity' }));
    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: 'Observation details' })
      ).toBeEnabled()
    );
    fireEvent.click(
      screen.getByRole('button', { name: 'Observation details' })
    );
    expect(
      await screen.findByText('Records expired in window')
    ).toBeInTheDocument();
    const descriptions = screen
      .getByText('Records expired in window')
      .closest('tr');
    expect(descriptions).not.toBeNull();
    expect(
      within(descriptions as HTMLElement).getByText('11')
    ).toBeInTheDocument();
    expect(screen.getByText('Records evicted in window')).toBeInTheDocument();
    expect(
      screen.getByText('Oversized records dropped in window')
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        /Counts cover observation records lost in this source scope/
      )
    ).toBeInTheDocument();
  });
  test('unavailable accounting is shown explicitly instead of zero loss', async () => {
    api.getEventRuntime.mockResolvedValue(
      withWorkspaceView({
        ...snapshot,
        observation_loss: {
          ...snapshot.observation_loss,
          accounting_available: false,
          window_started_at_unix_ms: null,
          expired: 0,
          evicted: 0,
          oversize_dropped: 0
        }
      })
    );
    renderPage();
    fireEvent.click(screen.getByRole('tab', { name: 'Recent activity' }));
    expect(
      await screen.findByText(
        /No observation loss accounting window is available/
      )
    ).toBeInTheDocument();
    expect(
      screen.queryByText('Records expired in window')
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText('Loss accounting started')
    ).not.toBeInTheDocument();
  });
  test('reports inactive workflow subscriptions without interpreting consumer execution as consumption', async () => {
    api.getEventRuntime.mockResolvedValue(
      withWorkspaceView({
        ...snapshot,
        workflow_refresh_error: 'configuration_refresh_failed',
        workflow_subscriptions: [
          {
            workspace_id: 'workspace',
            application_id: 'needs-attention-app',
            subscription_id: 'inactive-subscription',
            activation: 'unavailable',
            reason: 'activation_failed',
            last_signal_id: 'hidden-signal',
            last_run_id: 'hidden-run',
            last_delivery: 'hidden-execution-result',
            last_delivery_reason: 'hidden-execution-reason'
          },
          {
            workspace_id: 'workspace',
            application_id: 'unknown-reason-app',
            subscription_id: 'unknown-reason-subscription',
            activation: 'stopping',
            reason: 'future_activation_reason',
            last_signal_id: null,
            last_run_id: null,
            last_delivery: null,
            last_delivery_reason: null
          },
          {
            workspace_id: 'workspace',
            application_id: 'active-app',
            subscription_id: 'active-subscription',
            activation: 'active',
            reason: null,
            last_signal_id: null,
            last_run_id: null,
            last_delivery: null,
            last_delivery_reason: null
          }
        ]
      })
    );
    renderPage();
    fireEvent.click(screen.getByRole('tab', { name: 'Current queues' }));
    expect(
      await screen.findByText('inactive-subscription')
    ).toBeInTheDocument();
    expect(screen.getByText('needs-attention-app')).toBeInTheDocument();
    expect(screen.getByText('Unavailable')).toBeInTheDocument();
    expect(
      screen.getByText(
        'Workflow subscriptions could not be refreshed. Retry with Refresh.'
      )
    ).toBeInTheDocument();
    expect(
      screen.getByText('Subscription activation failed.')
    ).toBeInTheDocument();
    expect(screen.queryByText('activation_failed')).not.toBeInTheDocument();
    expect(screen.queryByText('active-app')).not.toBeInTheDocument();
    expect(
      screen.getByText(
        'The subscription cannot currently accept new events. Check its application configuration and permissions.'
      )
    ).toBeInTheDocument();
    expect(
      screen.queryByText('future_activation_reason')
    ).not.toBeInTheDocument();
    for (const value of [
      'hidden-signal',
      'hidden-run',
      'hidden-execution-result',
      'hidden-execution-reason'
    ])
      expect(screen.queryByText(value)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'custom.order' }));
    expect(
      within(screen.getByRole('dialog')).getByText('Dequeued deliveries')
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: /^consume(?: event)?$/i })
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    fireEvent.click(screen.getByRole('tab', { name: 'Recent activity' }));
    expect(screen.queryByText('inactive-subscription')).not.toBeInTheDocument();
  });
  test('shows a missing schema as literal null without inventing an accept-all schema', async () => {
    api.listEventCatalog.mockResolvedValue([
      {
        ...custom,
        id: null,
        origin: 'core',
        name: 'Source without schema',
        payload_schema: null,
        supported_phases: []
      }
    ]);
    renderPage();
    fireEvent.click(
      await screen.findByRole('button', { name: 'Source without schema' })
    );
    const drawer = screen.getByRole('dialog');
    expect(
      await within(drawer).findByTestId('schema-json-editor')
    ).toHaveTextContent('null');
    expect(
      within(drawer).queryByText(
        'This source has not declared a payload schema.'
      )
    ).not.toBeInTheDocument();
    expect(
      within(drawer).queryByRole('button', { name: 'Save' })
    ).not.toBeInTheDocument();
    expect(
      within(drawer).queryByRole('button', { name: 'Add schema field' })
    ).not.toBeInTheDocument();
    expect(
      within(drawer).queryByText(/accepts every payload/)
    ).not.toBeInTheDocument();
    expect(
      within(drawer).getByRole('button', {
        name: 'Copy Payload schema JSON'
      })
    ).toBeInTheDocument();
    expect(api.createEventDefinition).not.toHaveBeenCalled();
    expect(api.updateEventDefinition).not.toHaveBeenCalled();
  });
  test('a catalog-only observer receives no create/edit or runtime actions', async () => {
    access.operations = new Set(['events.catalog.list']);
    renderPage();
    expect(
      await screen.findByRole('button', { name: 'Order' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Create event' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Edit' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: 'Current queues' })
    ).not.toBeInTheDocument();
  });
});

test('source scope keeps native identities and unknown original queue counts without mixing observation windows', async () => {
  const workspace = withWorkspaceView(snapshot);
  const systemView = {
    ...workspace.source_views[0],
    scope: { kind: 'system' as const },
    observation_bytes: 987,
    source_metadata_bytes: 123,
    observation_loss: {
      ...snapshot.observation_loss,
      expired: 777,
      evicted: 778
    },
    sources: [
      {
        registration_id: 'native-registration',
        queue_observation_complete: true,
        metadata_bytes: 123,
        descriptor: {
          source_id: 'native.source',
          point_id: 'native.point',
          owner_id: 'native.owner',
          contract_id: 'native.contract',
          name: 'Native source',
          description: '',
          payload_schema: null,
          origin: 'core' as const,
          scope: { kind: 'system' as const },
          binding: 'active' as const,
          coverage: 'live' as const,
          supported_phases: ['consumed' as const]
        },
        queues: [
          {
            queue_id: 'native-queue',
            subscriber_id: 'native-subscriber',
            consumer_ids: ['native-consumer'],
            buffered: null,
            in_flight: null,
            accepted: null,
            dequeued: 13,
            dropped: null,
            observed_at_unix_ms: 1000
          }
        ]
      }
    ],
    workflow_queues: [],
    activity: [
      {
        ...snapshot.activity[0],
        source_context: {
          registration_id: 'native-registration',
          source_id: 'native.source',
          point_id: 'native.point',
          scope: { kind: 'system' as const },
          original_event_id: 'native:event:non-uuid',
          producer_id: 'native-producer'
        },
        original_subscriber_id: 'native-subscriber',
        original_consumer_id: 'native-consumer'
      }
    ]
  };
  api.getEventRuntime.mockResolvedValue({
    ...workspace,
    source_views: [...workspace.source_views, systemView]
  });
  window.history.replaceState(
    {},
    '',
    '/settings/events?tab=queues&scope=system'
  );
  renderPage();
  expect(await screen.findByText('native-queue')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'native-queue' }));
  expect(
    within(screen.getByRole('dialog')).getAllByText('Unknown')
  ).toHaveLength(4);
  fireEvent.click(screen.getByRole('button', { name: 'Close' }));
  fireEvent.click(screen.getByRole('button', { name: 'Observation details' }));
  expect(screen.getByText('777')).toBeInTheDocument();
  expect(screen.queryByText('11')).not.toBeInTheDocument();
  expect(screen.getByText('987 / 2000')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Close' }));
  fireEvent.click(screen.getByRole('tab', { name: 'Recent activity' }));
  fireEvent.click(await screen.findByRole('button', { name: 'Details' }));
  expect(await screen.findByText('native:event:non-uuid')).toBeInTheDocument();
  expect(screen.getByText('native-producer')).toBeInTheDocument();
  expect(screen.getByText('native-subscriber')).toBeInTheDocument();
  expect(window.location.search).toContain('scope=system');
});

test('unbound original queues retain request errors without displaying adapter status', async () => {
  const workspace = withWorkspaceView(snapshot);
  api.getEventRuntime.mockResolvedValue({
    ...workspace,
    source_queue_refresh_error: 'source_queue_observer_unavailable',
    source_views: [
      {
        ...workspace.source_views[0],
        scope: { kind: 'unbound' },
        sources: [
          {
            registration_id: 'unbound-registration',
            queue_observation_complete: true,
            metadata_bytes: 0,
            queues: [],
            descriptor: {
              source_id: 'declaration',
              point_id: 'point',
              owner_id: 'owner',
              contract_id: 'contract',
              name: 'Unbound source',
              description: '',
              payload_schema: null,
              origin: 'core',
              scope: { kind: 'unbound' },
              binding: 'declaration',
              coverage: 'unavailable',
              supported_phases: []
            }
          }
        ]
      },
      ...workspace.source_views
    ]
  });
  window.history.replaceState(
    {},
    '',
    '/settings/events?tab=queues&scope=unbound'
  );
  renderPage();
  expect(
    await screen.findByRole('button', { name: 'Unbound source' })
  ).toBeInTheDocument();
  expect(screen.queryByText('Declaration only')).not.toBeInTheDocument();
  expect(
    screen.queryByRole('columnheader', { name: 'Observation coverage' })
  ).not.toBeInTheDocument();
  expect(
    screen.getByText(/Source queue observations could not be refreshed/)
  ).toBeInTheDocument();
  expect(
    screen.queryByText('source_queue_observer_unavailable')
  ).not.toBeInTheDocument();
});

test.each([
  { complete: false, observedQueue: false },
  { complete: false, observedQueue: true },
  { complete: true, observedQueue: false }
])(
  'reports queue observation completeness=$complete with observed queue=$observedQueue',
  async ({ complete, observedQueue }) => {
    const workspace = withWorkspaceView(snapshot);
    const source = {
      registration_id: 'budget-registration',
      queue_observation_complete: complete,
      metadata_bytes: 100,
      descriptor: {
        source_id: 'runtime.required',
        point_id: 'runtime.required',
        owner_id: 'runtime',
        contract_id: 'runtime.event',
        name: 'Budgeted runtime source',
        description: '',
        payload_schema: null,
        origin: 'core' as const,
        scope: { kind: 'workspace' as const, workspace_id: 'workspace' },
        binding: 'active' as const,
        coverage: 'live' as const,
        supported_phases: ['consumed' as const]
      },
      queues: observedQueue
        ? [
            {
              queue_id: 'observed-original-queue',
              subscriber_id: 'observed-original-subscriber',
              consumer_ids: [],
              buffered: null,
              in_flight: null,
              accepted: null,
              dequeued: 4,
              dropped: null,
              observed_at_unix_ms: 1000
            }
          ]
        : []
    };
    api.getEventRuntime.mockResolvedValue({
      ...workspace,
      source_views: [{ ...workspace.source_views[0], sources: [source] }]
    });
    window.history.replaceState({}, '', '/settings/events?tab=queues');
    renderPage();
    const sourceButton = await screen.findByRole('button', {
      name: 'Budgeted runtime source'
    });
    const sourceRow = sourceButton.closest('tr');
    expect(sourceRow).not.toBeNull();
    expect(
      within(sourceRow!).getByText(complete ? 'Complete' : 'Incomplete')
    ).toBeInTheDocument();
    const warning = screen.queryByText(
      /The observation budget prevented recording/
    );
    if (complete) {
      expect(warning).not.toBeInTheDocument();
    } else {
      expect(warning).toBeInTheDocument();
      expect(warning).toHaveTextContent(
        'an empty list does not establish that no queues exist'
      );
      expect(warning).toHaveTextContent(
        'This does not indicate a delivery failure in the original queues'
      );
    }
    if (observedQueue) {
      expect(screen.getByText('observed-original-queue')).toBeInTheDocument();
      fireEvent.click(
        screen.getByRole('button', { name: 'observed-original-queue' })
      );
      expect(
        within(screen.getByRole('dialog')).getAllByText('Unknown')
      ).toHaveLength(4);
      expect(
        within(screen.getByRole('dialog')).getByText('4')
      ).toBeInTheDocument();
      fireEvent.click(screen.getByRole('button', { name: 'Close' }));
      const queueRow = screen
        .getByText('observed-original-queue')
        .closest('tr');
      expect(queueRow).not.toBeNull();
      expect(within(queueRow!).getAllByText('Unknown')).toHaveLength(2);
    } else {
      expect(
        screen.queryByText('observed-original-queue')
      ).not.toBeInTheDocument();
    }
  }
);
