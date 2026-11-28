import { expect, test, vi } from 'vitest';
import { evaluateConsoleAccess } from '../../../auth';

test('posts only the requested codes with the session CSRF token', async () => {
  const data = {
    policy_fingerprint: 'policy',
    allowed: { read: true, write: false }
  };
  const fetch = vi
    .spyOn(globalThis, 'fetch')
    .mockResolvedValue(
      new Response(JSON.stringify({ data }), {
        headers: { 'content-type': 'application/json' }
      })
    );
  try {
    await expect(
      evaluateConsoleAccess(['read', 'write'], 'csrf', 'https://flow.example')
    ).resolves.toEqual(data);
    expect(fetch).toHaveBeenCalledExactlyOnceWith(
      'https://flow.example/api/console/access/evaluate',
      expect.objectContaining({
        method: 'POST',
        credentials: 'include',
        body: JSON.stringify({ codes: ['read', 'write'] }),
        headers: expect.objectContaining({ 'x-csrf-token': 'csrf' })
      })
    );
  } finally {
    fetch.mockRestore();
  }
});
