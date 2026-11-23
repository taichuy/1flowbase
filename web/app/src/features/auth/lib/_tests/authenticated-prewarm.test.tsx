import { StrictMode, useRef } from 'react';
import { act, fireEvent, render, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import {
  pauseAuthenticatedPrewarm,
  useAuthenticatedPrewarm
} from '../authenticated-prewarm';
import {
  resolveRuntimeConcurrencyProfile,
  type RuntimeConcurrencyProfile
} from '../../../../shared/utils/runtime-concurrency-profile';

const { imported, preloadUrl } = vi.hoisted(() => ({
  imported: vi.fn(),
  preloadUrl: '/assets/authenticated-runtime.test.js'
}));
vi.mock('virtual:1flowbase-authenticated-preload', () => ({
  default: preloadUrl
}));
vi.mock('../../../../app/AuthenticatedAppRuntime', () => {
  imported();
  return { AuthenticatedAppRuntime: () => null };
});
vi.mock('../../components/default-password/DefaultPasswordSignIn', () => ({
  matchesDefaultPasswordSource: () => false
}));
function Host({
  ready,
  identity = 'same-entry',
  concurrencyProfile
}: {
  ready: boolean;
  identity?: string;
  concurrencyProfile?: RuntimeConcurrencyProfile;
}) {
  const host = useRef<HTMLDivElement>(null);
  useAuthenticatedPrewarm(host, identity, concurrencyProfile);
  return (
    <div ref={host}>
      {ready ? (
        <form>
          <input name="account" />
        </form>
      ) : (
        <span>loading</span>
      )}
    </div>
  );
}
const preloads = () =>
  [
    ...document.querySelectorAll<HTMLLinkElement>('link[rel="modulepreload"]')
  ].filter((link) => link.href === new URL(preloadUrl, document.baseURI).href);
let idle: IdleRequestCallback;
const request = vi.fn((callback: IdleRequestCallback) => {
  idle = callback;
  return 7;
});
const cancel = vi.fn();
const mark = vi.fn();
const originalMark = Object.getOwnPropertyDescriptor(performance, 'mark');
const runIdle = () =>
  act(async () => idle({ didTimeout: false, timeRemaining: () => 50 }));
beforeEach(() => {
  request.mockClear();
  cancel.mockClear();
  mark.mockClear();
  // Do not clear imported: an accidental startup evaluation must also fail.
  vi.stubGlobal('requestIdleCallback', request);
  vi.stubGlobal('cancelIdleCallback', cancel);
  Object.defineProperty(performance, 'mark', { configurable: true, value: mark });
  Object.defineProperty(navigator, 'connection', {
    configurable: true,
    value: undefined
  });
});
afterEach(() => {
  preloads().forEach((link) => link.remove());
  if (originalMark) Object.defineProperty(performance, 'mark', originalMark);
  else Reflect.deleteProperty(performance, 'mark');
  vi.unstubAllGlobals();
});
test('does not schedule during loading, schedules only after a real form commit, cancels on unmount', async () => {
  const view = render(
    <StrictMode>
      <Host ready={false} />
    </StrictMode>
  );
  expect(request).not.toHaveBeenCalled();
  expect(imported).not.toHaveBeenCalled();
  expect(preloads()).toHaveLength(0);
  expect(mark).not.toHaveBeenCalled();
  view.rerender(
    <StrictMode>
      <Host ready />
    </StrictMode>
  );
  await waitFor(() => expect(request).toHaveBeenCalledTimes(1));
  expect(imported).not.toHaveBeenCalled();
  expect(preloads()).toHaveLength(0);
  expect(mark).toHaveBeenCalledWith('anonymous:form-committed');
  view.unmount();
  expect(cancel).toHaveBeenCalledWith(7);
  await runIdle();
  expect(imported).not.toHaveBeenCalled();
  expect(preloads()).toHaveLength(0);
  expect(mark).not.toHaveBeenCalledWith('anonymous:authenticated-prewarm-start');
});
test.each([
  { saveData: true },
  { effectiveType: '2g' },
  { effectiveType: 'slow-2g' }
])('does not prewarm on constrained connection %j', (connection) => {
  Object.defineProperty(navigator, 'connection', {
    configurable: true,
    value: connection
  });
  render(<Host ready />);
  expect(request).not.toHaveBeenCalled();
  expect(imported).not.toHaveBeenCalled();
  expect(preloads()).toHaveLength(0);
});
test('connection is rechecked at idle, and a missing idle API never falls back to a timer', async () => {
  const view = render(<Host ready />);
  Object.defineProperty(navigator, 'connection', {
    configurable: true,
    value: { saveData: true }
  });
  await runIdle();
  expect(imported).not.toHaveBeenCalled();
  expect(preloads()).toHaveLength(0);
  view.unmount();
  request.mockClear();
  vi.stubGlobal('requestIdleCallback', undefined);
  Object.defineProperty(navigator, 'connection', {
    configurable: true,
    value: undefined
  });
  const timeout = vi.spyOn(window, 'setTimeout');
  try {
    render(<Host ready />);
    expect(request).not.toHaveBeenCalled();
    expect(timeout).not.toHaveBeenCalled();
    expect(preloads()).toHaveLength(0);
    expect(imported).not.toHaveBeenCalled();
  } finally {
    timeout.mockRestore();
  }
});
test('idle modulepreloads the authenticated URL at low priority without evaluating the runtime', async () => {
  render(<Host ready />);
  expect(request).toHaveBeenCalledTimes(1);
  expect(imported).not.toHaveBeenCalled();
  expect(preloads()).toHaveLength(0);
  await runIdle();
  expect(preloads()).toHaveLength(1);
  const [link] = preloads();
  expect(link).toHaveAttribute('href', preloadUrl);
  expect(link).toHaveAttribute('crossorigin', 'anonymous');
  expect(link.fetchPriority).toBe('low');
  expect(imported).not.toHaveBeenCalled();
  expect(mark.mock.calls.map(([name]) => name)).toEqual([
    'anonymous:form-committed',
    'anonymous:authenticated-prewarm-start'
  ]);
  fireEvent.load(link);
  expect(mark).toHaveBeenLastCalledWith('anonymous:authenticated-prewarm-end');
  expect(imported).not.toHaveBeenCalled();
});
test('a preload error is marked without importing the runtime as a fallback', async () => {
  render(<Host ready />);
  await runIdle();
  fireEvent.error(preloads()[0]);
  expect(mark).toHaveBeenLastCalledWith('anonymous:authenticated-prewarm-failed');
  expect(mark).not.toHaveBeenCalledWith('anonymous:authenticated-prewarm-end');
  expect(imported).not.toHaveBeenCalled();
});
test('removing the committed form cancels pending idle work', async () => {
  const view = render(<Host ready />);
  view.rerender(<Host ready={false} />);
  await waitFor(() => expect(cancel).toHaveBeenCalledWith(7));
  await runIdle();
  expect(preloads()).toHaveLength(0);
  expect(imported).not.toHaveBeenCalled();
});
test('StrictMode replay and entry changes retain one preload per document', async () => {
  const view = render(
    <StrictMode>
      <Host ready />
    </StrictMode>
  );
  expect(request).toHaveBeenCalledTimes(2);
  expect(cancel).toHaveBeenCalledWith(7);
  const staleIdle = request.mock.calls[0][0];
  await act(async () =>
    staleIdle({ didTimeout: false, timeRemaining: () => 50 })
  );
  expect(preloads()).toHaveLength(0);
  await runIdle();
  const [link] = preloads();
  expect(preloads()).toHaveLength(1);
  view.rerender(
    <StrictMode>
      <Host ready identity="another-entry" />
    </StrictMode>
  );
  await runIdle();
  expect(preloads()).toEqual([link]);
  view.unmount();
  render(<Host ready />);
  await runIdle();
  expect(preloads()).toEqual([link]);
  expect(
    mark.mock.calls.filter(
      ([name]) => name === 'anonymous:authenticated-prewarm-start'
    )
  ).toHaveLength(1);
  expect(imported).not.toHaveBeenCalled();
});
test('an existing modulepreload for the resolved URL is reused without claiming a new fetch', async () => {
  const existing = document.createElement('link');
  existing.rel = 'modulepreload';
  existing.href = new URL(preloadUrl, document.baseURI).href;
  document.head.append(existing);
  render(<Host ready />);
  await runIdle();
  expect(preloads()).toEqual([existing]);
  expect(mark).not.toHaveBeenCalledWith('anonymous:authenticated-prewarm-start');
  expect(imported).not.toHaveBeenCalled();
});

test('streams transition delta chunks with K=1 BDP pacing, prioritizes active locale, and halts at 0 ms on form submit', async () => {
  const previousLang = document.documentElement.lang;
  document.documentElement.lang = 'zh-CN';
  const allPreloads = () => [
    ...document.querySelectorAll<HTMLLinkElement>('link[rel="modulepreload"]')
  ];
  vi.stubGlobal(
    'fetch',
    vi.fn(async () => ({
      ok: true,
      json: async () => ({
        files: [
          'assets/auth-startup-11111111.js',
          'assets/i18n-shell-en-22222222.js',
          'assets/i18n-shell-zh-33333333.js',
          'assets/shell-core-44444444.js',
          'assets/frontstage-core-55555555.js'
        ]
      })
    }))
  );
  try {
    const view = render(<Host ready />);
    await runIdle();
    const entryLink = preloads()[0]!;
    await act(async () => {
      fireEvent.load(entryLink);
    });
    await waitFor(() => {
      const deltaLinks = allPreloads().filter((l) => l !== entryLink);
      expect(deltaLinks).toHaveLength(1);
    });
    const firstLink = allPreloads().find((l) => l !== entryLink)!;
    expect(new URL(firstLink.href).pathname).toBe(
      '/assets/auth-startup-11111111.js'
    );
    await act(async () => {
      fireEvent.load(firstLink);
    });
    const afterFirst = allPreloads().filter((l) => l !== entryLink);
    // Inactive English dictionary is deferred behind shell-core so zh-CN shell warms first
    expect(afterFirst.map((l) => new URL(l.href).pathname)).toEqual([
      '/assets/auth-startup-11111111.js',
      '/assets/i18n-shell-zh-33333333.js'
    ]);
    // Submitting the form halts prewarming in capture phase and removes unsettled pending link
    const formElement = view.container.querySelector('form')!;
    fireEvent.submit(formElement);
    expect(
      allPreloads()
        .filter((l) => l !== entryLink)
        .map((l) => new URL(l.href).pathname)
    ).toEqual(['/assets/auth-startup-11111111.js']);
    pauseAuthenticatedPrewarm();
  } finally {
    document.documentElement.lang = previousLang;
    allPreloads().forEach((l) => l.remove());
  }
});

test('PROD profile uncaps static delta chunk prewarm (K=Infinity) and retains in-flight static links on submit', async () => {
  const previousLang = document.documentElement.lang;
  document.documentElement.lang = 'zh-CN';
  const prodProfile = resolveRuntimeConcurrencyProfile({
    isDev: false,
    hardwareConcurrency: 8
  });
  const allPreloads = () => [
    ...document.querySelectorAll<HTMLLinkElement>('link[rel="modulepreload"]')
  ];
  vi.stubGlobal(
    'fetch',
    vi.fn(async () => ({
      ok: true,
      json: async () => ({
        files: [
          'assets/auth-startup-11111111.js',
          'assets/i18n-shell-zh-22222222.js',
          'assets/i18n-shell-en-99999999.js',
          'assets/shell-core-33333333.js',
          'assets/frontstage-shell-44444444.js',
          'assets/frontstage-canvas-55555555.js',
          'assets/native-host-66666666.js',
          'assets/tail-extra-77777777.js'
        ]
      })
    }))
  );
  try {
    const view = render(<Host ready concurrencyProfile={prodProfile} />);
    await runIdle();
    const entryLink = preloads()[0]!;
    await act(async () => {
      fireEvent.load(entryLink);
    });
    await waitFor(() => {
      const deltaLinks = allPreloads().filter((l) => l !== entryLink);
      expect(deltaLinks).toHaveLength(7);
    });
    const formElement = view.container.querySelector('form')!;
    fireEvent.submit(formElement);
    // In PROD static builds, all static delta chunks burst immediately via native HTTP/2
    // multiplexing and stay intact on submit so AuthenticatedStartup reuses their streams.
    expect(
      allPreloads()
        .filter((l) => l !== entryLink)
        .map((l) => new URL(l.href).pathname)
    ).toEqual([
      '/assets/auth-startup-11111111.js',
      '/assets/i18n-shell-zh-22222222.js',
      '/assets/shell-core-33333333.js',
      '/assets/frontstage-shell-44444444.js',
      '/assets/frontstage-canvas-55555555.js',
      '/assets/native-host-66666666.js',
      '/assets/tail-extra-77777777.js'
    ]);
    pauseAuthenticatedPrewarm({ removePendingLinks: true });
  } finally {
    document.documentElement.lang = previousLang;
    allPreloads().forEach((l) => l.remove());
  }
});
