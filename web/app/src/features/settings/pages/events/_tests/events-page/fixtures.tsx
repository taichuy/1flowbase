import '../../../../../../test/fixtures/rc-util-unique-ids';
import type {
  ApiEventOperationDetails,
  EventCatalogEntry,
  EventRuntimeSnapshot
} from '@1flowbase/api-client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render } from '@testing-library/react';
import { I18nextProvider } from 'react-i18next';
import { App as AntApp } from 'antd';
import { beforeEach, vi } from 'vitest';
import { appI18n } from '../../../../../../shared/i18n/app-i18n';
import { EventsPage } from '../../EventsPage';

const api = vi.hoisted(() => ({
  listEventCatalog: vi.fn(),
  createEventDefinition: vi.fn(),
  updateEventDefinition: vi.fn(),
  getEventRuntime: vi.fn(),
  listApiEventOperations: vi.fn(),
  getApiEventOperation: vi.fn()
}));
vi.mock('@1flowbase/charts', () => ({
  EChart: ({
    ariaLabel,
    className,
    option
  }: {
    ariaLabel?: string;
    className?: string;
    option: unknown;
  }) => (
    <img
      alt={ariaLabel}
      className={className}
      data-chart-option={JSON.stringify(option)}
    />
  )
}));
const clipboard = vi.hoisted(() => ({ copy: vi.fn() }));
vi.mock('../../../../../../shared/ui/clipboard/use-clipboard-copy', () => ({
  useClipboardCopy: () => ({ copied: false, copy: clipboard.copy })
}));
vi.mock('../../../../../../shared/code-block/monaco-runtime', () => ({
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
vi.mock('../../../../../../state/auth-store', () => ({
  useAuthStore: () => 'csrf'
}));
vi.mock('../../../../../../state/console-operation-access', () => ({
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
  event_kind: 'interface',
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
function renderPage(
  client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } }
  })
) {
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

export {
  access,
  api,
  clipboard,
  custom,
  declarations,
  documented,
  renderPage,
  schema,
  snapshot,
  withWorkspaceView
};
