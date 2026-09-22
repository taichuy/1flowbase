import { beforeEach, expect, test, vi } from 'vitest';
import { apiFetch } from '../../../transport';
import {
  getConsoleProviderTrajectory,
  getConsoleRunTrajectory
} from '../trajectory';
import { getConsoleClientTrajectory } from '../client-trajectory';
vi.mock('../../../transport', () => ({
  apiFetch: vi.fn().mockResolvedValue({})
}));
beforeEach(() => {
  vi.mocked(apiFetch).mockClear();
});

test('native focus and trigger filter remain scoped and focus is only sent on the first page', async () => {
  const options = { request_id: 'request-B', focus_event_id: 'event-late' };
  await getConsoleProviderTrajectory(
    'app',
    'run',
    'node',
    undefined,
    undefined,
    options
  );
  expect(apiFetch).toHaveBeenLastCalledWith({
    path: '/api/console/applications/app/logs/runs/run/nodes/node/trajectory?limit=50&request_id=request-B&focus_event_id=event-late',
    baseUrl: undefined
  });
  await getConsoleRunTrajectory('app', 'run', 950, undefined, options);
  expect(apiFetch).toHaveBeenLastCalledWith({
    path: '/api/console/applications/app/logs/runs/run/trajectory?limit=50&request_id=request-B&cursor=950',
    baseUrl: undefined
  });
});
test('client focus uses the supplied request identity without fetching preceding pages', async () => {
  const options = { request_id: 'request-A', focus_step_id: 'request-A' };
  await getConsoleClientTrajectory(
    'app',
    'old-run',
    undefined,
    undefined,
    undefined,
    options
  );
  expect(apiFetch).toHaveBeenLastCalledWith({
    path: '/api/console/applications/app/logs/runs/old-run/client-trajectory?limit=50&request_id=request-A&focus_step_id=request-A',
    baseUrl: undefined
  });
  await getConsoleClientTrajectory(
    'app',
    'old-run',
    undefined,
    42,
    undefined,
    options
  );
  expect(apiFetch).toHaveBeenLastCalledWith({
    path: '/api/console/applications/app/logs/runs/old-run/client-trajectory?limit=50&request_id=request-A&cursor=42',
    baseUrl: undefined
  });
  expect(apiFetch).toHaveBeenCalledTimes(2);
});
