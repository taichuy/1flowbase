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
import { beforeEach, describe, expect, test, vi } from 'vitest';
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import { EventsPage } from '../EventsPage';

const api = vi.hoisted(() => ({
  listEventCatalog: vi.fn(),
  createEventDefinition: vi.fn(),
  updateEventDefinition: vi.fn(),
  getEventRuntime: vi.fn()
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
function renderPage() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } }
  });
  return render(
    <I18nextProvider i18n={appI18n}>
      <QueryClientProvider client={client}>
        <EventsPage />
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
  api.getEventRuntime.mockResolvedValue(snapshot);
  api.createEventDefinition.mockResolvedValue({ id: 'new' });
  api.updateEventDefinition.mockResolvedValue({ id: 'definition' });
});

describe('event center configuration and observations', () => {
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
  test('editing a disabled custom definition preserves nested schema and immutable identity', async () => {
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Order' }));
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
    api.getEventRuntime.mockResolvedValue({
      ...snapshot,
      configured: false,
      queues: [],
      activity: []
    });
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
        /Counts cover observation records lost in this workspace/
      )
    ).toBeInTheDocument();
  });
  test('unavailable accounting is shown explicitly instead of zero loss', async () => {
    api.getEventRuntime.mockResolvedValue({
      ...snapshot,
      observation_loss: {
        ...snapshot.observation_loss,
        accounting_available: false,
        window_started_at_unix_ms: null,
        expired: 0,
        evicted: 0,
        oversize_dropped: 0
      }
    });
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
    api.getEventRuntime.mockResolvedValue({
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
    });
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
  test('keeps a source with an undeclared schema read-only without inventing an accept-all schema', async () => {
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
      within(drawer).getByText('This source has not declared a payload schema.')
    ).toBeInTheDocument();
    expect(
      within(drawer).queryByRole('button', { name: 'Save' })
    ).not.toBeInTheDocument();
    expect(
      within(drawer).queryByRole('button', { name: 'Add schema field' })
    ).not.toBeInTheDocument();
    expect(
      within(drawer).queryByText(/accepts every payload/)
    ).not.toBeInTheDocument();
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
