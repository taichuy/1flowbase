import '../../../../../test/fixtures/rc-util-unique-ids';
import {
  ApiClientError,
  type EventRuntimeSnapshot,
  type EventRegisteredSourceSnapshot
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
import { App as AntApp } from 'antd';
import { I18nextProvider } from 'react-i18next';
import { beforeEach, describe, expect, test, vi } from 'vitest';
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import { EventRuntimePanel } from '../EventRuntimePanel';
import { navigateEventScope } from '../event-navigation';

const api = vi.hoisted(() => ({ getEventRuntime: vi.fn() }));
const clipboard = vi.hoisted(() => ({ copy: vi.fn() }));
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
    expect(
      screen.getByRole('button', { name: 'Observation details' })
    ).toBeEnabled()
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
  test('source selection has pressed and row feedback, URL state, corresponding queue and copyable consumers', async () => {
    mount('queues');
    await ready();
    expect(screen.getByRole('button', { name: 'Source one' })).toHaveAttribute(
      'aria-pressed',
      'true'
    );
    fireEvent.click(screen.getByRole('button', { name: 'Source two' }));
    const selected = screen.getByRole('button', { name: 'Source two' });
    expect(selected).toHaveAttribute('aria-pressed', 'true');
    expect(selected.closest('tr')).toHaveAttribute('aria-selected', 'true');
    expect(window.location.search).toContain('source=two');
    expect(
      screen.getByRole('button', { name: 'queue:two' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'queue:one' })
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'queue:two' }));
    const drawer = screen.getByRole('dialog');
    expect(within(drawer).getByText('subscriber:two')).toBeInTheDocument();
    fireEvent.click(
      within(drawer).getByRole('button', { name: 'Copy Consumer ID' })
    );
    await waitFor(() =>
      expect(clipboard.copy).toHaveBeenCalledWith('consumer:two')
    );
    expect(within(drawer).getAllByText('Unknown')).toHaveLength(3);
  });
  test('an incomplete empty queue remains explicitly unknown, not a successful zero', async () => {
    const data = snapshot();
    data.source_views[0].sources = [
      { ...source('one'), queues: [], queue_observation_complete: false }
    ];
    api.getEventRuntime.mockResolvedValue(data);
    mount('queues');
    await ready();
    expect(
      screen.getByText(/an empty list does not establish that no queues exist/)
    ).toBeInTheDocument();
    expect(
      screen.getByText(/No queue metadata is visible/)
    ).toBeInTheDocument();
    expect(
      screen.queryByText(
        'No original queues were observed for this source in this snapshot.'
      )
    ).not.toBeInTheDocument();
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
    expect(await screen.findByText('Refreshing snapshot…')).toBeInTheDocument();
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
  test('an unavailable URL scope does not silently show the workspace and switching scopes clears scoped detail state', async () => {
    mount('activity', '&scope=system&record=old&source=old');
    await waitFor(() =>
      expect(
        screen.getByText(/The requested scope is not available/)
      ).toBeInTheDocument()
    );
    expect(
      screen.queryByText('Workflow loop prevented')
    ).not.toBeInTheDocument();
    await act(async () => navigateEventScope('workspace:workspace'));
    expect(
      await screen.findByText('Workflow loop prevented')
    ).toBeInTheDocument();
    expect(window.location.search).not.toContain('record=');
    expect(window.location.search).not.toContain('source=');
  });
  test('missing loss accounting is visible and does not invent zero counts in diagnostics', async () => {
    const data = snapshot();
    data.source_views[0].observation_loss.accounting_available = false;
    api.getEventRuntime.mockResolvedValue(data);
    mount();
    await ready();
    expect(screen.getByRole('alert')).toHaveTextContent(
      'No observation loss accounting window is available'
    );
    fireEvent.click(
      screen.getByRole('button', { name: 'Observation details' })
    );
    expect(
      within(screen.getByRole('dialog')).queryByText(
        'Records expired in window'
      )
    ).not.toBeInTheDocument();
  });
  test('a disappeared record URL explains the missing context instead of silently closing', async () => {
    mount('activity', '&record=missing');
    await ready();
    expect(screen.getByRole('dialog')).toHaveTextContent(
      'This record is not in the latest snapshot.'
    );
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
