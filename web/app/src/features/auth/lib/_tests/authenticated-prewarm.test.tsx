import { StrictMode, useRef } from 'react';
import { act, render, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { useAuthenticatedPrewarm } from '../authenticated-prewarm';

const imported = vi.hoisted(() => vi.fn());
vi.mock('../../../../app/AuthenticatedAppRuntime', () => {
  imported();
  return { AuthenticatedAppRuntime: () => null };
});
vi.mock('../../components/default-password/DefaultPasswordSignIn', () => ({
  matchesDefaultPasswordSource: () => false
}));
function Host({ ready }: { ready: boolean }) {
  const host = useRef<HTMLDivElement>(null);
  useAuthenticatedPrewarm(host, 'same-entry');
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
let idle: IdleRequestCallback;
const request = vi.fn((callback: IdleRequestCallback) => {
  idle = callback;
  return 7;
});
const cancel = vi.fn();
beforeEach(() => {
  request.mockClear();
  cancel.mockClear();
  imported.mockClear();
  vi.stubGlobal('requestIdleCallback', request);
  vi.stubGlobal('cancelIdleCallback', cancel);
  Object.defineProperty(navigator, 'connection', {
    configurable: true,
    value: undefined
  });
});
afterEach(() => vi.unstubAllGlobals());
test('does not schedule during loading, schedules only after a real form commit, cancels on unmount', async () => {
  const view = render(
    <StrictMode>
      <Host ready={false} />
    </StrictMode>
  );
  expect(request).not.toHaveBeenCalled();
  expect(imported).not.toHaveBeenCalled();
  view.rerender(
    <StrictMode>
      <Host ready />
    </StrictMode>
  );
  await waitFor(() => expect(request).toHaveBeenCalledTimes(1));
  expect(imported).not.toHaveBeenCalled();
  view.unmount();
  expect(cancel).toHaveBeenCalledWith(7);
  await act(async () => idle({ didTimeout: false, timeRemaining: () => 50 }));
  expect(imported).not.toHaveBeenCalled();
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
});
test('connection is rechecked at idle, and a missing idle API never falls back to a timer', async () => {
  const view = render(<Host ready />);
  Object.defineProperty(navigator, 'connection', {
    configurable: true,
    value: { saveData: true }
  });
  await act(async () => idle({ didTimeout: false, timeRemaining: () => 50 }));
  expect(imported).not.toHaveBeenCalled();
  view.unmount();
  request.mockClear();
  vi.stubGlobal('requestIdleCallback', undefined);
  render(<Host ready />);
  expect(request).not.toHaveBeenCalled();
});
test('idle imports the authenticated runtime only after the form is committed', async () => {
  render(<Host ready />);
  expect(request).toHaveBeenCalledTimes(1);
  expect(imported).not.toHaveBeenCalled();
  await act(async () => idle({ didTimeout: false, timeRemaining: () => 50 }));
  await waitFor(() => expect(imported).toHaveBeenCalledTimes(1));
});
