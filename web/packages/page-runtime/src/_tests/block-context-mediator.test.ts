import { describe, expect, test } from 'vitest';

import { createBlockContextMediator } from '../index';

describe('BlockContext host mediator', () => {
  test('allows declared events and complete interface source descriptors', () => {
    const mediator = createBlockContextMediator({
      allowedEvents: ['record.saved'],
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
        interfaceId: 'records.list',
        schemaDigest: 'digest-1',
        request: { query: { page: 1 } }
      }).result
    ).toMatchObject({
      ok: true,
      effect: {
        type: 'interface',
        interfaceId: 'records.list',
        schemaDigest: 'digest-1'
      }
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
        interfaceId: 'delete_everything'
      },
      'effect_invalid'
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
