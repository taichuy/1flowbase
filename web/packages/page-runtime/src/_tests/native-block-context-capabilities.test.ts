import { describe, expect, test, vi } from 'vitest';

import { createNativeBlockContextCapabilities } from '../native-block-context/capabilities';

describe('Native Block context capabilities', () => {
  test('D4-AC-001/003 observes concurrent API calls without replacing the capability instance', async () => {
    const pending = deferred<unknown>();
    const observations: Array<{ status: string; callId: string }> = [];
    const interfaceHandler = vi
      .fn()
      .mockReturnValueOnce(pending.promise)
      .mockResolvedValueOnce({ id: 'second' });
    const capabilities = createNativeBlockContextCapabilities({
      requestId: 'native:block-1:epoch-1',
      instanceEpoch: 'epoch-1',
      isCurrentInstance: () => true,
      interfaceHandler,
      outputs: { publish: () => ({ ok: true, stale: false }) },
      observeApiCall: (observation) => observations.push(observation)
    });

    const first = capabilities.api.get('/api/console/records/first');
    await expect(
      capabilities.api.get('/api/console/records/second')
    ).resolves.toEqual({ id: 'second' });
    expect(capabilities.api).toBe(capabilities.api);
    expect(observations.filter(({ status }) => status === 'pending')).toHaveLength(2);

    pending.resolve({ id: 'first' });
    await expect(first).resolves.toEqual({ id: 'first' });
    expect(new Set(observations.map(({ callId }) => callId).values()).size).toBe(2);
  });

  test('D4-AC-004 rejects stale API/events/output channels by instance epoch', async () => {
    let current = true;
    const interfaceHandler = vi.fn().mockResolvedValue({ ok: true });
    const emitEvent = vi.fn();
    const publish = vi.fn(() => ({ ok: true, stale: false }));
    const capabilities = createNativeBlockContextCapabilities({
      requestId: 'native:block-1:epoch-1',
      instanceEpoch: 'epoch-1',
      isCurrentInstance: () => current,
      interfaceHandler,
      emitEvent,
      outputs: { publish }
    });
    current = false;

    await expect(capabilities.api.get('/api/console/records')).rejects.toThrow(
      'stale instance'
    );
    expect(() => capabilities.events.emit('record.opened')).toThrow(
      'stale instance'
    );
    expect(capabilities.outputs.publish({ record_id: 'record-1' })).toEqual({
      ok: false,
      stale: true
    });
    expect(interfaceHandler).not.toHaveBeenCalled();
    expect(emitEvent).not.toHaveBeenCalled();
    expect(publish).not.toHaveBeenCalled();
  });

  test('D4-AC-004 marks an asynchronous output stale when its instance epoch ends while publishing', async () => {
    let current = true;
    const pending = deferred<{ ok: boolean; stale: boolean }>();
    const capabilities = createNativeBlockContextCapabilities({
      requestId: 'native:block-1:epoch-1',
      instanceEpoch: 'epoch-1',
      isCurrentInstance: () => current,
      interfaceHandler: vi.fn(),
      outputs: { publish: () => pending.promise }
    });

    const published = capabilities.outputs.publish({ total: 1 });
    current = false;
    pending.resolve({ ok: true, stale: false });
    await expect(published).resolves.toEqual({ ok: false, stale: true });
  });
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => {
    resolve = next;
  });
  return { promise, resolve };
}
