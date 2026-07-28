import { beforeEach, describe, expect, test, vi } from 'vitest';

import {
  STUDIO_RUN_CONSOLE_ENTRY_LIMIT,
  createStudioRunConsole,
  createStudioRunConsoleStore,
  formatStudioRunConsoleArguments
} from '../../components/jsx-studio/studio-run-console';

describe('Studio run Console observer', () => {
  const forwardConsole = {
    ...console,
    debug: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
    log: vi.fn(),
    warn: vi.fn()
  } as Console;

  beforeEach(() => {
    vi.clearAllMocks();
  });

  test('R7-AC-001/003 captures only the current run and preserves method order', async () => {
    const store = createStudioRunConsoleStore();
    let current = true;
    const runConsole = createStudioRunConsole({
      store,
      isCurrentRun: () => current,
      forwardConsole
    });
    const listener = vi.fn();
    store.subscribe(listener);

    runConsole.log('first', 1);
    runConsole.warn('second');
    current = false;
    runConsole.error('stale');
    await Promise.resolve();

    expect(store.getSnapshot()).toEqual([
      { sequence: 0, level: 'log', message: 'first 1' },
      { sequence: 1, level: 'warn', message: 'second' }
    ]);
    expect(listener).toHaveBeenCalledOnce();
    expect(forwardConsole.error).toHaveBeenCalledWith('stale');
  });

  test('R7-AC-002 serializes complex and hostile values without throwing', () => {
    const circular: Record<string, unknown> = { answer: 42 };
    circular.self = circular;
    const hostile = Object.create(null, {
      broken: {
        enumerable: true,
        get() {
          throw new Error('getter exploded');
        }
      }
    });

    expect(
      formatStudioRunConsoleArguments([
        'payload',
        circular,
        ['value', 2n],
        new TypeError('bad value'),
        hostile
      ])
    ).toBe(
      'payload {"answer": 42, "self": [Circular]} ["value", 2n] TypeError: bad value {"broken": [Thrown: getter exploded]}'
    );
  });

  test('R7-AC-004 keeps a bounded buffer and clears it for a new run', () => {
    const store = createStudioRunConsoleStore();
    for (let index = 0; index <= STUDIO_RUN_CONSOLE_ENTRY_LIMIT; index += 1) {
      store.publish('log', [index]);
    }

    expect(store.getSnapshot()).toHaveLength(STUDIO_RUN_CONSOLE_ENTRY_LIMIT);
    expect(store.getSnapshot()[0]).toMatchObject({ message: '1' });
    store.clear();
    expect(store.getSnapshot()).toEqual([]);
  });

  test('R7-AC-003 stops notifying after a subscriber leaves', async () => {
    const store = createStudioRunConsoleStore();
    const listener = vi.fn();
    const unsubscribe = store.subscribe(listener);
    unsubscribe();
    store.publish('info', ['after unsubscribe']);
    await Promise.resolve();

    expect(store.getSnapshot()).toEqual([
      { sequence: 0, level: 'info', message: 'after unsubscribe' }
    ]);
    expect(listener).not.toHaveBeenCalled();
  });
});
