import type { NativeReactRuntimeConsole } from '@1flowbase/page-runtime';

export type StudioRunConsoleLevel =
  | 'debug'
  | 'info'
  | 'log'
  | 'warn'
  | 'error';

export interface StudioRunConsoleEntry {
  sequence: number;
  level: StudioRunConsoleLevel;
  message: string;
}

export interface StudioRunConsoleStore {
  clear(): void;
  getSnapshot(): readonly StudioRunConsoleEntry[];
  publish(level: StudioRunConsoleLevel, args: readonly unknown[]): void;
  subscribe(listener: () => void): () => void;
}

export const STUDIO_RUN_CONSOLE_ENTRY_LIMIT = 200;

const CONSOLE_METHOD_LEVELS = {
  debug: 'debug',
  error: 'error',
  info: 'info',
  log: 'log',
  warn: 'warn'
} as const;
const MAX_VALUE_DEPTH = 5;
const MAX_COLLECTION_ITEMS = 50;
const MAX_MESSAGE_LENGTH = 10_000;

export function createStudioRunConsoleStore(): StudioRunConsoleStore {
  let snapshot: readonly StudioRunConsoleEntry[] = [];
  let sequence = 0;
  let notificationPending = false;
  const listeners = new Set<() => void>();

  const notify = () => {
    if (notificationPending) return;
    notificationPending = true;
    queueMicrotask(() => {
      notificationPending = false;
      for (const listener of listeners) listener();
    });
  };

  return {
    clear() {
      if (snapshot.length === 0) return;
      snapshot = [];
      sequence = 0;
      notify();
    },
    getSnapshot() {
      return snapshot;
    },
    publish(level, args) {
      const entry: StudioRunConsoleEntry = {
        sequence,
        level,
        message: formatStudioRunConsoleArguments(args)
      };
      sequence += 1;
      snapshot = [...snapshot, entry].slice(-STUDIO_RUN_CONSOLE_ENTRY_LIMIT);
      notify();
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    }
  };
}

export function createStudioRunConsole({
  store,
  isCurrentRun,
  forwardConsole = globalThis.console
}: {
  store: StudioRunConsoleStore;
  isCurrentRun(): boolean;
  forwardConsole?: Console;
}): NativeReactRuntimeConsole {
  const observedMethods = Object.fromEntries(
    Object.entries(CONSOLE_METHOD_LEVELS).map(([method, level]) => [
      method,
      (...args: unknown[]) => {
        if (isCurrentRun()) store.publish(level, args);
        const forward = forwardConsole[method as keyof typeof CONSOLE_METHOD_LEVELS];
        if (typeof forward === 'function') {
          Reflect.apply(forward, forwardConsole, args);
        }
      }
    ])
  );

  return new Proxy(forwardConsole, {
    get(target, property, receiver) {
      if (typeof property === 'string' && property in observedMethods) {
        return observedMethods[property];
      }
      const value = Reflect.get(target, property, receiver);
      return typeof value === 'function' ? value.bind(target) : value;
    }
  });
}

export function formatStudioRunConsoleArguments(
  args: readonly unknown[]
): string {
  const message = args
    .map((value) => formatConsoleValue(value, false, new WeakSet(), 0))
    .join(' ');
  return message.length > MAX_MESSAGE_LENGTH
    ? `${message.slice(0, MAX_MESSAGE_LENGTH)}…`
    : message;
}

function formatConsoleValue(
  value: unknown,
  nested: boolean,
  ancestors: WeakSet<object>,
  depth: number
): string {
  if (value === null) return 'null';
  if (typeof value === 'string') return nested ? JSON.stringify(value) : value;
  if (typeof value === 'undefined') return 'undefined';
  if (typeof value === 'bigint') return `${value}n`;
  if (typeof value === 'symbol') return String(value);
  if (typeof value === 'function') {
    return `[Function${value.name ? ` ${value.name}` : ''}]`;
  }
  if (typeof value !== 'object') return String(value);
  if (value instanceof Error) return `${value.name}: ${value.message}`;
  if (value instanceof Date) {
    return Number.isNaN(value.valueOf()) ? 'Invalid Date' : value.toISOString();
  }
  if (value instanceof RegExp) return String(value);
  if (depth >= MAX_VALUE_DEPTH) return '[Max Depth]';
  if (ancestors.has(value)) return '[Circular]';

  ancestors.add(value);
  try {
    if (Array.isArray(value)) {
      const items = value
        .slice(0, MAX_COLLECTION_ITEMS)
        .map((item) => formatConsoleValue(item, true, ancestors, depth + 1));
      if (value.length > MAX_COLLECTION_ITEMS) items.push('…');
      return `[${items.join(', ')}]`;
    }

    let keys: string[];
    try {
      keys = Object.keys(value).sort().slice(0, MAX_COLLECTION_ITEMS);
    } catch (error) {
      return `[Unserializable: ${errorMessage(error)}]`;
    }
    const fields = keys.map((key) => {
      try {
        return `${JSON.stringify(key)}: ${formatConsoleValue(
          (value as Record<string, unknown>)[key],
          true,
          ancestors,
          depth + 1
        )}`;
      } catch (error) {
        return `${JSON.stringify(key)}: [Thrown: ${errorMessage(error)}]`;
      }
    });
    try {
      if (Object.keys(value).length > MAX_COLLECTION_ITEMS) fields.push('…');
    } catch {
      // The first own-key read already produced the useful diagnostic.
    }
    return `{${fields.join(', ')}}`;
  } finally {
    ancestors.delete(value);
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error && error.message
    ? error.message
    : String(error);
}
