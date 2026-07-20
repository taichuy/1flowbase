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
      mediator: createBlockContextMediator({ allowedInterfaces: ['records'] }),
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
        bindingAlias: 'records',
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

  test('returns failed effect results for denied aliases and missing handlers', () => {
    const denied: JsBlockWorkerEffectResultMessage[] = [];
    const deniedBridge = createJsBlockHostEffectBridge({
      mediator: createBlockContextMediator({ allowedInterfaces: [] }),
      resolveEffect: (message) => denied.push(message)
    });
    deniedBridge.handle({
      direction: 'worker_to_host',
      type: 'interface',
      requestId: 'run-1',
      effectId: 'effect-1',
      bindingAlias: 'records'
    });
    expect(denied[0]).toMatchObject({
      ok: false,
      error: { errors: [{ code: 'interface_denied' }] }
    });

    const missing: JsBlockWorkerEffectResultMessage[] = [];
    createJsBlockHostEffectBridge({
      mediator: createBlockContextMediator({ allowedInterfaces: ['records'] }),
      resolveEffect: (message) => missing.push(message)
    }).handle({
      direction: 'worker_to_host',
      type: 'interface',
      requestId: 'run-1',
      effectId: 'effect-1',
      bindingAlias: 'records'
    });
    expect(missing[0]).toMatchObject({
      ok: false,
      error: { errors: [{ path: 'interface.handler' }] }
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
