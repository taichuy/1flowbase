import { beforeEach, expect, test, vi } from 'vitest';
import { apiFetch } from '../../../transport';
import {
  getConsoleApplicationLogRecord,
  getConsoleApplicationLogRecordClientTrajectory,
  getConsoleApplicationLogRecordClientTrajectorySection
} from '../records';
import { getConsoleClientTrajectory } from '../client-trajectory';
vi.mock('../../../transport', () => ({
  apiFetch: vi.fn().mockResolvedValue({})
}));
beforeEach(() => vi.mocked(apiFetch).mockClear());
test('record overview uses its source-neutral record route', async () => {
  await getConsoleApplicationLogRecord('app', 'record', 'https://api.example');
  expect(apiFetch).toHaveBeenLastCalledWith({
    path: '/api/console/applications/app/logs/records/record',
    baseUrl: 'https://api.example'
  });
});
test('record trajectory preserves exact focus and cursors and sections stay scoped to record', async () => {
  const options = {
    request_id: 'source-session',
    focus_step_id: 'source-event'
  };
  await getConsoleApplicationLogRecordClientTrajectory(
    'app',
    'record',
    undefined,
    options
  );
  expect(apiFetch).toHaveBeenLastCalledWith({
    path: '/api/console/applications/app/logs/records/record/client-trajectory?limit=50&request_id=source-session&focus_step_id=source-event',
    baseUrl: undefined
  });
  await getConsoleApplicationLogRecordClientTrajectory(
    'app',
    'record',
    99,
    options
  );
  expect(apiFetch).toHaveBeenLastCalledWith({
    path: '/api/console/applications/app/logs/records/record/client-trajectory?limit=100&cursor=99&request_id=source-session',
    baseUrl: undefined
  });
  await getConsoleApplicationLogRecordClientTrajectorySection(
    'app',
    'record',
    'step',
    'raw',
    88
  );
  expect(apiFetch).toHaveBeenLastCalledWith({
    path: '/api/console/applications/app/logs/records/record/client-trajectory/step?limit=8&section=raw&cursor=88',
    baseUrl: undefined
  });
  await getConsoleClientTrajectory('app', 'native-run');
  expect(apiFetch).toHaveBeenLastCalledWith({
    path: '/api/console/applications/app/logs/runs/native-run/client-trajectory?limit=50',
    baseUrl: undefined
  });
});
