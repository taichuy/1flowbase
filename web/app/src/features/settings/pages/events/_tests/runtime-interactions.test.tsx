import '../../../../../test/fixtures/rc-util-unique-ids';
import {
  ApiClientError,
  type EventRuntimeSnapshot,
  type EventRegisteredSourceSnapshot
} from '@1flowbase/api-client';
import {
  focusManager,
  onlineManager,
  QueryClient,
  QueryClientProvider
} from '@tanstack/react-query';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { App as AntApp } from 'antd';
import { I18nextProvider } from 'react-i18next';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import { EventRuntimePanel } from '../EventRuntimePanel';

const api = vi.hoisted(() => ({ getEventRuntime: vi.fn() }));
const clipboard = vi.hoisted(() => ({ copy: vi.fn() }));
vi.mock('@1flowbase/charts', () => ({
  EChart: ({ ariaLabel }: { ariaLabel?: string }) => (
    <div role="img" aria-label={ariaLabel} />
  )
}));
vi.mock('@1flowbase/api-client', async (original) => ({
  ...(await original<typeof import('@1flowbase/api-client')>()),
  ...api
}));
vi.mock('../../../../../state/auth-store', () => ({
  useAuthStore: (selector: (s: unknown) => unknown) =>
    selector({ actor: { current_workspace_id: 'workspace' } })
}));
vi.mock('../../../../../state/console-operation-access', () => ({
  useConsoleOperationAccess: () => () => true
}));
vi.mock('../../../../../shared/ui/clipboard/use-clipboard-copy', () => ({
  useClipboardCopy: () => ({ copy: clipboard.copy, copied: false })
}));
const source = (id: string): EventRegisteredSourceSnapshot => ({
  registration_id: id,
  queue_observation_complete: true,
  metadata_bytes: 120,
  descriptor: {
    source_id: `source:${id}`,
    owner_id: 'owner',
    point_id: 'point',
    contract_id: 'invoice',
    name: `Source ${id}`,
    description: 'Native source',
    payload_schema: null,
    origin: 'core',
    scope: { kind: 'workspace', workspace_id: 'workspace' },
    binding: 'active',
    coverage: 'live',
    supported_phases: ['published', 'consumed']
  },
  queues: [
    {
      queue_id: `queue:${id}`,
      subscriber_id: `subscriber:${id}`,
      consumer_ids: [`consumer:${id}`],
      buffered: 7,
      in_flight: null,
      accepted: null,
      dequeued: 3,
      dropped: null,
      observed_at_unix_ms: Date.now()
    }
  ]
});
function snapshot(): EventRuntimeSnapshot {
  const scope = { kind: 'workspace' as const, workspace_id: 'workspace' };
  const data = {
    instance_id: 'host-instance',
    workspace_id: 'workspace',
    configured: true,
    source_queue_refresh_error: null,
    workflow_refresh_error: null,
    queues: [],
    workflow_subscriptions: [],
    admission_rejected: 0,
    source_metadata_bytes: 240,
    observation_bytes: 500,
    observation_max_bytes: 10000,
    observation_ttl_ms: 60000,
    observation_loss: {
      accounting_available: true,
      window_started_at_unix_ms: Date.now(),
      window_duration_ms: 120000,
      expired: 8,
      evicted: 0,
      oversize_dropped: 0
    }
  };
  const activity = ['published', 'consumed', 'subscribed'].map(
    (phase, index) => ({
      event_id: `event-${index}`,
      workspace_id: 'workspace',
      contract_id: `invoice-${index}`,
      phase: phase as 'published' | 'consumed' | 'subscribed',
      admission:
        index === 0
          ? ('reentrant' as const)
          : index === 1
            ? ('resource_exhausted' as const)
            : null,
      source_event_id: `source-event-${index}`,
      subscription_id: 'subscription',
      consumer_id: 'consumer',
      observed_at_unix_ms: Date.now() - index * 1000,
      original_subscriber_id: 'original-subscriber',
      original_consumer_id: 'original-consumer',
      source_context: {
        registration_id: 'one',
        scope,
        source_id: 'source:one',
        point_id: 'point',
        original_event_id: 'native-original',
        producer_id: 'producer',
        attributes: { terminal: 'completed', run_mode: 'event' }
      }
    })
  );
  return {
    ...data,
    activity,
    source_views: [
      {
        ...data,
        scope,
        sources: [source('one'), source('two')],
        workflow_queues: [],
        activity
      }
    ]
  };
}
function mount(tab: 'queues' | 'activity' = 'activity', suffix = '') {
  window.history.replaceState({}, '', `/settings/events?tab=${tab}${suffix}`);
  return render(
    <I18nextProvider i18n={appI18n}>
      <QueryClientProvider
        client={
          new QueryClient({ defaultOptions: { queries: { retry: false } } })
        }
      >
        <AntApp>
          <EventRuntimePanel tab={tab} />
        </AntApp>
      </QueryClientProvider>
    </I18nextProvider>
  );
}
beforeEach(async () => {
  vi.clearAllMocks();
  api.getEventRuntime.mockReset();
  window.localStorage.clear();
  await appI18n.changeLanguage('en_US');
  api.getEventRuntime.mockResolvedValue(snapshot());
  clipboard.copy.mockResolvedValue(undefined);
});
async function ready() {
  await waitFor(() =>
    expect(screen.getByRole('button', { name: 'Refresh' })).toBeEnabled()
  );
}

