import { describe, expect, test, vi } from 'vitest';

import {
  createBlockContextMediator,
  createJsBlockHostEffectBridge,
  type JsBlockWorkerEffectResultMessage
} from '../index';

describe('JS block host mediator effect bridge', () => {
  test('resolves an allowed interface and records a redacted trace', async () => {
    const messages: JsBlockWorkerEffectResultMessage[] = [];
    const traces: unknown[] = [];
    const bridge = createJsBlockHostEffectBridge({
      mediator: createBlockContextMediator({}),
      resolveEffect: (message) => messages.push(message),
      handlers: { interface: async () => ({ token: 'secret', total: 2 }) },
      onInterfaceCall: (trace) => traces.push(trace)
    });
    expect(
      bridge.handle({
        direction: 'worker_to_host',
        type: 'interface',
        requestId: 'run-1',
        effectId: 'effect-1',
        method: 'GET',
        path: '/api/console/test',
        request: { headers: { authorization: 'Bearer secret' } }
      })
    ).toMatchObject({ handled: true, transition: { result: { ok: true } } });
    await vi.waitFor(() => expect(messages).toHaveLength(1));
    expect(messages[0]).toMatchObject({ ok: true, value: { total: 2 } });
    expect(traces[0]).toMatchObject({
      status: 'succeeded',
      request: { headers: { authorization: '[REDACTED]' } },
      response: { token: '[REDACTED]', total: 2 }
    });
  });

  test('returns failed effect results for incomplete routes and missing handlers', () => {
    expect(
      createJsBlockHostEffectBridge({
        mediator: createBlockContextMediator({}),
        resolveEffect: vi.fn()
      }).handle({
        direction: 'worker_to_host',
        type: 'interface',
        requestId: 'run-1',
        effectId: 'effect-invalid',
        method: 'GET'
      })
    ).toMatchObject({
      handled: true,
      transition: { result: { ok: false, code: 'effect_invalid' } }
    });

    const missing: JsBlockWorkerEffectResultMessage[] = [];
    createJsBlockHostEffectBridge({
      mediator: createBlockContextMediator({}),
      resolveEffect: (message) => missing.push(message)
    }).handle({
      direction: 'worker_to_host',
      type: 'interface',
      requestId: 'run-1',
      effectId: 'effect-1',
      method: 'GET',
      path: '/api/console/test'
    });
    expect(missing[0]).toMatchObject({
      ok: false,
      error: { errors: [{ path: 'interface.handler' }] }
    });
  });

  test('summarizes binary payloads instead of copying them into interface traces', async () => {
    const traces: unknown[] = [];
    const bridge = createJsBlockHostEffectBridge({
      mediator: createBlockContextMediator({}),
      resolveEffect: vi.fn(),
      handlers: {
        interface: async () => ({
          bytes: new Uint8Array([1, 2, 3]),
          file_name: 'download.bin',
          content_type: 'application/octet-stream'
        })
      },
      onInterfaceCall: (trace) => traces.push(trace)
    });
    bridge.handle({
      direction: 'worker_to_host',
      type: 'interface',
      requestId: 'run-1',
      effectId: 'effect-binary',
      method: 'GET',
      path: '/api/console/test',
      request: { body: { base64: 'A'.repeat(8_000) } }
    });
    await vi.waitFor(() => expect(traces).toHaveLength(1));
    expect(traces[0]).toMatchObject({
      request: { body: { base64: '[BASE64 8000 chars]' } },
      response: {
        bytes: { type: 'Uint8Array', byte_length: 3 },
        file_name: 'download.bin'
      }
    });
  });

  test('passes events without requiring a result handler and ignores unknown messages', () => {
    const bridge = createJsBlockHostEffectBridge({
      mediator: createBlockContextMediator({ allowedEvents: ['ready'] }),
      resolveEffect: vi.fn()
    });
    expect(
      bridge.handle({
        direction: 'worker_to_host',
        type: 'event',
        requestId: 'run-1',
        name: 'ready'
      })
    ).toMatchObject({ handled: true });
    expect(bridge.handle({ type: 'log' })).toEqual({ handled: false });
  });
});
