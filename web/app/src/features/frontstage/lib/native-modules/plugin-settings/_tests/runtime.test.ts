import { beforeEach, describe, expect, test, vi } from 'vitest';
import { fetchRecordCapabilities } from '@1flowbase/api-client';
import { getRecordCapabilities } from '../runtime';
import {
  resetAuthStore,
  useAuthStore
} from '../../../../../../state/auth-store';

vi.mock('@1flowbase/api-client', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@1flowbase/api-client')>()),
  fetchRecordCapabilities: vi.fn()
}));

beforeEach(() => {
  vi.clearAllMocks();
  resetAuthStore();
});

describe('plugin settings capabilities bridge', () => {
  test('reads the current session for each call and forwards only resource identity', async () => {
    vi.mocked(fetchRecordCapabilities).mockResolvedValue({
      items: [
        {
          record_id: 'h1',
          allowed_actions: ['view'],
          can_manage_permissions: false
        }
      ]
    });
    const signal = new AbortController().signal;
    useAuthStore.setState({ csrfToken: 'session-a' });
    const first = await getRecordCapabilities('ssh', 'hosts', ['h1'], signal);
    expect(first.items[0].allowed_actions).toEqual(['view']);
    expect(fetchRecordCapabilities).toHaveBeenNthCalledWith(
      1,
      { plugin_code: 'ssh', resource_code: 'hosts', record_ids: ['h1'] },
      'session-a',
      undefined,
      signal
    );
    useAuthStore.setState({ csrfToken: 'session-b' });
    await getRecordCapabilities('ssh', 'hosts', ['h1']);
    expect(fetchRecordCapabilities).toHaveBeenNthCalledWith(
      2,
      { plugin_code: 'ssh', resource_code: 'hosts', record_ids: ['h1'] },
      'session-b',
      undefined,
      undefined
    );
  });
});
