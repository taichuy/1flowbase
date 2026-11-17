import {
  access,
  api,
  custom,
  renderPage,
  snapshot,
  withWorkspaceView
} from './fixtures';
import { QueryClient } from '@tanstack/react-query';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { I18nextProvider } from 'react-i18next';
import { ConfigProvider } from 'antd';
import { describe, expect, test } from 'vitest';
import { appI18n } from '../../../../../../shared/i18n/app-i18n';
import { ObservationStatistics } from '../../runtime/statistics/ObservationStatistics';

describe('event center configuration and observations', () => {

  test.each([
    ['queues', 'Current queues'],
    ['activity', 'Recent activity']
  ])('opens and restores the %s tab from its URL', async (tab, label) => {
    window.history.replaceState({}, '', `/settings/events?tab=${tab}`);
    const view = renderPage();
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Refresh' })).toBeEnabled()
    );
    expect(screen.getByRole('tab', { name: label })).toHaveAttribute(
      'aria-selected',
      'true'
    );
    expect(api.listEventCatalog).not.toHaveBeenCalled();
    view.unmount();
    renderPage();
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Refresh' })).toBeEnabled()
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
    window.history.back();
    await waitFor(() => expect(window.location.search).toBe('?tab=queues'));
    expect(screen.getByRole('tab', { name: 'Current queues' })).toHaveAttribute(
      'aria-selected',
      'true'
    );
    window.history.forward();
    await waitFor(() => expect(window.location.search).toBe('?tab=activity'));
    expect(
      screen.getByRole('tab', { name: 'Recent activity' })
    ).toHaveAttribute('aria-selected', 'true');
    fireEvent.click(screen.getByRole('tab', { name: 'Event catalog' }));
    expect(window.location.search).toBe('?tab=catalog');
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
  test('shows queue facts separately from the activity window and exposes rejected admission without success metrics', async () => {
    renderPage();
    fireEvent.click(screen.getByRole('tab', { name: 'Current queues' }));
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Refresh' })).toBeEnabled()
    );
    fireEvent.click(screen.getByRole('button', { name: 'custom.order' }));
    expect(
      within(screen.getByRole('dialog')).getByText(
        /An event can fan out to independent subscriptions/
      )
    ).toBeInTheDocument();
    fireEvent.click(
      within(screen.getByRole('dialog')).getByRole('button', {
        name: 'Details'
      })
    );
    expect(
      within(screen.getByRole('dialog')).getByText('Dequeued deliveries')
    ).toBeInTheDocument();
    expect(
      screen.getByText(/does not indicate workflow success or failure/)
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
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
  test('exposes the previous raw diagnostics as readable metrics and a cleanup report', async () => {
    renderPage();
    fireEvent.click(screen.getByRole('tab', { name: 'Recent activity' }));
    const expired = await screen.findByRole('button', {
      name: 'Expired records'
    });
    expect(
      within(
        expired.closest<HTMLElement>('.event-observation-metric')!
      ).getByText('11')
    ).toBeInTheDocument();
    expect(
      within(
        screen
          .getByRole('button', { name: 'Evicted for space' })
          .closest<HTMLElement>('.event-observation-metric')!
      ).getByText('8')
    ).toBeInTheDocument();
    expect(
      within(
        screen
          .getByRole('button', { name: 'Oversized records dropped' })
          .closest<HTMLElement>('.event-observation-metric')!
      ).getByText('6')
    ).toBeInTheDocument();
    expect(
      screen.getByRole('img', { name: 'Record cleanup by scope' })
    ).toBeInTheDocument();
    expect(screen.getByText('Retained for 1 min')).toBeInTheDocument();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Record diagnostics' })
    ).not.toBeInTheDocument();
  });
  test('unavailable cleanup statistics remain unknown in the report', async () => {
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
    const expired = await screen.findByRole('button', {
      name: 'Expired records'
    });
    const metric = expired.closest<HTMLElement>('.event-observation-metric')!;
    expect(within(metric).getByText('—')).toBeInTheDocument();
    expect(
      within(metric).getByText('No cleanup statistics')
    ).toBeInTheDocument();
    expect(within(metric).queryByText('0')).not.toBeInTheDocument();
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
            contract_id: 'custom.order',
            trigger_phase: 'published',
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
            contract_id: 'custom.order',
            trigger_phase: 'published',
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
            contract_id: 'custom.order',
            trigger_phase: 'published',
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
    fireEvent.click(
      await screen.findByRole('button', { name: 'custom.order' })
    );
    expect(screen.getByText('needs-attention-app')).toBeInTheDocument();
    expect(screen.getByText('Unavailable')).toBeInTheDocument();
    expect(
      screen.getByText(
        'Workflow subscriptions could not be refreshed. Retry with Refresh.'
      )
    ).toBeInTheDocument();
    const inactiveRow = screen.getByText('needs-attention-app').closest('tr');
    fireEvent.click(
      within(inactiveRow!).getByRole('button', { name: 'Details' })
    );
    expect(
      screen.getByText('Subscription activation failed.')
    ).toBeInTheDocument();
    expect(screen.queryByText('activation_failed')).not.toBeInTheDocument();
    for (const value of [
      'hidden-signal',
      'hidden-run',
      'hidden-execution-result',
      'hidden-execution-reason'
    ])
      expect(screen.queryByText(value)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    fireEvent.click(
      within(screen.getByRole('dialog')).getAllByRole('button', {
        name: 'Details'
      })[0]
    );
    expect(screen.getByText('Dequeued deliveries')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(
      screen.queryByRole('button', { name: /^consume(?: event)?$/i })
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('tab', { name: 'Recent activity' }));
    expect(screen.queryByText('inactive-subscription')).not.toBeInTheDocument();
  });
});

test('source scope retains activity identities and separate observation windows without native queue navigation', async () => {
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
        event_id: 'native-event',
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
  await waitFor(() =>
    expect(screen.getByRole('button', { name: 'Refresh' })).toBeEnabled()
  );
  expect(
    screen.queryByRole('button', { name: 'native.contract' })
  ).not.toBeInTheDocument();
  expect(screen.queryByText('native-queue')).not.toBeInTheDocument();
  expect(
    screen.queryByRole('button', { name: 'Record diagnostics' })
  ).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('tab', { name: 'Recent activity' }));
  fireEvent.click(await screen.findByRole('button', { name: 'Native source' }));
  expect(await screen.findByText('native:event:non-uuid')).toBeInTheDocument();
  expect(screen.getByText('native-producer')).toBeInTheDocument();
  expect(screen.getByText('native-subscriber')).toBeInTheDocument();
  expect(window.location.search).toContain('scope=system');
});

test('unbound source metadata and native queue errors do not enter the subscription UI', async () => {
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
  await waitFor(() =>
    expect(screen.getByRole('button', { name: 'Refresh' })).toBeEnabled()
  );
  expect(screen.queryByText('Unbound source')).not.toBeInTheDocument();
  expect(screen.queryByText(/Source diagnostics/)).not.toBeInTheDocument();
  expect(
    screen.queryByText(/Source queue observations could not be refreshed/)
  ).not.toBeInTheDocument();
  expect(
    screen.queryByText('source_queue_observer_unavailable')
  ).not.toBeInTheDocument();
});

test.each([
  { complete: false, observedQueue: false },
  { complete: false, observedQueue: true },
  { complete: true, observedQueue: false }
])(
  'native source completeness=$complete and queued=$observedQueue do not generate subscriptions',
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
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Refresh' })).toBeEnabled()
    );
    expect(
      screen.queryByRole('button', { name: 'Budgeted runtime source' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText('observed-original-queue')
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'runtime.event' })
    ).not.toBeInTheDocument();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(screen.queryByText(/Source diagnostics/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Source metadata bytes/)).not.toBeInTheDocument();
  }
);

function observationTrend() {
  const chart = document.querySelector('.event-observation-trend-chart');
  expect(chart).not.toBeNull();
  return JSON.parse(chart!.getAttribute('data-chart-option')!) as {
    color: string[];
    series: Array<{ name: string; data: number[] }>;
  };
}

test('observation visit history survives locale and theme changes without extra samples', async () => {
  const views = withWorkspaceView({
    ...snapshot,
    observation_bytes: 2048,
    source_metadata_bytes: 512
  }).source_views;
  const nextViews = withWorkspaceView({
    ...snapshot,
    observation_bytes: 4096,
    source_metadata_bytes: 1024
  }).source_views;
  const content = (
    currentViews: typeof views,
    capturedAt: number,
    color: string
  ) => (
    <I18nextProvider i18n={appI18n}>
      <ConfigProvider theme={{ token: { colorPrimary: color } }}>
        <ObservationStatistics
          views={currentViews}
          capturedAt={capturedAt}
          admissionRejected={3}
        />
      </ConfigProvider>
    </I18nextProvider>
  );
  const view = render(content(views, 1000, '#112233'));
  view.rerender(content(nextViews, 6000, '#112233'));
  await waitFor(() =>
    expect(observationTrend().series.map((series) => series.data)).toEqual([
      [1.5, 3],
      [0.5, 1]
    ])
  );
  const previousName = observationTrend().series[0].name;
  await act(async () => {
    await appI18n.changeLanguage('zh_Hans');
  });
  view.rerender(content(nextViews, 6000, '#445566'));
  await waitFor(() => expect(observationTrend().color[0]).toBe('#445566'));
  expect(observationTrend().series[0].name).not.toBe(previousName);
  expect(observationTrend().series.map((series) => series.data)).toEqual([
    [1.5, 3],
    [0.5, 1]
  ]);
});

test('runtime host changes start a new observation visit series', async () => {
  window.history.replaceState({}, '', '/settings/events?tab=activity');
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  const view = renderPage(client);
  try {
    await screen.findByRole('img', { name: 'Record storage over time' });
    const runtimeKey = ['settings', 'events', 'runtime', 'csrf'];
    const capturedAt = client.getQueryState(runtimeKey)!.dataUpdatedAt;
    const next = withWorkspaceView({
      ...snapshot,
      observation_bytes: 2048,
      source_metadata_bytes: 512
    });
    act(() => {
      client.setQueryData(runtimeKey, next, { updatedAt: capturedAt + 5000 });
    });
    await waitFor(() =>
      expect(observationTrend().series[0].data).toHaveLength(2)
    );
    act(() => {
      client.setQueryData(
        runtimeKey,
        withWorkspaceView({ ...next, instance_id: 'replacement-host' }),
        { updatedAt: capturedAt + 10000 }
      );
    });
    await waitFor(() =>
      expect(observationTrend().series.map((series) => series.data)).toEqual([
        [1.5],
        [0.5]
      ])
    );
  } finally {
    view.unmount();
    client.clear();
  }
});