function openFirstActivity() {
  fireEvent.click(screen.getAllByRole('button', { name: 'Details' })[0]);
  return screen.getByRole('dialog');
}

describe('runtime user tasks and snapshot semantics', () => {
  test('defaults to decision fields and provides the complete source context through explicit details', async () => {
    mount();
    await ready();
    expect(
      screen.getAllByRole('columnheader').map((n) => n.textContent)
    ).toEqual([
      'Observed at',
      'Event',
      'Trigger phase',
      'Admission result',
      'Actions'
    ]);
    expect(screen.queryByText('native-original')).not.toBeInTheDocument();
    const drawer = openFirstActivity();
    for (const text of [
      'native-original',
      'original-subscriber',
      'original-consumer',
      'producer',
      'completed',
      'event'
    ])
      expect(within(drawer).getByText(text)).toBeInTheDocument();
    fireEvent.click(
      within(drawer).getByRole('button', { name: 'Copy Original event ID' })
    );
    await waitFor(() =>
      expect(clipboard.copy).toHaveBeenCalledWith('native-original')
    );
    expect(window.location.search).toContain('record=');
    expect(
      within(drawer).getByText(/does not indicate workflow success or failure/)
    ).toBeInTheDocument();
  });
  test('protective admission is neutral and resource rejection is a warning, never execution failure', async () => {
    mount();
    await ready();
    expect(
      screen.getByText('Workflow loop prevented').closest('.ant-tag')
    ).not.toHaveClass('ant-tag-error');
    expect(
      screen.getByText('Resource budget exhausted').closest('.ant-tag')
    ).toHaveClass('ant-tag-warning');
    expect(document.querySelector('.ant-tag-error')).toBeNull();
  });
  test('search only filters the fetched snapshot, distinguishes no results and restores rows', async () => {
    mount();
    await ready();
    fireEvent.change(
      screen.getByRole('searchbox', {
        name: 'Find event or identifier in this snapshot'
      }),
      { target: { value: 'event-1' } }
    );
    expect(screen.getAllByRole('button', { name: 'Details' })).toHaveLength(1);
    expect(
      screen.getByText(/Filters apply only to this fetched snapshot/)
    ).toBeInTheDocument();
    expect(api.getEventRuntime).toHaveBeenCalledTimes(1);
    fireEvent.change(
      screen.getByRole('searchbox', {
        name: 'Find event or identifier in this snapshot'
      }),
      { target: { value: 'missing' } }
    );
    expect(screen.getByText(/No matches in this snapshot/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Clear filters' }));
    expect(screen.getAllByRole('button', { name: 'Details' })).toHaveLength(3);
  });
  test('URL phase and admission filters survive remount and browser navigation without another search API', async () => {
    const view = mount(
      'activity',
      '&phase=consumed&admission=resource_exhausted'
    );
    await ready();
    expect(screen.getAllByRole('button', { name: 'Details' })).toHaveLength(1);
    view.unmount();
    mount('activity', '&phase=consumed&admission=resource_exhausted');
    await ready();
    expect(screen.getAllByRole('button', { name: 'Details' })).toHaveLength(1);
    await act(async () => {
      window.history.pushState({}, '', '?tab=activity&phase=published');
      window.dispatchEvent(new PopStateEvent('popstate'));
    });
    expect(screen.getByText('Workflow loop prevented')).toBeInTheDocument();
    expect(
      screen.queryByText('Resource budget exhausted')
    ).not.toBeInTheDocument();
  });
  test('queue statistics do not expose a record diagnostics entry or raw inspector', async () => {
    mount('queues');
    await ready();
    expect(
      screen.queryByRole('button', { name: 'Record diagnostics' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'More' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Source one' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'queue:one' })
    ).not.toBeInTheDocument();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  test('native-only source snapshots never appear as event-center subscriptions', async () => {
    const data = snapshot();
    data.source_views[0].sources = [
      { ...source('one'), queues: [] },
      source('two')
    ];
    api.getEventRuntime.mockResolvedValue(data);
    mount('queues');
    await ready();
    expect(
      screen.getByRole('columnheader', { name: 'Event' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'invoice' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('navigation', { name: 'Registered sources' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('queue:two')).not.toBeInTheDocument();
    expect(screen.queryByText('Source two')).not.toBeInTheDocument();
  });
  test('workspace event subscriptions include custom-event queues absent from the source-specific projection', async () => {
    const data = snapshot();
    data.queues = [
      {
        subscription_id: 'custom-subscription',
        workspace_id: 'workspace',
        contract_id: 'custom.created',
        trigger_phase: 'published',
        consumer_ids: ['custom-worker'],
        closed: false,
        buffered: 4,
        in_flight: 1,
        accepted: 6,
        consumed: 2,
        admission_rejected: 0
      }
    ];
    // The source projection is intentionally empty, but workspace snapshot.queues
    // must still make this custom-event subscription visible.
    data.source_views[0].workflow_queues = [];
    data.workflow_subscriptions = [
      {
        workspace_id: 'workspace',
        application_id: 'custom-workflow',
        subscription_id: 'custom-subscription',
        contract_id: 'custom.created',
        trigger_phase: 'published',
        source: null,
        activation: 'active',
        reason: null,
        last_signal_id: null,
        last_run_id: null,
        last_delivery: null,
        last_delivery_reason: null
      },
      {
        workspace_id: 'workspace',
        application_id: 'unavailable-workflow',
        subscription_id: 'no-queue-subscription',
        contract_id: 'custom.created',
        trigger_phase: 'published',
        source: null,
        activation: 'unavailable',
        reason: 'activation_failed',
        last_signal_id: null,
        last_run_id: null,
        last_delivery: null,
        last_delivery_reason: null
      }
    ];
    api.getEventRuntime.mockResolvedValue(data);
    mount('queues');
    await ready();
    const eventButton = screen.getByRole('button', { name: 'custom.created' });
    expect(eventButton).toBeInTheDocument();
    const eventRow = eventButton.closest('tr');
    expect(within(eventRow!).getByText('2')).toBeInTheDocument();
    expect(within(eventRow!).getByText('Unknown')).toBeInTheDocument();
    fireEvent.click(eventButton);
    const eventDrawer = screen.getByRole('dialog');
    expect(
      within(eventDrawer).getByText('custom-workflow')
    ).toBeInTheDocument();
    expect(
      within(eventDrawer).getByText('unavailable-workflow')
    ).toBeInTheDocument();
    const unavailableRow = within(eventDrawer)
      .getByText('unavailable-workflow')
      .closest('tr');
    fireEvent.click(
      within(unavailableRow!).getByRole('button', { name: 'Details' })
    );
    const details = screen.getByRole('dialog');
    expect(
      within(details).getByText('Subscription activation failed.')
    ).toBeInTheDocument();
    expect(
      within(details).getByText(/Runtime queue was not observed/)
    ).toBeInTheDocument();
    expect(window.location.search).toContain(
      'subscription=no-queue-subscription'
    );
  });

  test('system-scoped workflow subscriptions remain separate from the workspace queue projection', async () => {
    const data = snapshot();
    const systemScope = { kind: 'system' as const };
    data.source_views.push({
      ...data.source_views[0],
      scope: systemScope,
      sources: [],
      activity: [],
      workflow_queues: [
        {
          subscription_id: 'system-subscription',
          workspace_id: 'workspace',
          contract_id: 'system.changed',
          trigger_phase: 'published',
          source_target: {
            source_id: 'system.source',
            scope: systemScope,
            filters: {},
            workflow_id: 'system-app'
          },
          consumer_ids: ['system-app'],
          closed: false,
          buffered: 3,
          in_flight: 0,
          accepted: 4,
          consumed: 1,
          admission_rejected: 0
        }
      ]
    });
    data.workflow_subscriptions = [
      {
        workspace_id: 'workspace',
        application_id: 'system-app',
        subscription_id: 'system-subscription',
        contract_id: 'system.changed',
        trigger_phase: 'published',
        source: { source_id: 'system.source', scope: systemScope },
        activation: 'active',
        reason: null,
        last_signal_id: null,
        last_run_id: null,
        last_delivery: null,
        last_delivery_reason: null
      }
    ];
    api.getEventRuntime.mockResolvedValue(data);
    mount('queues', '&scope=system');
    await ready();
    expect(
      screen.getByRole('button', { name: 'system.changed' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'custom.created' })
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'system.changed' }));
    expect(
      within(screen.getByRole('dialog')).getByText('system-app')
    ).toBeInTheDocument();
    expect(window.location.search).toContain('scope=system');
  });
  test('an incomplete native-only queue snapshot does not fabricate an event subscription', async () => {
    const data = snapshot();
    data.source_views[0].sources = [
      { ...source('one'), queues: [], queue_observation_complete: false }
    ];
    api.getEventRuntime.mockResolvedValue(data);
    mount('queues');
    await ready();
    expect(
      screen.queryByRole('button', { name: 'Source one' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('queue:one')).not.toBeInTheDocument();
    expect(screen.queryByText(/Source diagnostics/)).not.toBeInTheDocument();
  });
  test('a refresh reports progress, marks retained data on failure and recovers with retry', async () => {
    mount();
    await ready();
    let reject!: (e: Error) => void;
    api.getEventRuntime.mockImplementationOnce(
      () =>
        new Promise((_, no) => {
          reject = no;
        })
    );
    fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
    await waitFor(() =>
      expect(document.querySelector('.event-runtime')).toHaveAttribute(
        'aria-busy',
        'true'
      )
    );
    await act(async () => reject(new Error('offline')));
    expect(
      await screen.findByText(/Refresh failed. Showing the previous snapshot/)
    ).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: 'Details' })).toHaveLength(3);
    api.getEventRuntime.mockResolvedValue(snapshot());
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    await waitFor(() =>
      expect(
        screen.queryByText(/Refresh failed. Showing the previous snapshot/)
      ).not.toBeInTheDocument()
    );
  });
  test('403 after a successful snapshot removes privileged cached records', async () => {
    mount();
    await ready();
    api.getEventRuntime.mockRejectedValue(
      new ApiClientError({ status: 403, message: 'forbidden' })
    );
    fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
    expect(
      await screen.findByText(
        'You do not have permission to view this runtime snapshot.'
      )
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Details' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText('Workflow loop prevented')
    ).not.toBeInTheDocument();
  });
  test('a legacy scope parameter does not hide observations or restore the removed scope filter', async () => {
    mount('activity', '&scope=system');
    await ready();
    expect(screen.getByText('Workflow loop prevented')).toBeInTheDocument();
    expect(
      screen.queryByRole('combobox', { name: 'Scope' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText(/The requested scope is not available/)
    ).not.toBeInTheDocument();
  });
  test('missing cleanup counts are shown in the report without opening a diagnostics drawer', async () => {
    const data = snapshot();
    data.source_views[0].observation_loss.accounting_available = false;
    api.getEventRuntime.mockResolvedValue(data);
    mount('activity', '&observation=1');
    await ready();
    const metric = screen
      .getByRole('button', { name: 'Expired records' })
      .closest<HTMLElement>('.event-observation-metric')!;
    expect(within(metric).getByText('—')).toBeInTheDocument();
    expect(
      within(metric).getByText('No cleanup statistics')
    ).toBeInTheDocument();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'More' })
    ).not.toBeInTheDocument();
  });
  test('available cleanup counts remain visible when other scopes have no accounting', async () => {
    const data = snapshot();
    data.source_views[0].observation_loss.expired = 78;
    data.source_views.push({
      ...data.source_views[0],
      scope: { kind: 'system' },
      sources: [],
      activity: [],
      observation_loss: {
        ...data.source_views[0].observation_loss,
        accounting_available: false,
        expired: 0
      }
    });
    api.getEventRuntime.mockResolvedValue(data);
    mount('activity');
    await ready();
    const metric = screen
      .getByRole('button', { name: 'Expired records' })
      .closest<HTMLElement>('.event-observation-metric')!;
    expect(within(metric).getByText('78')).toBeInTheDocument();
    expect(
      within(metric).getByText('Partial scope coverage')
    ).toBeInTheDocument();
  });
  test('a disappeared record URL explains the missing context instead of silently closing', async () => {
    mount('activity', '&record=missing');
    await ready();
    expect(screen.getByRole('dialog')).toHaveTextContent(
      'This record is not in the latest snapshot.'
    );
  });
});

describe('continuous runtime observation', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    focusManager.setFocused(true);
    onlineManager.setOnline(true);
  });
  afterEach(() => {
    focusManager.setFocused(undefined);
    onlineManager.setOnline(true);
    vi.useRealTimers();
  });

  async function advance(milliseconds = 20) {
    await act(async () => {
      await vi.advanceTimersByTimeAsync(milliseconds);
    });
  }
  function observation(buffered: number) {
    const data = snapshot();
    data.queues = [
      {
        subscription_id: 'observed-subscription',
        workspace_id: 'workspace',
        contract_id: 'observed.event',
        trigger_phase: 'published',
        consumer_ids: ['worker'],
        closed: false,
        buffered,
        in_flight: 0,
        accepted: buffered,
        consumed: 0,
        admission_rejected: 0
      }
    ];
    data.source_views[0].activity = [
      {
        ...data.activity[0],
        source_context: undefined,
        contract_id: `revision-${buffered}`
      }
    ];
    return data;
  }

  test.each(['queues', 'activity'] as const)(
    '%s displays new snapshots without a manual refresh and stops when unmounted',
    async (tab) => {
      api.getEventRuntime.mockResolvedValueOnce(observation(2));
      const view = mount(tab);
      await advance();
      if (tab === 'activity')
        expect(
          screen.getByRole('button', { name: 'revision-2' })
        ).toBeInTheDocument();
      else
        expect(
          within(
            screen
              .getByRole('button', { name: 'observed.event' })
              .closest('tr')!
          ).getByText('2')
        ).toBeInTheDocument();

      api.getEventRuntime.mockResolvedValue(observation(9));
      await advance(5050);
      if (tab === 'activity') {
        expect(
          screen.getByRole('button', { name: 'revision-9' })
        ).toBeInTheDocument();
        expect(
          screen.queryByRole('button', { name: 'revision-2' })
        ).not.toBeInTheDocument();
      } else {
        expect(
          within(
            screen
              .getByRole('button', { name: 'observed.event' })
              .closest('tr')!
          ).getByText('9')
        ).toBeInTheDocument();
      }
      view.unmount();
      const callsAtUnmount = api.getEventRuntime.mock.calls.length;
      await advance(15000);
      expect(api.getEventRuntime).toHaveBeenCalledTimes(callsAtUnmount);
    }
  );

  test('pauses in the background and refreshes immediately on return', async () => {
    mount();
    await advance();
    focusManager.setFocused(false);
    api.getEventRuntime.mockResolvedValue(observation(9));
    const callsBeforePause = api.getEventRuntime.mock.calls.length;
    await advance(15000);
    expect(api.getEventRuntime).toHaveBeenCalledTimes(callsBeforePause);
    await act(async () => focusManager.setFocused(true));
    await advance();
    expect(
      screen.getByRole('button', { name: 'revision-9' })
    ).toBeInTheDocument();
  });

  test('retains data on a transient error and recovers on the next automatic refresh', async () => {
    api.getEventRuntime.mockResolvedValueOnce(observation(2));
    mount();
    await advance();
    api.getEventRuntime.mockRejectedValueOnce(
      new Error('temporarily unavailable')
    );
    await advance(5050);
    expect(
      screen.getByText(/Refresh failed. Showing the previous snapshot/)
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'revision-2' })
    ).toBeInTheDocument();
    api.getEventRuntime.mockResolvedValue(observation(9));
    await advance(5050);
    expect(
      screen.queryByText(/Refresh failed. Showing the previous snapshot/)
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'revision-9' })
    ).toBeInTheDocument();
  });

  test('resumes automatically when the network reconnects', async () => {
    mount();
    await advance();
    onlineManager.setOnline(false);
    api.getEventRuntime.mockResolvedValue(observation(9));
    const callsBeforeOffline = api.getEventRuntime.mock.calls.length;
    await advance(5050);
    expect(api.getEventRuntime).toHaveBeenCalledTimes(callsBeforeOffline);
    expect(screen.getByText(/Waiting for a connection/)).toBeInTheDocument();
    await act(async () => onlineManager.setOnline(true));
    await advance();
    expect(
      screen.getByRole('button', { name: 'revision-9' })
    ).toBeInTheDocument();
  });

  test.each([401, 403])(
    'stops polling and hides cached records after HTTP %s',
    async (status) => {
      mount();
      await advance();
      api.getEventRuntime.mockRejectedValue(
        new ApiClientError({ status, message: 'denied' })
      );
      await advance(5050);
      expect(
        screen.getByText(
          'You do not have permission to view this runtime snapshot.'
        )
      ).toBeInTheDocument();
      expect(
        screen.queryByText('Workflow loop prevented')
      ).not.toBeInTheDocument();
      const callsAfterDenial = api.getEventRuntime.mock.calls.length;
      await advance(15000);
      expect(api.getEventRuntime).toHaveBeenCalledTimes(callsAfterDenial);
    }
  );

  test('does not overlap a slow automatic request with another refresh', async () => {
    mount();
    await advance();
    let finish!: (value: EventRuntimeSnapshot) => void;
    api.getEventRuntime.mockImplementationOnce(
      () =>
        new Promise<EventRuntimeSnapshot>((resolve) => {
          finish = resolve;
        })
    );
    await advance(5050);
    const callsInFlight = api.getEventRuntime.mock.calls.length;
    await advance(10000);
    fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
    expect(api.getEventRuntime).toHaveBeenCalledTimes(callsInFlight);
    await act(async () => finish(observation(9)));
    await advance();
    expect(
      screen.getByRole('button', { name: 'revision-9' })
    ).toBeInTheDocument();
  });
});

test('closing a URL-backed detail returns keyboard focus to its trigger', async () => {
  mount();
  await ready();
  const trigger = screen.getAllByRole('button', { name: 'Details' })[0];
  trigger.focus();
  fireEvent.click(trigger);
  expect(screen.getByRole('dialog')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Close' }));
  await waitFor(() => expect(trigger).toHaveFocus());
  expect(new URLSearchParams(window.location.search).has('record')).toBe(false);
});
