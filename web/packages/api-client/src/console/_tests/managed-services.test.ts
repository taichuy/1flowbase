import { afterEach, describe, expect, it, vi } from 'vitest';
import { requestManagedService } from '../managed-services';

afterEach(() => vi.unstubAllGlobals());

describe('managed service transport', () => {
  it('uses the session CSRF token and keeps the backend DTO unchanged', async () => {
    const fetch = vi.fn().mockResolvedValue(new Response(JSON.stringify({ data: { host_id: 'host-1' } })));
    vi.stubGlobal('fetch', fetch);
    const body = { host_id: 'host-1' };
    await expect(requestManagedService({ path: '/api/console/managed-services/example/hosts', method: 'POST', body }, 'csrf')).resolves.toEqual(body);
    expect(fetch).toHaveBeenCalledWith(expect.stringContaining('/api/console/managed-services/example/hosts'), expect.objectContaining({ credentials: 'include', headers: { 'content-type': 'application/json', 'x-csrf-token': 'csrf' }, body: JSON.stringify(body) }));
  });

  it.each(['https://external.invalid/api/console/managed-services/x', '/api/console/managed-services/../../auth', '/api/public/auth/sign-in', '/api/console/managed-services/x#fragment'])(
    'rejects paths outside the managed service transport: %s', async (path) => {
      const fetch = vi.fn();
      vi.stubGlobal('fetch', fetch);
      await expect(requestManagedService({ path }, null)).rejects.toThrow('Invalid managed service route');
      expect(fetch).not.toHaveBeenCalled();
    }
  );
});
