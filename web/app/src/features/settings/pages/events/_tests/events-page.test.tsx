import type {
  EventCatalogEntry,
  EventRuntimeSnapshot
} from '@1flowbase/api-client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
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
