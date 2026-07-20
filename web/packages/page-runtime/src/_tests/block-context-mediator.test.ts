import { describe, expect, test } from 'vitest';

import { createBlockContextMediator } from '../index';

describe('BlockContext host mediator', () => {
  test('allows only declared events and interface aliases', () => {
    const mediator = createBlockContextMediator({
      allowedEvents: ['record.saved'],
      allowedInterfaces: ['records'],
      maxEventChainDepth: 1
    });
    expect(
      mediator.handle({
        type: 'event',
        requestId: 'run-1',
        name: 'record.saved',
        payload: { id: 'record-1' }
      }).result
    ).toMatchObject({ ok: true });
    expect(
      mediator.handle({
        type: 'interface',
        requestId: 'run-1',
        effectId: 'effect-1',
        bindingAlias: 'records',
        request: { query: { page: 1 } }
      }).result
    ).toMatchObject({
      ok: true,
      effect: { type: 'interface', bindingAlias: 'records' }
    });
  });

  test.each([
    [
      { type: 'event', requestId: 'run-1', name: 'private.event' },
      'event_denied'
    ],
    [
      {
        type: 'interface',
        requestId: 'run-1',
        bindingAlias: 'deleteEverything'
      },
      'interface_denied'
    ],
    [
      { type: 'action', requestId: 'run-1', actionId: 'legacy' },
      'effect_invalid'
    ]
  ])('rejects denied or removed effects', (effect, code) => {
    expect(createBlockContextMediator({}).handle(effect).result).toMatchObject({
      ok: false,
      code
    });
  });

  test('rejects cyclic event chains and non-serializable payloads', () => {
    const mediator = createBlockContextMediator({
      allowedEvents: ['next'],
      maxEventChainDepth: 1
    });
    expect(
      mediator.handle(
        { type: 'event', requestId: 'run-1', name: 'next' },
        { tickId: 'tick-1' }
      ).result
    ).toMatchObject({ ok: true });
    expect(
      mediator.handle(
        { type: 'event', requestId: 'run-1', name: 'next' },
        { tickId: 'tick-1' }
      ).result
    ).toMatchObject({ ok: false, code: 'event_denied', path: 'event.chain' });
    expect(
      mediator.handle({
        type: 'event',
        requestId: 'run-2',
        name: 'next',
        payload: { invalid: () => undefined }
      }).result
    ).toMatchObject({ ok: false, code: 'payload_invalid' });
  });
});
