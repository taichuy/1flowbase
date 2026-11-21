import { StrictMode, useRef } from 'react';
import { act, cleanup, fireEvent, render, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { useAuthenticatedPrewarm } from '../authenticated-prewarm';

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
  identity = 'same-entry'
}: {
  ready: boolean;
  identity?: string;
}) {
  const host = useRef<HTMLDivElement>(null);
  useAuthenticatedPrewarm(host, identity);
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
  cleanup();
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
