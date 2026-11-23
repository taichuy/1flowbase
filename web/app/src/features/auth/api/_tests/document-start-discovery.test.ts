// @vitest-environment jsdom
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import {
  discardDocumentLoginEntryDiscovery,
  startDocumentAuthDiscovery,
  takeDocumentLoginEntryDiscovery,
  takeDocumentSessionDiscovery
} from '../document-start-discovery';
import {
  consumeLoginEntryDiscovery,
  discardLoginEntryDiscovery,
  startLoginEntryDiscovery
} from '../login-entry-discovery';
import {
  resetAuthSessionDiscovery,
  startAuthSessionDiscovery
} from '../auth-session-discovery';
import {
  fetchCurrentMe,
  fetchCurrentSession,
  fetchLoginEntries
} from '../session';

vi.mock('../session', () => ({
  fetchCurrentMe: vi.fn(),
  fetchCurrentSession: vi.fn(),
  fetchLoginEntries: vi.fn()
}));

const entries = { default_login_entry_id: 'local', login_entries: [] };
beforeEach(() => {
  window.history.replaceState({}, '', '/sign-in');
  Reflect.deleteProperty(window, '__oneflowbaseSignInDiscovery');
  vi.mocked(fetchLoginEntries).mockResolvedValue(entries);
});
afterEach(() => {
  discardLoginEntryDiscovery();
  resetAuthSessionDiscovery();
  Reflect.deleteProperty(window, '__oneflowbaseSignInDiscovery');
  vi.resetAllMocks();
});

test('starts session and entries together, exactly once per document, outside React', () => {
  vi.mocked(fetchCurrentSession).mockReturnValue(new Promise(() => {}));
  startDocumentAuthDiscovery();
  startDocumentAuthDiscovery();
  expect(fetchCurrentSession).toHaveBeenCalledTimes(1);
  expect(fetchLoginEntries).toHaveBeenCalledTimes(1);
  expect(takeDocumentSessionDiscovery()).toBeDefined();
  expect(takeDocumentSessionDiscovery()).toBeUndefined();
  expect(takeDocumentLoginEntryDiscovery()).toBeDefined();
  expect(takeDocumentLoginEntryDiscovery()).toBeUndefined();
  startDocumentAuthDiscovery();
  expect(fetchCurrentSession).toHaveBeenCalledTimes(1);
});

test('starts session and entries on root / when served the credential-free opening document', () => {
  window.history.replaceState({}, '', '/');
  vi.mocked(fetchCurrentSession).mockReturnValue(new Promise(() => {}));
  startDocumentAuthDiscovery();
  expect(fetchCurrentSession).toHaveBeenCalledTimes(1);
  expect(fetchLoginEntries).toHaveBeenCalledTimes(1);
  expect(takeDocumentSessionDiscovery()).toBeDefined();
  expect(takeDocumentLoginEntryDiscovery()).toBeDefined();
});

test('does not speculate session or entries for unrelated routes', () => {
  window.history.replaceState({}, '', '/mcp/authorize?request_id=one');
  startDocumentAuthDiscovery();
  expect(fetchCurrentSession).not.toHaveBeenCalled();
  expect(fetchLoginEntries).not.toHaveBeenCalled();
});

test('resolved inline entries reach bootstrap and StrictMode consumers without a duplicate', async () => {
  vi.mocked(fetchCurrentSession).mockRejectedValue(new Error('anonymous'));
  startDocumentAuthDiscovery();
  await Promise.resolve();
  startLoginEntryDiscovery();
  const firstMount = consumeLoginEntryDiscovery();
  const strictRemount = consumeLoginEntryDiscovery();
  expect(strictRemount).toBe(firstMount);
  await expect(firstMount).resolves.toBe(entries);
  expect(fetchLoginEntries).toHaveBeenCalledTimes(1);
  await consumeLoginEntryDiscovery();
  expect(fetchLoginEntries).toHaveBeenCalledTimes(2);
});

test('consumes the inline session with existing anonymous/error semantics', async () => {
  vi.mocked(fetchCurrentSession).mockRejectedValue(new Error('401'));
  startDocumentAuthDiscovery();
  await expect(startAuthSessionDiscovery()).resolves.toEqual({
    status: 'anonymous'
  });
  expect(fetchCurrentSession).toHaveBeenCalledTimes(1);
  expect(fetchCurrentMe).not.toHaveBeenCalled();
});

test('failed inline entry discovery reaches its consumer and does not poison a revisit', async () => {
  vi.mocked(fetchCurrentSession).mockRejectedValue(new Error('401'));
  vi.mocked(fetchLoginEntries).mockRejectedValueOnce(new Error('offline'));
  startDocumentAuthDiscovery();
  await expect(consumeLoginEntryDiscovery()).rejects.toThrow('offline');
  await expect(consumeLoginEntryDiscovery()).resolves.toEqual(entries);
  expect(fetchLoginEntries).toHaveBeenCalledTimes(2);
});

test('discard on authentication prevents later consumers from using anonymous data', async () => {
  vi.mocked(fetchCurrentSession).mockRejectedValue(new Error('401'));
  startDocumentAuthDiscovery();
  discardDocumentLoginEntryDiscovery();
  await consumeLoginEntryDiscovery();
  expect(fetchLoginEntries).toHaveBeenCalledTimes(2);
});
