import '../../../../../test/fixtures/rc-util-unique-ids';
import type {
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
  getEventRuntime: vi.fn()
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
  enabled: false,
  owner_id: 'workspace',
  contract_id: 'custom.order',
  contract_version: '1',
  name: 'Order',
  description: 'New order',
  payload_schema: schema,
  origin: 'custom',
  supported_phases: ['subscribed', 'published', 'consumed']
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
      contract_version: '1',
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
      contract_version: '1',
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
    'events.definitions.create',
    'events.definitions.update',
    'events.runtime.get'
  ]);
  api.listEventCatalog.mockResolvedValue([custom]);
  api.getEventRuntime.mockResolvedValue(withWorkspaceView(snapshot));
  api.createEventDefinition.mockResolvedValue({ id: 'new' });
  api.updateEventDefinition.mockResolvedValue({ id: 'definition' });
});

describe('event center configuration and observations', () => {
  test.each([schema, {}, true, false, null])(
    'previews and copies the complete read-only schema %j',
    async (payload_schema) => {
      api.listEventCatalog.mockResolvedValue([
        { ...custom, origin: 'core', id: null, payload_schema }
      ]);
      renderPage();
      fireEvent.click(await screen.findByRole('button', { name: 'Details' }));
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
      { ...custom, origin: 'core', id: null }
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
    expect(await screen.findByText('host-instance')).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: label })).toHaveAttribute(
      'aria-selected',
      'true'
    );
    expect(api.listEventCatalog).not.toHaveBeenCalled();
    view.unmount();
    renderPage();
    expect(await screen.findByText('host-instance')).toBeInTheDocument();
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
    fireEvent.click(within(fields).getByRole('checkbox', { name: 'Owner' }));
    await waitFor(() =>
      expect(
        screen.queryByRole('columnheader', { name: 'Owner' })
      ).not.toBeInTheDocument()
    );
    view.unmount();
    renderPage();
    await screen.findByRole('button', { name: 'Order' });
    expect(
      screen.queryByRole('columnheader', { name: 'Owner' })
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: 'Contract ID' })
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
  test('creates a definition using the structured schema editor and retains disabled configuration', async () => {
    renderPage();
    fireEvent.click(
      await screen.findByRole('button', { name: 'Create event' })
    );
    const drawer = screen.getByRole('dialog');
    fireEvent.change(within(drawer).getByLabelText('Name'), {
      target: { value: 'Request' }
    });
    fireEvent.change(within(drawer).getByLabelText('Contract ID'), {
      target: { value: 'custom.request' }
    });
    fireEvent.click(within(drawer).getByRole('switch', { name: 'Enabled' }));
    fireEvent.click(
      within(drawer).getByRole('button', { name: 'Add schema field' })
    );
    fireEvent.change(within(drawer).getByLabelText('Schema field name 1'), {
      target: { value: 'message' }
    });
    fireEvent.click(within(drawer).getByRole('button', { name: 'Save' }));
    await waitFor(() =>
      expect(api.createEventDefinition).toHaveBeenCalledWith(
        expect.objectContaining({
          contract_id: 'custom.request',
          contract_version: '1',
          name: 'Request',
          enabled: false,
          payload_schema: expect.objectContaining({
            type: 'object',
            properties: expect.objectContaining({ message: expect.any(Object) })
          })
        }),
        'csrf'
      )
    );
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
  test('editing a disabled custom definition preserves nested schema and immutable identity', async () => {
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Edit' }));
    const drawer = screen.getByRole('dialog');
    expect(within(drawer).getByLabelText('Contract ID')).toBeDisabled();
    expect(within(drawer).getByLabelText('Contract version')).toBeDisabled();
    fireEvent.change(within(drawer).getByLabelText('Name'), {
      target: { value: 'Changed order' }
    });
    fireEvent.click(within(drawer).getByRole('button', { name: 'Save' }));
    await waitFor(() =>
      expect(api.updateEventDefinition).toHaveBeenCalledWith(
        'definition',
        {
          name: 'Changed order',
          description: 'New order',
          enabled: false,
          payload_schema: schema
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
    expect(within(drawer).getByText('workspace')).toBeInTheDocument();
    expect(api.updateEventDefinition).not.toHaveBeenCalled();
  });
  test('shows queue facts separately from the activity window and exposes rejected admission without success metrics', async () => {
    renderPage();
    fireEvent.click(screen.getByRole('tab', { name: 'Current queues' }));
    expect(await screen.findByText('host-instance')).toBeInTheDocument();
    expect(screen.getByText('Dequeued deliveries')).toBeInTheDocument();
    expect(
      screen.getByText(/does not indicate workflow success or failure/)
    ).toBeInTheDocument();
    expect(
      screen.getByText('Rejections retained in activity window')
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: /consume/i })
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('tab', { name: 'Recent activity' }));
    expect(
      await screen.findByText('Resource budget exhausted')
    ).toBeInTheDocument();
    expect(screen.getByText('source-event')).toBeInTheDocument();
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
    expect(screen.getByText('Dequeued deliveries')).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: /consume/i })
    ).not.toBeInTheDocument();
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
        metadata_bytes: 123,
        descriptor: {
          source_id: 'native.source',
          point_id: 'native.point',
          owner_id: 'native.owner',
          contract_id: 'native.contract',
          contract_version: '1',
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
  expect(screen.getAllByText('Unknown')).toHaveLength(4);
  expect(screen.getByText('777')).toBeInTheDocument();
  expect(screen.queryByText('11')).not.toBeInTheDocument();
  expect(screen.getByText('987 / 2000')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('tab', { name: 'Recent activity' }));
  expect(await screen.findByText('native:event:non-uuid')).toBeInTheDocument();
  expect(screen.getByText('native-producer')).toBeInTheDocument();
  expect(screen.getByText('native-subscriber')).toBeInTheDocument();
  expect(window.location.search).toContain('scope=system');
});

test('unbound declarations remain visibly unavailable even without original queues', async () => {
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
            metadata_bytes: 0,
            queues: [],
            descriptor: {
              source_id: 'declaration',
              point_id: 'point',
              owner_id: 'owner',
              contract_id: 'contract',
              contract_version: '1',
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
  expect(screen.getAllByText('Declaration only').length).toBeGreaterThan(0);
  expect(screen.getAllByText('Unavailable').length).toBeGreaterThan(0);
  expect(
    screen.getByText(/Source queue observations could not be refreshed/)
  ).toBeInTheDocument();
  expect(
    screen.queryByText('source_queue_observer_unavailable')
  ).not.toBeInTheDocument();
});
